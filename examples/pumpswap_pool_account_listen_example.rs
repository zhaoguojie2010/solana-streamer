use solana_streamer_sdk::streaming::{
    event_parser::{ParseOptions, ParsePlan},
    yellowstone_grpc::{StreamEvent, SubscriptionRequest},
};
mod common;

use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;
use solana_streamer_sdk::streaming::{
    event_parser::{
        common::{filter::EventTypeFilter, EventType},
        AccountEvent,
    },
    grpc::ClientConfig,
    yellowstone_grpc::AccountFilter,
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

    // Listen to transaction data

    // Pump.fun AMM (PUMP-USDC) Market
    let pump_usdc = Pubkey::from_str("2uF4Xh61rDwxnG9woyxsVQP7zuA6kLFpb3NvnRQeoiSd").unwrap();
    let wsol_deepseekai = Pubkey::from_str("BJAjivuMVANjpRWtrRfcxzGhnMSywBN19Sa4jAzWxXDx").unwrap();

    // Listen to account data belonging to owner programs -> account event monitoring
    let pump_usdc_account_filter = AccountFilter {
        account: vec![],
        owner: vec![],
        filters: vec![SubscribeRequestFilterAccountsFilter {
            filter: Some(Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp {
                offset: 32,
                data: Some(Data::Bytes(pump_usdc.to_bytes().to_vec())),
            })),
        }],
        cuckoo_accounts_filter: None,
    };
    let wsol_deepseekai_account_filter = AccountFilter {
        account: vec![],
        owner: vec![],
        filters: vec![SubscribeRequestFilterAccountsFilter {
            filter: Some(Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp {
                offset: 32,
                data: Some(Data::Bytes(wsol_deepseekai.to_bytes().to_vec())),
            })),
        }],
        cuckoo_accounts_filter: None,
    };

    // Event filtering
    let event_type_filter = Some(EventTypeFilter { include: vec![EventType::TokenAccount] });

    println!("Starting to listen for events, press Ctrl+C to stop...");
    println!("Starting subscription...");

    let plan = ParsePlan::new(
        &protocols,
        event_type_filter.as_ref().map(
            |f: &solana_streamer_sdk::streaming::event_parser::common::filter::EventTypeFilter| {
                f.include.as_slice()
            },
        ),
        ParseOptions::default(),
    );
    let mut request = SubscriptionRequest::new(plan);
    request.transactions = Vec::new();
    request.accounts = vec![pump_usdc_account_filter, wsol_deepseekai_account_filter];
    grpc.subscribe(request, callback).await?;

    println!("Waiting for Ctrl+C to stop...");
    let shutdown = common::wait_for_shutdown(&grpc.subscription_handle).await;
    let stopped = grpc.stop().await;
    shutdown?;
    stopped?;

    Ok(())
}

fn create_event_callback() -> impl for<'a> FnMut(StreamEvent<'a>) {
    |stream| {
        let StreamEvent::Account(view) = stream else {
            return;
        };
        let Some(event) = view.decode() else {
            return;
        };
        match event {
            AccountEvent::TokenAccountEvent(e) => {
                println!("TokenAccount: {:?} amount: {:?}", e.pubkey, e.amount);
            }
            _ => {}
        }
    }
}
