mod common;

use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;
use solana_streamer_sdk::streaming::{
    event_parser::{
        common::{filter::EventTypeFilter, EventType},
        DexEvent,
    },
    grpc::ClientConfig,
    yellowstone_grpc::{AccountFilter, TransactionFilter},
    YellowstoneGrpc,
};
use yellowstone_grpc_proto::geyser::{
    subscribe_request_filter_accounts_filter::Filter,
    subscribe_request_filter_accounts_filter_memcmp::Data, SubscribeRequestFilterAccountsFilter,
    SubscribeRequestFilterAccountsFilterMemcmp,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    println!("Starting Yellowstone gRPC Streamer...");
    test_grpc().await?;
    Ok(())
}

async fn test_grpc() -> Result<(), Box<dyn std::error::Error>> {
    println!("Subscribing to Yellowstone gRPC events...");
    // Create low-latency configuration
    let mut config: ClientConfig = ClientConfig::default();
    // Enable performance monitoring, has performance overhead, disabled by default
    config.enable_metrics = true;
    let grpc =
        YellowstoneGrpc::new_with_config(common::grpc_endpoint()?, common::grpc_token()?, config)?;
    println!("GRPC client created successfully");
    let callback = create_event_callback();
    // Will try to parse corresponding protocol events from transactions
    let protocols = vec![];
    println!("Protocols to monitor: {:?}", protocols);
    // Filter accounts
    let account_include = vec![];
    let account_exclude = vec![];
    let account_required = vec![];

    // Listen to transaction data
    let transaction_filter =
        TransactionFilter { account_include, account_exclude, account_required };

    let pump = Pubkey::from_str("pumpCmXqMfrsAkQ5r49WcJnRayYRqmXz6ae8H7H9Dfn").unwrap();
    let usdc = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();

    let all_pump_ata = AccountFilter {
        account: vec![],
        owner: vec![],
        filters: vec![SubscribeRequestFilterAccountsFilter {
            filter: Some(Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp {
                offset: 0,
                data: Some(Data::Bytes(pump.to_bytes().to_vec())),
            })),
        }],
        cuckoo_accounts_filter: None,
    };
    let all_usdc_ata = AccountFilter {
        account: vec![],
        owner: vec![],
        filters: vec![SubscribeRequestFilterAccountsFilter {
            filter: Some(Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp {
                offset: 0,
                data: Some(Data::Bytes(usdc.to_bytes().to_vec())),
            })),
        }],
        cuckoo_accounts_filter: None,
    };

    // Event filtering
    let event_type_filter = Some(EventTypeFilter { include: vec![EventType::TokenAccount] });

    println!("Starting to listen for events, press Ctrl+C to stop...");
    println!("Starting subscription...");

    grpc.subscribe_events_immediate(
        protocols.clone(),
        None,
        vec![transaction_filter.clone()],
        vec![all_pump_ata.clone(), all_usdc_ata.clone()],
        event_type_filter.clone(),
        None,
        callback,
    )
    .await?;

    println!("Waiting for Ctrl+C to stop...");
    let shutdown = common::wait_for_shutdown(&grpc.subscription_handle).await;
    grpc.stop().await;
    shutdown?;

    Ok(())
}

fn create_event_callback() -> impl Fn(DexEvent) {
    |event: DexEvent| match event {
        DexEvent::TokenAccountEvent(e) => {
            println!("TokenAccount: {:?} amount: {:?}", e.pubkey, e.amount);
        }
        _ => {}
    }
}
