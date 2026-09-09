//! Update filters and a complete parsing plan between stream messages.
mod common;
use solana_streamer_sdk::streaming::{
    event_parser::{common::EventType, ParseOptions, ParsePlan, Protocol},
    yellowstone_grpc::{StreamEvent, SubscriptionRequest, TransactionFilter},
    YellowstoneGrpc,
};
use std::time::Duration;
fn request(protocol: Protocol, types: &[EventType]) -> SubscriptionRequest {
    let mut request =
        SubscriptionRequest::new(ParsePlan::new(&[protocol], Some(types), ParseOptions::default()));
    request.transactions = vec![TransactionFilter {
        account_include: protocol.get_program_id().iter().map(ToString::to_string).collect(),
        ..TransactionFilter::default()
    }];
    request
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let client = YellowstoneGrpc::new(common::grpc_endpoint()?, common::grpc_token()?)?;
    client
        .subscribe(
            request(Protocol::PumpFun, &[EventType::PumpFunBuy, EventType::PumpFunSell]),
            |event| {
                if let StreamEvent::Transaction(batch) = event {
                    println!("{}: {} events", batch.meta.signature, batch.events.len());
                }
            },
        )
        .await?;
    let shutdown = tokio::select! {
        result = common::wait_for_shutdown(&client.subscription_handle) => result,
        result = async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            client.update_subscription(request(Protocol::PumpSwap, &[EventType::PumpSwapBuy, EventType::PumpSwapSell])).await?;
            common::wait_for_shutdown(&client.subscription_handle).await
        } => result,
    };
    let stopped = client.stop().await;
    shutdown?;
    stopped
}
