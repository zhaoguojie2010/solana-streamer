use solana_streamer_sdk::streaming::{
    event_parser::{
        protocols::whirlpool::parser::WHIRLPOOL_PROGRAM_ID, DexEvent, Protocol,
    },
    grpc::ClientConfig,
    yellowstone_grpc::{AccountFilter, TransactionFilter},
    YellowstoneGrpc,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("开始 Whirlpool TickArray 账户数据订阅示例...");
    subscribe_whirlpool_tick_array_accounts().await?;
    Ok(())
}

async fn subscribe_whirlpool_tick_array_accounts() -> Result<(), Box<dyn std::error::Error>> {
    println!("订阅 Whirlpool TickArray 账户数据...");

    let mut config: ClientConfig = ClientConfig::default();
    config.enable_metrics = true;
    let grpc = YellowstoneGrpc::new_with_config(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
        config,
    )?;

    println!("gRPC 客户端创建成功");

    let callback = create_event_callback();

    let protocols = vec![Protocol::Whirlpool];

    println!("监控协议: {:?}", protocols);

    let account_filter = AccountFilter {
        account: vec![],
        owner: vec![WHIRLPOOL_PROGRAM_ID.to_string()],
        filters: vec![],
    };

    let transaction_filter = TransactionFilter {
        account_include: vec![WHIRLPOOL_PROGRAM_ID.to_string()],
        account_exclude: vec![],
        account_required: vec![],
    };

    use solana_streamer_sdk::streaming::event_parser::common::EventType;
    use solana_streamer_sdk::streaming::event_parser::common::filter::EventTypeFilter;
    let event_type_filter = Some(EventTypeFilter {
        include: vec![EventType::AccountWhirlpoolTickArray],
    });

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

    let grpc_clone = grpc.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1000)).await;
        grpc_clone.stop().await;
    });

    println!("等待 Ctrl+C 停止...");
    tokio::signal::ctrl_c().await?;

    Ok(())
}

fn create_event_callback() -> impl Fn(DexEvent) {
    |event: DexEvent| match event {
        DexEvent::WhirlpoolTickArrayAccountEvent(e) => {
            let initialized_ticks = e
                .tick_array
                .ticks
                .iter()
                .filter(|tick| tick.initialized)
                .count();
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
