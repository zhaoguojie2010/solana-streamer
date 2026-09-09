#[path = "../examples/common/mod.rs"]
mod common;

use futures::{stream, StreamExt};
use solana_streamer_sdk::streaming::{
    common::{OwnedStreamEvent, QueueConfig, StreamClientConfig},
    event_parser::{common::EventType, ParseOptions, ParsePlan},
    yellowstone_grpc::{StreamEvent, SubscriptionRequest, TransactionFilter},
    YellowstoneGrpc,
};
use std::{
    convert::Infallible,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};
use tokio::sync::mpsc;
use tonic::{
    codegen::{http, BoxFuture, Service},
    Request, Response, Status,
};
use yellowstone_grpc_proto::prelude::*;

type UpdateReceiver = mpsc::Receiver<Result<SubscribeUpdate, Status>>;
#[derive(Clone)]
struct TestGeyserService {
    updates: Arc<Mutex<Option<UpdateReceiver>>>,
    requests: mpsc::Sender<SubscribeRequest>,
}
impl tonic::server::StreamingService<SubscribeRequest> for TestGeyserService {
    type Response = SubscribeUpdate;
    type ResponseStream =
        Pin<Box<dyn futures::Stream<Item = Result<SubscribeUpdate, Status>> + Send>>;
    type Future = BoxFuture<Response<Self::ResponseStream>, Status>;
    fn call(&mut self, request: Request<tonic::Streaming<SubscribeRequest>>) -> Self::Future {
        let updates = self.updates.lock().unwrap().take().unwrap();
        let requests = self.requests.clone();
        let mut input = request.into_inner();
        Box::pin(async move {
            tokio::spawn(async move {
                while let Some(Ok(request)) = input.next().await {
                    if requests.send(request).await.is_err() {
                        break;
                    }
                }
            });
            let output: Self::ResponseStream =
                Box::pin(stream::unfold(updates, |mut updates| async {
                    updates.recv().await.map(|value| (value, updates))
                }));
            Ok(Response::new(output))
        })
    }
}
impl tonic::server::NamedService for TestGeyserService {
    const NAME: &'static str = "geyser.Geyser";
}
impl Service<http::Request<tonic::body::Body>> for TestGeyserService {
    type Response = http::Response<tonic::body::Body>;
    type Error = Infallible;
    type Future = BoxFuture<Self::Response, Self::Error>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, request: http::Request<tonic::body::Body>) -> Self::Future {
        assert_eq!(request.uri().path(), "/geyser.Geyser/Subscribe");
        let service = self.clone();
        Box::pin(async move {
            let codec = tonic_prost::ProstCodec::<SubscribeUpdate, SubscribeRequest>::default();
            Ok(tonic::server::Grpc::new(codec).streaming(service, request).await)
        })
    }
}
async fn server() -> (
    String,
    mpsc::Sender<Result<SubscribeUpdate, Status>>,
    mpsc::Receiver<SubscribeRequest>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let incoming = stream::unfold(listener, |listener| async {
        let socket = listener.accept().await.map(|(socket, _)| socket);
        Some((socket, listener))
    });
    let (updates, receiver) = mpsc::channel(16);
    let (requests, request_receiver) = mpsc::channel(16);
    let service = TestGeyserService { updates: Arc::new(Mutex::new(Some(receiver))), requests };
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });
    (format!("http://{address}"), updates, request_receiver, task)
}
fn request(types: Option<&[EventType]>) -> SubscriptionRequest {
    let mut request = SubscriptionRequest::new(ParsePlan::new(&[], types, ParseOptions::default()));
    request.transactions = vec![TransactionFilter::default()];
    request
}
fn transaction(slot: u64) -> SubscribeUpdate {
    let compute = solana_sdk::pubkey!("ComputeBudget111111111111111111111111111111");
    SubscribeUpdate {
        update_oneof: Some(subscribe_update::UpdateOneof::Transaction(
            SubscribeUpdateTransaction {
                slot,
                transaction: Some(SubscribeUpdateTransactionInfo {
                    signature: vec![7; 64],
                    transaction: Some(Transaction {
                        message: Some(Message {
                            account_keys: vec![compute.to_bytes().to_vec()],
                            instructions: vec![
                                CompiledInstruction {
                                    program_id_index: 0,
                                    accounts: vec![],
                                    data: [&[2][..], &123u32.to_le_bytes()].concat(),
                                },
                                CompiledInstruction {
                                    program_id_index: 0,
                                    accounts: vec![],
                                    data: [&[3][..], &456u64.to_le_bytes()].concat(),
                                },
                            ],
                            ..Message::default()
                        }),
                        ..Transaction::default()
                    }),
                    meta: Some(TransactionStatusMeta::default()),
                    ..SubscribeUpdateTransactionInfo::default()
                }),
            },
        )),
        ..SubscribeUpdate::default()
    }
}
async fn ended(client: &YellowstoneGrpc) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !client.subscription_handle.lock().await.as_ref().unwrap().is_finished() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("worker must terminate");
}

