use solana_streamer_sdk::streaming::event_parser::protocols::whirlpool::parser::WHIRLPOOL_PROGRAM_ID;
use solana_streamer_sdk::streaming::event_parser::{DexEvent, Protocol};
use solana_streamer_sdk::streaming::{
    grpc::ClientConfig, yellowstone_grpc::TransactionFilter, YellowstoneGrpc,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("开始 Whirlpool Swap/SwapV2 事件订阅示例（带 Traded log 解析）...");
    subscribe_whirlpool_swaps().await?;
    Ok(())
}

async fn subscribe_whirlpool_swaps() -> Result<(), Box<dyn std::error::Error>> {
    println!("订阅 Whirlpool Swap/SwapV2 交易...");

    let mut config: ClientConfig = ClientConfig::default();
    config.enable_metrics = true;
    let grpc = YellowstoneGrpc::new_with_config(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        Some("3ec495919af1e20d458053d07565e8c785d10b17c0a33d7ed9e4e0a9df05b8ff".to_string()),
        config,
    )?;

    println!("gRPC 客户端创建成功");

    let callback = create_event_callback();
    let protocols = vec![Protocol::Whirlpool];

    let transaction_filter = TransactionFilter {
        account_include: vec![WHIRLPOOL_PROGRAM_ID.to_string()],
        account_exclude: vec![],
        account_required: vec![],
    };

    use solana_streamer_sdk::streaming::event_parser::common::filter::EventTypeFilter;
    use solana_streamer_sdk::streaming::event_parser::common::EventType;
    let event_type_filter = Some(EventTypeFilter {
        include: vec![EventType::WhirlpoolSwap, EventType::WhirlpoolSwapV2],
    });

    println!("开始监听事件，按 Ctrl+C 停止...");
    println!("监控程序: {}", WHIRLPOOL_PROGRAM_ID);

    grpc.subscribe_events_immediate(
        protocols,
        None,
        vec![transaction_filter],
        vec![],
        event_type_filter,
        None,
        callback,
    )
    .await?;

    println!("等待 Ctrl+C 停止...");
    tokio::signal::ctrl_c().await?;

    Ok(())
}

fn create_event_callback() -> impl Fn(DexEvent) {
    |event: DexEvent| match event {
        DexEvent::WhirlpoolSwapEvent(e) => {
            println!("=== Whirlpool Swap 事件 ===");
            println!("事件类型: {:?}", e.metadata.event_type);
            println!("交易签名: {}", e.metadata.signature);
            println!("Slot: {}", e.metadata.slot);
            println!("池子: {}", e.whirlpool);
            println!("指令 amount: {}", e.amount);
            println!("指令 other_amount_threshold: {}", e.other_amount_threshold);
            println!("指令 amount_specified_is_input: {}", e.amount_specified_is_input);
            println!("方向 a_to_b: {}", e.a_to_b);
            println!("pre_sqrt_price: {}", e.pre_sqrt_price);
            println!("post_sqrt_price: {}", e.post_sqrt_price);
            println!("input_amount: {}", e.input_amount);
            println!("output_amount: {}", e.output_amount);
            println!("input_transfer_fee: {}", e.input_transfer_fee);
            println!("output_transfer_fee: {}", e.output_transfer_fee);
            println!("lp_fee: {}", e.lp_fee);
            println!("protocol_fee: {}", e.protocol_fee);
            println!("=====================================\n");
        }
        DexEvent::WhirlpoolSwapV2Event(e) => {
            println!("=== Whirlpool SwapV2 事件 ===");
            println!("事件类型: {:?}", e.metadata.event_type);
            println!("交易签名: {}", e.metadata.signature);
            println!("Slot: {}", e.metadata.slot);
            println!("池子: {}", e.whirlpool);
            println!("Token Mint A: {}", e.token_mint_a);
            println!("Token Mint B: {}", e.token_mint_b);
            println!("指令 amount: {}", e.amount);
            println!("指令 other_amount_threshold: {}", e.other_amount_threshold);
            println!("指令 amount_specified_is_input: {}", e.amount_specified_is_input);
            println!("方向 a_to_b: {}", e.a_to_b);
            println!("pre_sqrt_price: {}", e.pre_sqrt_price);
            println!("post_sqrt_price: {}", e.post_sqrt_price);
            println!("input_amount: {}", e.input_amount);
            println!("output_amount: {}", e.output_amount);
            println!("input_transfer_fee: {}", e.input_transfer_fee);
            println!("output_transfer_fee: {}", e.output_transfer_fee);
            println!("lp_fee: {}", e.lp_fee);
            println!("protocol_fee: {}", e.protocol_fee);
            println!("=====================================\n");
        }
        _ => {}
    }
}
