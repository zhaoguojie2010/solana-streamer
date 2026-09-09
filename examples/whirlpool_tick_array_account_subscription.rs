use solana_streamer_sdk::streaming::{
    event_parser::{ParseOptions, ParsePlan},
    yellowstone_grpc::{StreamEvent, SubscriptionRequest},
};
mod common;

use solana_streamer_sdk::streaming::{
    event_parser::{protocols::whirlpool::parser::WHIRLPOOL_PROGRAM_ID, AccountEvent, Protocol},
    grpc::ClientConfig,
    yellowstone_grpc::AccountFilter,
    YellowstoneGrpc,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    println!("开始 Whirlpool TickArray 账户数据订阅示例...");
    subscribe_whirlpool_tick_array_accounts().await?;
    Ok(())
}

async fn subscribe_whirlpool_tick_array_accounts() -> Result<(), Box<dyn std::error::Error>> {
    println!("订阅 Whirlpool TickArray 账户数据...");

    let mut config: ClientConfig = ClientConfig::default();
    config.enable_metrics = true;
    let grpc =
        YellowstoneGrpc::new_with_config(common::grpc_endpoint()?, common::grpc_token()?, config)?;

    println!("gRPC 客户端创建成功");

    let callback = create_event_callback();

    let protocols = vec![Protocol::Whirlpool];

    println!("监控协议: {:?}", protocols);

    let account_filter = AccountFilter {
        account: vec![],
        owner: vec![WHIRLPOOL_PROGRAM_ID.to_string()],
        filters: vec![],
        cuckoo_accounts_filter: None,
    };

    use solana_streamer_sdk::streaming::event_parser::common::filter::EventTypeFilter;
    use solana_streamer_sdk::streaming::event_parser::common::EventType;
    let event_type_filter =
        Some(EventTypeFilter { include: vec![EventType::AccountWhirlpoolTickArray] });

    println!("开始监听事件，按 Ctrl+C 停止...");
    println!("监控程序: {}", WHIRLPOOL_PROGRAM_ID);

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
    request.accounts = vec![account_filter];
    grpc.subscribe(request, callback).await?;

    println!("等待 Ctrl+C 停止...");
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
            AccountEvent::WhirlpoolTickArrayAccountEvent(e) => {
                let initialized_ticks =
                    e.tick_array.ticks.iter().filter(|tick| tick.initialized).count();
                println!("=== Whirlpool TickArray 账户更新 ===");
                println!("账户地址: {}", e.pubkey);
                println!("Whirlpool: {}", e.tick_array.whirlpool);
                println!("Start Tick Index: {}", e.tick_array.start_tick_index);
                println!("Initialized Ticks: {}", initialized_ticks);
                println!("=====================================");
            }
            _ => {}
        }
    }
}
