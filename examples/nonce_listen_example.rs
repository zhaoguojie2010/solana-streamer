mod common;

use solana_streamer_sdk::streaming::{
    event_parser::{
        common::{filter::EventTypeFilter, EventType},
        DexEvent,
    },
    grpc::ClientConfig,
    yellowstone_grpc::{AccountFilter, TransactionFilter},
    YellowstoneGrpc,
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

    let nonce_account = common::required_pubkey("NONCE_ACCOUNT")?;
    // Listen to account data belonging to owner programs -> account event monitoring
    let account_filter = AccountFilter {
        account: vec![nonce_account],
        owner: vec![],
        filters: vec![],
        cuckoo_accounts_filter: None,
    };

    // Event filtering
    let event_type_filter = Some(EventTypeFilter { include: vec![EventType::NonceAccount] });

    println!("Starting to listen for events, press Ctrl+C to stop...");
    println!("Starting subscription...");

    grpc.subscribe_events_immediate(
        protocols,
        None,
        vec![transaction_filter],
        vec![account_filter],
        event_type_filter,
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
    |event: DexEvent| {
        println!("🎉 Event received! {:?}", event);
    }
}
