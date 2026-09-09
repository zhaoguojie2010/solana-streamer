#[path = "../examples/common/mod.rs"]
mod common;

use futures::stream;
use solana_streamer_sdk::{
    protos::shredstream::{
        shredstream_proxy_server::{ShredstreamProxy, ShredstreamProxyServer},
        Entry, SubscribeEntriesRequest,
    },
    streaming::ShredStreamGrpc,
};
use std::{pin::Pin, time::Duration};
use tonic::{Request, Response, Status};

struct RateLimitedStream;

#[tonic::async_trait]
impl ShredstreamProxy for RateLimitedStream {
    type SubscribeEntriesStream =
        Pin<Box<dyn futures::Stream<Item = Result<Entry, Status>> + Send>>;

    async fn subscribe_entries(
        &self,
        _request: Request<SubscribeEntriesRequest>,
    ) -> Result<Response<Self::SubscribeEntriesStream>, Status> {
        // Accept the subscription, then fail in the background stream.
        Ok(Response::new(Box::pin(stream::once(async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Err(Status::resource_exhausted("test subscription rate limit"))
        }))))
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
            .add_service(ShredstreamProxyServer::new(RateLimitedStream))
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });

    let client = ShredStreamGrpc::new(format!("http://{address}")).await.unwrap();
    client.shredstream_subscribe(vec![], None, None, |_| {}).await.unwrap();
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
