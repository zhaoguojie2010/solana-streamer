use solana_streamer_sdk::streaming::{
    event_parser::{ParseOptions, ParsePlan},
    yellowstone_grpc::{StreamEvent, SubscriptionRequest},
};
mod common;

use solana_streamer_sdk::streaming::{
    event_parser::{
        common::{filter::EventTypeFilter, EventType},
        protocols::meteora_dlmm::{events::discriminators, parser::METEORA_DLMM_PROGRAM_ID},
        AccountEvent, Protocol,
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
    println!("开始 Meteora DLMM BinArray 账户数据订阅示例...");
    subscribe_meteora_dlmm_bin_array_accounts().await?;
    Ok(())
}

async fn subscribe_meteora_dlmm_bin_array_accounts() -> Result<(), Box<dyn std::error::Error>> {
    println!("订阅 Meteora DLMM BinArray 账户数据...");

    // 创建客户端配置
    let mut config: ClientConfig = ClientConfig::default();
    // 启用性能监控（可选，有性能开销）
    config.enable_metrics = true;
    let grpc =
        YellowstoneGrpc::new_with_config(common::grpc_endpoint()?, common::grpc_token()?, config)?;

    println!("gRPC 客户端创建成功");

    let callback = create_event_callback();

    // 只订阅 Meteora DLMM 协议
    let protocols = vec![Protocol::MeteoraDlmm];

    println!("监控协议: {:?}", protocols);

    // 账户过滤器 - 只订阅 BinArray 账户
    // 使用 Memcmp 过滤器在 gRPC 层面过滤，只匹配 BinArray 的 discriminator
    // 这样可以减小 gRPC streaming 压力，避免接收 LbPair 和 BinArrayBitmapExtension 账户
    let account_filter = AccountFilter {
        account: vec![],
        owner: vec![METEORA_DLMM_PROGRAM_ID.to_string()],
        filters: vec![SubscribeRequestFilterAccountsFilter {
            filter: Some(Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp {
                // discriminator 在账户数据的前 8 字节
                offset: 0,
                data: Some(Data::Bytes(discriminators::BIN_ARRAY.to_vec())),
            })),
        }],
        cuckoo_accounts_filter: None,
    };

    // 交易过滤器（可选，如果只想订阅账户数据，可以留空）

    // 事件类型过滤器 - 只订阅 BinArray 账户事件
    let event_type_filter =
        Some(EventTypeFilter { include: vec![EventType::AccountMeteoraDlmmBinArray] });

    println!("开始监听事件，按 Ctrl+C 停止...");
    println!("监控程序: {}", METEORA_DLMM_PROGRAM_ID);

    println!("开始订阅...");

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
        {
            match event {
                AccountEvent::MeteoraDlmmBinArrayAccountEvent(e) => {
                    println!("=== Meteora DLMM BinArray 账户更新 ===");
                    println!("账户地址: {}", e.pubkey);
                    println!("BinArray Index: {}", e.bin_array.index);
                    println!("版本: {}", e.bin_array.version);
                    println!("关联 LbPair: {}", e.bin_array.lb_pair);

                    // 统计非空的 bin 数量
                    let non_empty_bins: usize = e
                        .bin_array
                        .bins
                        .iter()
                        .filter(|bin| {
                            bin.amount_x > 0 || bin.amount_y > 0 || bin.liquidity_supply > 0
                        })
                        .count();

                    println!("非空 Bin 数量: {}/70", non_empty_bins);

                    // 显示前几个非空 bin 的信息
                    let mut shown = 0;
                    for (idx, bin) in e.bin_array.bins.iter().enumerate() {
                        if (bin.amount_x > 0 || bin.amount_y > 0 || bin.liquidity_supply > 0)
                            && shown < 5
                        {
                            println!(
                                "  Bin[{}]: X={}, Y={}, Price={}, Liquidity={}",
                                idx, bin.amount_x, bin.amount_y, bin.price, bin.liquidity_supply
                            );
                            shown += 1;
                        }
                    }

                    println!("Slot: {}", view.frame.slot);
                    println!("=====================================");
                }
                _ => {
                    // 忽略其他事件
                }
            }
        }
    }
}
