#[path = "../examples/common/mod.rs"]
mod common;

use futures::stream;
use solana_streamer_sdk::streaming::YellowstoneGrpc;
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tonic::{
    codegen::{http, BoxFuture, Service},
    Request, Response, Status,
};
use yellowstone_grpc_proto::geyser::{SubscribeRequest, SubscribeUpdate};

struct RateLimitedStream;

impl tonic::server::StreamingService<SubscribeRequest> for RateLimitedStream {
    type Response = SubscribeUpdate;
    type ResponseStream =
        Pin<Box<dyn futures::Stream<Item = Result<SubscribeUpdate, Status>> + Send>>;
    type Future = BoxFuture<Response<Self::ResponseStream>, Status>;

    fn call(&mut self, _request: Request<tonic::Streaming<SubscribeRequest>>) -> Self::Future {
        Box::pin(async {
            // Accept the subscription, then fail in the background stream.
            let updates: Self::ResponseStream = Box::pin(stream::once(async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Err(Status::resource_exhausted("test subscription rate limit"))
            }));
            Ok(Response::new(updates))
        })
    }
}

/// The test server implements only the Geyser subscription used by the example.
#[derive(Clone)]
struct TestGeyserService;

impl tonic::server::NamedService for TestGeyserService {
    const NAME: &'static str = "geyser.Geyser";
}

impl Service<http::Request<tonic::body::Body>> for TestGeyserService {
    type Response = http::Response<tonic::body::Body>;
    type Error = Infallible;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<tonic::body::Body>) -> Self::Future {
        assert_eq!(request.uri().path(), "/geyser.Geyser/Subscribe");
        Box::pin(async move {
            let codec = tonic_prost::ProstCodec::<SubscribeUpdate, SubscribeRequest>::default();
            let mut grpc = tonic::server::Grpc::new(codec);
            Ok(grpc.streaming(RateLimitedStream, request).await)
        })
    }
}

#[tokio::test]
async fn example_exits_when_the_subscription_fails_after_connecting() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let incoming = stream::unfold(listener, |listener| async {
        let socket = listener.accept().await.map(|(socket, _)| socket);
        Some((socket, listener))
    });
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(TestGeyserService)
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });

    let client = YellowstoneGrpc::new(format!("http://{address}"), None).unwrap();
    client
        .subscribe_events_immediate(vec![], None, vec![], vec![], None, None, |_| {})
        .await
        .unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        common::wait_for_shutdown(&client.subscription_handle),
    )
    .await;
    client.stop().await;
    server.abort();

    let error = result.expect("the example must not keep waiting for Ctrl+C").unwrap_err();
    assert!(error.to_string().contains("Subscription stream ended unexpectedly"));
}
