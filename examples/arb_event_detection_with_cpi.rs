//! Transaction-local classification replaces global maps keyed by signature.
mod common;
use solana_streamer_sdk::streaming::{
    event_parser::{ParseOptions, ParsePlan, Protocol, TxSwapKind},
    yellowstone_grpc::{StreamEvent, SubscriptionRequest, TransactionFilter},
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
    let plan = ParsePlan::new(
        &protocols,
        None,
        ParseOptions {
            classify_swaps: true,
            enrich_logs: true,
            retain_instructions: true,
            ..ParseOptions::default()
        },
    );
    let mut request = SubscriptionRequest::new(plan);
    request.transactions = vec![TransactionFilter {
        account_include: protocols
            .iter()
            .flat_map(Protocol::get_program_id)
            .map(|key| key.to_string())
            .collect(),
        ..TransactionFilter::default()
    }];
    client
        .subscribe(request, |event| {
            let StreamEvent::Transaction(batch) = event else {
                return;
            };
            if !matches!(batch.summary.swap_kind, Some(TxSwapKind::Arb | TxSwapKind::Route)) {
                return;
            }
            println!(
                "signature={} slot={} kind={:?}",
                batch.meta.signature, batch.meta.slot, batch.summary.swap_kind
            );
            for event in batch.events {
                let meta = event.metadata();
                println!(
                    "outer={} inner={:?} event={:?}",
                    meta.outer_index, meta.inner_index, meta.event_type
                );
                if let Some(instruction) = batch.instruction(meta.instruction_index) {
                    println!(
                        "program={} data_bytes={}",
                        instruction.program_id,
                        instruction.data.len()
                    );
                }
            }
        })
        .await?;
    let shutdown = common::wait_for_shutdown(&client.subscription_handle).await;
    let stopped = client.stop().await;
    shutdown?;
    stopped
}
