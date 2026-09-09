//! RPC I/O and synchronous parsing use separate APIs.
mod common;
use anyhow::{Context, Result};
use solana_streamer_sdk::rpc::{
    transaction_frame, types::UiTransactionEncoding, CommitmentConfig, RpcClient,
    RpcTransactionConfig,
};
use solana_streamer_sdk::streaming::event_parser::{
    common::high_performance_clock::get_high_perf_clock, ParseOptions, ParsePlan, Protocol,
    TxParser,
};
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let signatures: Vec<solana_sdk::signature::Signature> = std::env::args()
        .skip(1)
        .map(|signature| {
            signature.parse().with_context(|| format!("invalid signature: {signature}"))
        })
        .collect::<Result<_>>()?;
    anyhow::ensure!(
        !signatures.is_empty(),
        "usage: cargo run --features rpc-client --example parse_tx_events -- <signature> [signature...]"
    );
    let client = RpcClient::new_with_timeout(
        common::env_or_default("SOLANA_RPC_URL", "https://api.mainnet-beta.solana.com")?,
        std::time::Duration::from_secs(30),
    );
    let plan = ParsePlan::new(
        &[
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
        ],
        None,
        ParseOptions {
            enrich_logs: true,
            retain_instructions: true,
            balance_audit: true,
            ..ParseOptions::default()
        },
    );
    let mut parser = TxParser::default();
    for signature in signatures {
        let response = client
            .get_transaction_with_config(
                &signature,
                RpcTransactionConfig {
                    encoding: Some(UiTransactionEncoding::Base64),
                    commitment: Some(CommitmentConfig::confirmed()),
                    max_supported_transaction_version: Some(0),
                },
            )
            .await
            .with_context(|| format!("cannot fetch {signature}"))?;
        let frame = transaction_frame(response, get_high_perf_clock())?;
        parser.visit(&frame, &plan, |batch| {
            println!(
                "signature={} slot={} status={:?}",
                batch.meta.signature, batch.meta.slot, batch.meta.execution_status
            );
            for event in batch.events {
                println!("{event:?}");
                if let Some(raw) = batch.instruction(event.metadata().instruction_index) {
                    println!(
                        "raw program={} accounts={} bytes={}",
                        raw.program_id,
                        raw.accounts.len(),
                        raw.data.len()
                    );
                }
            }
            println!("audit={:?}", batch.audit);
        })?;
    }
    Ok(())
}
