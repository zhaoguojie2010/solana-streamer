//! Own whole batches across awaits without cloning event or instruction buffers.
mod common;
use solana_streamer_sdk::streaming::{
    common::{OwnedStreamEvent, QueueConfig},
    event_parser::{ParsePlan, Protocol},
    yellowstone_grpc::{SubscriptionRequest, TransactionFilter},
    YellowstoneGrpc,
};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let client = YellowstoneGrpc::new(common::grpc_endpoint()?, common::grpc_token()?)?;
    let mut request = SubscriptionRequest::new(ParsePlan::all(&[Protocol::PumpFun]));
    request.transactions = vec![TransactionFilter {
        account_include: Protocol::PumpFun
            .get_program_id()
            .iter()
            .map(ToString::to_string)
            .collect(),
        ..TransactionFilter::default()
    }];
    let mut stream = client.subscribe_queued(request, QueueConfig::default()).await?;
    let result = tokio::select! {
        result=common::wait_for_shutdown(&client.subscription_handle)=>result,
        result=async {
            while let Some(envelope)=stream.recv().await? {
                if let OwnedStreamEvent::Transaction(batch)=&*envelope {
                    println!("{}: {} events",batch.meta.signature,batch.events.len());
                    // Keep the envelope alive across database/network awaits; it holds the byte permit.
                    tokio::task::yield_now().await;
                }
            }
            anyhow::Ok(())
        }=>result,
    };
    let stopped = client.stop().await;
    // Graceful stop closes production. Accepted queue items can still be drained here.
    while let Some(envelope) = stream.recv().await? {
        println!("draining: {envelope:?}");
    }
    result?;
    stopped
}
