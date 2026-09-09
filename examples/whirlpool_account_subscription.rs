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
    // 初始化日志系统，设置日志级别为 debug 以便查看详细信息
    //env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    println!("开始 Whirlpool 账户数据订阅示例...");
    subscribe_whirlpool_accounts().await?;
    Ok(())
}

async fn subscribe_whirlpool_accounts() -> Result<(), Box<dyn std::error::Error>> {
    println!("订阅 Whirlpool 账户数据...");

    // 创建客户端配置
    let mut config: ClientConfig = ClientConfig::default();
    // 启用性能监控（可选，有性能开销）
    config.enable_metrics = true;
    let grpc =
        YellowstoneGrpc::new_with_config(common::grpc_endpoint()?, common::grpc_token()?, config)?;

    println!("gRPC 客户端创建成功");

    let callback = create_event_callback();

    // 只订阅 Whirlpool 协议
    let protocols = vec![Protocol::Whirlpool];

    println!("监控协议: {:?}", protocols);

    // 账户过滤器 - 订阅 Whirlpool 程序拥有的账户
    let account_filter = AccountFilter {
        account: vec![],
        owner: vec![WHIRLPOOL_PROGRAM_ID.to_string()],
        filters: vec![],
        cuckoo_accounts_filter: None,
    };

    // 交易过滤器（可选，如果只想订阅账户数据，可以留空）
    let transaction_filter = TransactionFilter {
        account_include: vec![WHIRLPOOL_PROGRAM_ID.to_string()],
        account_exclude: vec![],
        account_required: vec![],
    };

    // 事件类型过滤器 - 只订阅账户事件
    use solana_streamer_sdk::streaming::event_parser::common::filter::EventTypeFilter;
    use solana_streamer_sdk::streaming::event_parser::common::EventType;
    let event_type_filter = Some(EventTypeFilter { include: vec![EventType::AccountWhirlpool] });

    println!("开始监听事件，按 Ctrl+C 停止...");
    println!("监控程序: {}", WHIRLPOOL_PROGRAM_ID);

    println!("开始订阅...");

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
    |event: DexEvent| {
        // println!(
        //     "🎉 事件接收! 类型: {:?}, slot: {:?}",
        //     event.metadata().event_type,
        //     event.metadata().slot
        // );
        match event {
            DexEvent::WhirlpoolAccountEvent(e) => {
                println!("=== Whirlpool 账户更新 ===");
                println!("账户地址: {}", e.pubkey);
                println!("Whirlpools Config: {}", e.whirlpool.whirlpools_config);
                println!("Token Mint A: {}", e.whirlpool.token_mint_a);
                println!("Token Mint B: {}", e.whirlpool.token_mint_b);
                println!("Token Vault A: {}", e.whirlpool.token_vault_a);
                println!("Token Vault B: {}", e.whirlpool.token_vault_b);
                println!("Tick Spacing: {}", e.whirlpool.tick_spacing);
                println!("Fee Rate: {}", e.whirlpool.fee_rate);
                println!("Protocol Fee Rate: {}", e.whirlpool.protocol_fee_rate);
                println!("Liquidity: {}", e.whirlpool.liquidity);
                println!("Sqrt Price: {}", e.whirlpool.sqrt_price);
                println!("Tick Current Index: {}", e.whirlpool.tick_current_index);
                println!("Protocol Fee Owed A: {}", e.whirlpool.protocol_fee_owed_a);
                println!("Protocol Fee Owed B: {}", e.whirlpool.protocol_fee_owed_b);
                println!("Fee Growth Global A: {}", e.whirlpool.fee_growth_global_a);
                println!("Fee Growth Global B: {}", e.whirlpool.fee_growth_global_b);
                println!(
                    "Reward Last Updated Timestamp: {}",
                    e.whirlpool.reward_last_updated_timestamp
                );
                println!("奖励信息数量: {}", e.whirlpool.reward_infos.len());
                for (i, reward_info) in e.whirlpool.reward_infos.iter().enumerate() {
                    if reward_info.mint != solana_sdk::pubkey::Pubkey::default() {
                        println!(
                            "  奖励 {}: Mint={}, Vault={}, Authority={}, Emissions={}, Growth={}",
                            i,
                            reward_info.mint,
                            reward_info.vault,
                            reward_info.authority,
                            reward_info.emissions_per_second_x64,
                            reward_info.growth_global_x64
                        );
                    }
                }
                println!("=====================================");
            }
            _ => {
                //println!("其他事件: {:?}", event);
            }
        }
    }
}
