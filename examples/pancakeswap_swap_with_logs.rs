mod common;

use solana_streamer_sdk::streaming::event_parser::protocols::pancakeswap::parser::PANCAKESWAP_PROGRAM_ID;
use solana_streamer_sdk::streaming::event_parser::{DexEvent, Protocol};
use solana_streamer_sdk::streaming::{
    grpc::ClientConfig, yellowstone_grpc::TransactionFilter, YellowstoneGrpc,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    println!("开始 PancakeSwap Swap/SwapV2 事件订阅示例（指令解析）...");
    subscribe_pancakeswap_swaps().await?;
    Ok(())
}

async fn subscribe_pancakeswap_swaps() -> Result<(), Box<dyn std::error::Error>> {
    println!("订阅 PancakeSwap Swap/SwapV2 交易...");

    let mut config: ClientConfig = ClientConfig::default();
    config.enable_metrics = true;
    let grpc =
        YellowstoneGrpc::new_with_config(common::grpc_endpoint()?, common::grpc_token()?, config)?;

    println!("gRPC 客户端创建成功");

    let callback = create_event_callback();
    let protocols = vec![Protocol::PancakeSwap];

    let transaction_filter = TransactionFilter {
        account_include: vec![PANCAKESWAP_PROGRAM_ID.to_string()],
        account_exclude: vec![],
        account_required: vec![],
    };

    use solana_streamer_sdk::streaming::event_parser::common::filter::EventTypeFilter;
    use solana_streamer_sdk::streaming::event_parser::common::EventType;
    let event_type_filter = Some(EventTypeFilter {
        include: vec![EventType::PancakeSwapSwap, EventType::PancakeSwapSwapV2],
    });

    println!("开始监听事件，按 Ctrl+C 停止...");
    println!("监控程序: {}", PANCAKESWAP_PROGRAM_ID);

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
    let shutdown = common::wait_for_shutdown(&grpc.subscription_handle).await;
    grpc.stop().await;
    shutdown?;

    Ok(())
}

fn create_event_callback() -> impl Fn(DexEvent) {
    |event: DexEvent| match event {
        /*
        DexEvent::PancakeSwapSwapEvent(e) => {
            println!("=== PancakeSwap Swap 事件 ===");
            println!("事件类型: {:?}", e.metadata.event_type);
            println!("交易签名: {}", e.metadata.signature);
            println!("Slot: {}", e.metadata.slot);
            println!("池子: {}", e.pool_state);
            println!("amount: {}", e.amount);
            println!("other_amount_threshold: {}", e.other_amount_threshold);
            println!("sqrt_price_limit: {}", e.sqrt_price_limit);
            println!("is_base_input: {}", e.is_base_input);
            println!("input_token_account: {}", e.input_token_account);
            println!("output_token_account: {}", e.output_token_account);
            println!("input_vault: {}", e.input_vault);
            println!("output_vault: {}", e.output_vault);
            println!("amount_0(log): {}", e.amount_0);
            println!("amount_1(log): {}", e.amount_1);
            println!("transfer_fee_0(log): {}", e.transfer_fee_0);
            println!("transfer_fee_1(log): {}", e.transfer_fee_1);
            println!("zero_for_one(log): {}", e.zero_for_one);
            println!("sqrt_price_x64(log): {}", e.sqrt_price_x64);
            println!("liquidity(log): {}", e.liquidity);
            println!("tick(log): {}", e.tick);
            println!("remaining_accounts: {}", e.remaining_accounts.len());
            println!("=====================================\n");
        }*/
        DexEvent::PancakeSwapSwapV2Event(e) => {
            println!("=== PancakeSwap SwapV2 事件 ===");
            println!("事件类型: {:?}", e.metadata.event_type);
            println!("交易签名: {}", e.metadata.signature);
            println!("Slot: {}", e.metadata.slot);
            println!("池子: {}", e.pool_state);
            println!("amount: {}", e.amount);
            println!("other_amount_threshold: {}", e.other_amount_threshold);
            println!("sqrt_price_limit: {}", e.sqrt_price_limit);
            println!("is_base_input: {}", e.is_base_input);
            println!("input_token_account: {}", e.input_token_account);
            println!("output_token_account: {}", e.output_token_account);
            println!("input_vault: {}", e.input_vault);
            println!("output_vault: {}", e.output_vault);
            println!("input_mint: {}", e.input_mint);
            println!("output_mint: {}", e.output_mint);
            println!("amount_0(log): {}", e.amount_0);
            println!("amount_1(log): {}", e.amount_1);
            println!("transfer_fee_0(log): {}", e.transfer_fee_0);
            println!("transfer_fee_1(log): {}", e.transfer_fee_1);
            println!("zero_for_one(log): {}", e.zero_for_one);
            println!("sqrt_price_x64(log): {}", e.sqrt_price_x64);
            println!("liquidity(log): {}", e.liquidity);
            println!("tick(log): {}", e.tick);
            println!("remaining_accounts: {}", e.remaining_accounts.len());
            println!("=====================================\n");
        }
        _ => {}
    }
}
