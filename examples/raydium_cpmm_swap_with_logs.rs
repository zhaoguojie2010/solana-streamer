use solana_streamer_sdk::streaming::event_parser::protocols::raydium_cpmm::parser::RAYDIUM_CPMM_PROGRAM_ID;
use solana_streamer_sdk::streaming::event_parser::{DexEvent, Protocol};
use solana_streamer_sdk::streaming::{
    grpc::ClientConfig,
    yellowstone_grpc::TransactionFilter,
    YellowstoneGrpc,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("开始 Raydium CPMM Swap 事件订阅示例（带日志解析）...");
    subscribe_raydium_cpmm_swaps().await?;
    Ok(())
}

async fn subscribe_raydium_cpmm_swaps() -> Result<(), Box<dyn std::error::Error>> {
    println!("订阅 Raydium CPMM Swap 交易...");

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

    // 只订阅 Raydium CPMM 协议
    let protocols = vec![Protocol::RaydiumCpmm];

    println!("监控协议: {:?}", protocols);

    // 交易过滤器
    let transaction_filter = TransactionFilter {
        account_include: vec![RAYDIUM_CPMM_PROGRAM_ID.to_string()],
        account_exclude: vec![],
        account_required: vec![],
    };

    // 事件类型过滤器 - 只订阅 Swap 事件
    use solana_streamer_sdk::streaming::event_parser::common::filter::EventTypeFilter;
    use solana_streamer_sdk::streaming::event_parser::common::EventType;
    let event_type_filter = Some(EventTypeFilter {
        include: vec![
            EventType::RaydiumCpmmSwapBaseInput,
            EventType::RaydiumCpmmSwapBaseOutput,
        ],
    });

    println!("开始监听事件，按 Ctrl+C 停止...");
    println!("监控程序: {}", RAYDIUM_CPMM_PROGRAM_ID);

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
    |event: DexEvent| {
        match event {
            DexEvent::RaydiumCpmmSwapEvent(e) => {
                println!("=== Raydium CPMM Swap 事件 ===");
                println!("事件类型: {:?}", e.metadata.event_type);
                println!("交易签名: {}", e.metadata.signature);
                println!("Slot: {}", e.metadata.slot);
                println!();
                
                // 指令参数
                println!("--- 指令参数 ---");
                if e.amount_in > 0 {
                    println!("输入金额: {}", e.amount_in);
                    println!("最小输出: {}", e.minimum_amount_out);
                } else {
                    println!("最大输入: {}", e.max_amount_in);
                    println!("输出金额: {}", e.amount_out);
                }
                println!();
                
                // 从日志解析的事件数据
                if e.input_amount > 0 || e.output_amount > 0 {
                    println!("--- 从日志解析的实际数据 ---");
                    println!("输入 Vault 之前余额: {}", e.input_vault_before);
                    println!("输出 Vault 之前余额: {}", e.output_vault_before);
                    println!("实际输入金额: {}", e.input_amount);
                    println!("实际输出金额: {}", e.output_amount);
                    println!("输入转账费: {}", e.input_transfer_fee);
                    println!("输出转账费: {}", e.output_transfer_fee);
                    println!("交易费: {}", e.trade_fee);
                    println!("创建者费用: {}", e.creator_fee);
                    println!("基于输入: {}", e.base_input);
                    println!();
                }
                
                // 账户信息
                println!("--- 账户信息 ---");
                println!("付款人: {}", e.payer);
                println!("池状态: {}", e.pool_state);
                println!("输入代币: {}", e.input_token_mint);
                println!("输出代币: {}", e.output_token_mint);
                println!("输入 Vault: {}", e.input_vault);
                println!("输出 Vault: {}", e.output_vault);
                println!("=====================================\n");
            }
            _ => {
                // 其他事件
            }
        }
    }
}
