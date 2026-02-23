use solana_streamer_sdk::streaming::event_parser::protocols::meteora_dlmm::parser::METEORA_DLMM_PROGRAM_ID;
use solana_streamer_sdk::streaming::event_parser::{DexEvent, Protocol};
use solana_streamer_sdk::streaming::{
    grpc::ClientConfig, yellowstone_grpc::TransactionFilter, YellowstoneGrpc,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("开始 Meteora DLMM Swap/Swap2 事件订阅示例（含 CPI 日志解析）...");
    subscribe_meteora_dlmm_swaps().await?;
    Ok(())
}

async fn subscribe_meteora_dlmm_swaps() -> Result<(), Box<dyn std::error::Error>> {
    println!("订阅 Meteora DLMM Swap/Swap2 交易...");

    // 创建客户端配置
    let mut config: ClientConfig = ClientConfig::default();
    config.enable_metrics = true;
    let grpc = YellowstoneGrpc::new_with_config(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        Some("3ec495919af1e20d458053d07565e8c785d10b17c0a33d7ed9e4e0a9df05b8ff".to_string()),
        config,
    )?;

    println!("gRPC 客户端创建成功");

    let callback = create_event_callback();

    // 只订阅 Meteora DLMM 协议
    let protocols = vec![Protocol::MeteoraDlmm];

    println!("监控协议: {:?}", protocols);

    // 交易过滤器
    let transaction_filter = TransactionFilter {
        account_include: vec![METEORA_DLMM_PROGRAM_ID.to_string()],
        account_exclude: vec![],
        account_required: vec![],
    };

    // 事件类型过滤器 - 只订阅 Swap/Swap2 事件
    use solana_streamer_sdk::streaming::event_parser::common::filter::EventTypeFilter;
    use solana_streamer_sdk::streaming::event_parser::common::EventType;
    let event_type_filter = Some(EventTypeFilter {
        include: vec![EventType::MeteoraDlmmSwap, EventType::MeteoraDlmmSwap2],
    });

    println!("开始监听事件，按 Ctrl+C 停止...");
    println!("监控程序: {}", METEORA_DLMM_PROGRAM_ID);

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
        DexEvent::MeteoraDlmmSwapEvent(e) => {
            println!("=== Meteora DLMM Swap 事件 ===");
            println!("事件类型: {:?}", e.metadata.event_type);
            println!("交易签名: {}", e.metadata.signature);
            println!("Slot: {}", e.metadata.slot);
            println!();

            // 指令参数
            println!("--- 指令参数 ---");
            println!("amount_in: {}", e.amount_in);
            println!("min_amount_out: {}", e.min_amount_out);
            println!();

            // CPI 事件数据
            println!("--- CPI 日志解析 ---");
            println!("lb_pair: {}", e.lb_pair);
            println!("from: {}", e.from);
            println!("start_bin_id: {}", e.start_bin_id);
            println!("end_bin_id: {}", e.end_bin_id);
            println!("实际 amount_in: {}", e.cpi_amount_in);
            println!("实际 amount_out: {}", e.cpi_amount_out);
            println!("swap_for_y: {}", e.swap_for_y);
            println!("fee: {}", e.fee);
            println!("protocol_fee: {}", e.protocol_fee);
            println!("fee_bps: {}", e.fee_bps);
            println!("host_fee: {}", e.host_fee);
            println!();

            // 账户信息
            println!("--- 账户信息 ---");
            println!("user: {}", e.user);
            println!("token_x_mint: {:?}", e.token_x_mint);
            println!("token_y_mint: {:?}", e.token_y_mint);
            println!("user_token_in: {:?}", e.user_token_in);
            println!("user_token_out: {:?}", e.user_token_out);
            println!("reserve_x: {:?}", e.reserve_x);
            println!("reserve_y: {:?}", e.reserve_y);
            println!("oracle: {:?}", e.oracle);
            println!("=====================================\n");
        }
        DexEvent::MeteoraDlmmSwap2Event(e) => {
            println!("=== Meteora DLMM Swap2 事件 ===");
            println!("事件类型: {:?}", e.metadata.event_type);
            println!("交易签名: {}", e.metadata.signature);
            println!("Slot: {}", e.metadata.slot);
            println!();

            // 指令参数
            println!("--- 指令参数 ---");
            println!("amount_in: {}", e.amount_in);
            println!("min_amount_out: {}", e.min_amount_out);
            println!();

            // CPI 事件数据
            println!("--- CPI 日志解析 ---");
            println!("lb_pair: {}", e.lb_pair);
            println!("from: {}", e.from);
            println!("start_bin_id: {}", e.start_bin_id);
            println!("end_bin_id: {}", e.end_bin_id);
            println!("swap_for_y: {}", e.swap_for_y);
            println!("fee_bps: {}", e.fee_bps);
            println!("swap_result.amount_in: {}", e.swap_result.amount_in);
            println!("swap_result.amount_left: {}", e.swap_result.amount_left);
            println!("swap_result.amount_out: {}", e.swap_result.amount_out);
            println!("swap_result.total_fee: {}", e.swap_result.total_fee);
            println!("swap_result.lp_mm_fee: {}", e.swap_result.lp_mm_fee);
            println!("swap_result.protocol_fee: {}", e.swap_result.protocol_fee);
            println!("swap_result.host_fee: {}", e.swap_result.host_fee);
            println!("swap_result.lp_limit_order_fee: {}", e.swap_result.lp_limit_order_fee);
            println!(
                "swap_result.limit_order_filled_amount: {}",
                e.swap_result.limit_order_filled_amount
            );
            println!(
                "swap_result.limit_order_swapped_amount: {}",
                e.swap_result.limit_order_swapped_amount
            );
            println!();

            // 账户信息
            println!("--- 账户信息 ---");
            println!("user: {}", e.user);
            println!("token_x_mint: {:?}", e.token_x_mint);
            println!("token_y_mint: {:?}", e.token_y_mint);
            println!("user_token_in: {:?}", e.user_token_in);
            println!("user_token_out: {:?}", e.user_token_out);
            println!("reserve_x: {:?}", e.reserve_x);
            println!("reserve_y: {:?}", e.reserve_y);
            println!("oracle: {:?}", e.oracle);
            println!("=====================================\n");
        }
        _ => {
            // 其他事件
        }
    }
}
