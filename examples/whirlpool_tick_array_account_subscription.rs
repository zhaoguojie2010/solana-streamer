mod common;

use solana_streamer_sdk::streaming::{
    event_parser::{protocols::whirlpool::parser::WHIRLPOOL_PROGRAM_ID, DexEvent, Protocol},
    grpc::ClientConfig,
    yellowstone_grpc::{AccountFilter, TransactionFilter},
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

    let transaction_filter = TransactionFilter {
        account_include: vec![WHIRLPOOL_PROGRAM_ID.to_string()],
        account_exclude: vec![],
        account_required: vec![],
    };

    use solana_streamer_sdk::streaming::event_parser::common::filter::EventTypeFilter;
    use solana_streamer_sdk::streaming::event_parser::common::EventType;
    let event_type_filter =
        Some(EventTypeFilter { include: vec![EventType::AccountWhirlpoolTickArray] });

    println!("开始监听事件，按 Ctrl+C 停止...");
    println!("监控程序: {}", WHIRLPOOL_PROGRAM_ID);

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

    println!("等待 Ctrl+C 停止...");
    let shutdown = common::wait_for_shutdown(&grpc.subscription_handle).await;
    grpc.stop().await;
    shutdown?;

    Ok(())
}

fn create_event_callback() -> impl Fn(DexEvent) {
    |event: DexEvent| match event {
        DexEvent::WhirlpoolTickArrayAccountEvent(e) => {
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