#[tokio::test]
async fn example_exits_when_the_subscription_fails_after_connecting() {
    let (endpoint, updates, mut requests, server) = server().await;
    let client = YellowstoneGrpc::new(endpoint, None).unwrap();
    client.subscribe(request(None), |_| {}).await.unwrap();
    requests.recv().await.unwrap();
    updates.send(Err(Status::resource_exhausted("test subscription rate limit"))).await.unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        common::wait_for_shutdown(&client.subscription_handle),
    )
    .await;
    assert!(client.stop().await.is_err());
    server.abort();
    let error = result.expect("example must not keep waiting for Ctrl+C").unwrap_err();
    assert!(error.to_string().contains("Subscription stream ended unexpectedly"));
    assert!(client.last_error().unwrap().contains("test subscription rate limit"));
}

#[tokio::test]
async fn borrowed_fn_mut_and_dynamic_plan_updates_apply_between_messages() {
    let (endpoint, updates, mut requests, server) = server().await;
    let client = YellowstoneGrpc::new(endpoint, None).unwrap();
    let (observed, mut received) = mpsc::unbounded_channel();
    // Cell is Send but not Sync: callbacks need no shared callback state or Arc adapter.
    let counter = std::cell::Cell::new(0);
    client
        .subscribe(request(Some(&[EventType::SetComputeUnitLimit])), move |event| {
            if let StreamEvent::Transaction(batch) = event {
                counter.set(counter.get() + 1);
                observed
                    .send((
                        counter.get(),
                        batch.meta.slot,
                        batch.events.iter().map(|e| e.metadata().event_type).collect::<Vec<_>>(),
                    ))
                    .unwrap();
            }
        })
        .await
        .unwrap();
    requests.recv().await.unwrap();
    updates.send(Ok(transaction(1))).await.unwrap();
    let first =
        tokio::time::timeout(Duration::from_secs(2), received.recv()).await.unwrap().unwrap();
    assert_eq!(first, (1, 1, vec![EventType::SetComputeUnitLimit]));
    client.update_subscription(request(Some(&[EventType::SetComputeUnitPrice]))).await.unwrap();
    requests.recv().await.unwrap();
    updates.send(Ok(transaction(2))).await.unwrap();
    let second =
        tokio::time::timeout(Duration::from_secs(2), received.recv()).await.unwrap().unwrap();
    assert_eq!(second, (2, 2, vec![EventType::SetComputeUnitPrice]));
    client.stop().await.unwrap();
    server.abort();
}

#[tokio::test]
async fn owned_overload_preserves_accepted_batch_then_reports_failure() {
    let (endpoint, updates, mut requests, server) = server().await;
    let client = YellowstoneGrpc::new(endpoint, None).unwrap();
    let mut output = client
        .subscribe_queued(request(None), QueueConfig { capacity: 1, ..QueueConfig::default() })
        .await
        .unwrap();
    requests.recv().await.unwrap();
    updates.send(Ok(transaction(1))).await.unwrap();
    updates.send(Ok(transaction(2))).await.unwrap();
    ended(&client).await;
    let envelope = output.recv().await.unwrap().unwrap();
    let OwnedStreamEvent::Transaction(batch) = &*envelope else { panic!("wrong event") };
    assert_eq!(batch.meta.slot, 1);
    assert_eq!(batch.events.len(), 2);
    assert!(output.recv().await.unwrap_err().to_string().contains("queue unavailable"));
    assert!(client.stop().await.is_err());
    server.abort();
}

#[tokio::test]
async fn stopping_production_allows_queue_drain_and_metrics_are_per_client() {
    let (endpoint, updates, mut requests, server) = server().await;
    let enabled = YellowstoneGrpc::new_with_config(
        endpoint,
        None,
        StreamClientConfig { enable_metrics: true, ..StreamClientConfig::default() },
    )
    .unwrap();
    let disabled = YellowstoneGrpc::new("http://127.0.0.1:1".into(), None).unwrap();
    let mut output = enabled.subscribe_queued(request(None), QueueConfig::default()).await.unwrap();
    requests.recv().await.unwrap();
    updates.send(Ok(transaction(42))).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while enabled.get_metrics().transactions != 1 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(disabled.get_metrics().transactions, 0);
    assert_eq!(enabled.get_metrics().events, 2);
    enabled.stop().await.unwrap();
    let envelope = output.recv().await.unwrap().unwrap();
    let OwnedStreamEvent::Transaction(batch) = &*envelope else { panic!("wrong event") };
    assert_eq!(batch.meta.slot, 42);
    assert!(output.recv().await.unwrap().is_none());
    server.abort();
}

#[tokio::test]
async fn dropping_owned_receiver_stops_idle_producer() {
    let (endpoint, _updates, mut requests, server) = server().await;
    let client = YellowstoneGrpc::new(endpoint, None).unwrap();
    let output = client.subscribe_queued(request(None), QueueConfig::default()).await.unwrap();
    requests.recv().await.unwrap();
    drop(output);
    ended(&client).await;
    client.stop().await.unwrap();
    server.abort();
}

#[tokio::test]
async fn callback_panic_is_retained_as_subscription_failure() {
    let (endpoint, updates, mut requests, server) = server().await;
    let client = YellowstoneGrpc::new(endpoint, None).unwrap();
    client.subscribe(request(None), |_| panic!("intentional callback failure")).await.unwrap();
    requests.recv().await.unwrap();
    updates.send(Ok(transaction(1))).await.unwrap();
    ended(&client).await;
    assert!(client.last_error().unwrap().contains("intentional callback failure"));
    assert!(client.stop().await.is_err());
    server.abort();
}
