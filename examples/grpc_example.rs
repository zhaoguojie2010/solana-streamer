mod common;
use solana_streamer_sdk::streaming::{
    event_parser::{ParseOptions, ParsePlan, Protocol},
    yellowstone_grpc::{AccountFilter, StreamEvent, SubscriptionRequest, TransactionFilter},
    YellowstoneGrpc,
};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let client = YellowstoneGrpc::new(common::grpc_endpoint()?, common::grpc_token()?)?;
    let protocols = [
        Protocol::PumpFun,
        Protocol::PumpSwap,
        Protocol::Bonk,
        Protocol::RaydiumCpmm,
        Protocol::RaydiumClmm,
        Protocol::RaydiumAmmV4,
        Protocol::MeteoraDammV2,
        Protocol::MeteoraDlmm,
        Protocol::Whirlpool,
        Protocol::PancakeSwap,
    ];
    let owners: Vec<String> =
        protocols.iter().flat_map(Protocol::get_program_id).map(|key| key.to_string()).collect();
    let plan = ParsePlan::new(
        &protocols,
        None,
        ParseOptions { enrich_logs: true, ..ParseOptions::default() },
    );
    let mut request = SubscriptionRequest::new(plan);
    request.transactions =
        vec![TransactionFilter { account_include: owners.clone(), ..TransactionFilter::default() }];
    request.accounts = vec![AccountFilter { owner: owners, ..AccountFilter::default() }];
    request.blocks_meta = true;
    client
        .subscribe(request, |event| match event {
            StreamEvent::Transaction(batch) => {
                println!(
                    "signature={} slot={} events={}",
                    batch.meta.signature,
                    batch.meta.slot,
                    batch.events.len()
                );
                for event in batch.events {
                    println!("{event:?}");
                }
            }
            StreamEvent::Account(account) => {
                // Read a few checked fields directly, or decode a full snapshot on demand.
                println!(
                    "account={} slot={} bytes={}",
                    account.frame.pubkey,
                    account.frame.slot,
                    account.bytes().len()
                );
                if let Some(snapshot) = account.decode() {
                    println!("{snapshot:?}");
                }
            }
            StreamEvent::BlockMeta(block) => println!("{block:?}"),
            StreamEvent::Slot(slot) => println!("{slot:?}"),
        })
        .await?;
    let shutdown = common::wait_for_shutdown(&client.subscription_handle).await;
    let stopped = client.stop().await;
    shutdown?;
    stopped
}
