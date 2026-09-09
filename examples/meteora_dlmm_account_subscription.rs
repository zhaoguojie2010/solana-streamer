use solana_streamer_sdk::streaming::{
    event_parser::{ParseOptions, ParsePlan},
    yellowstone_grpc::{StreamEvent, SubscriptionRequest},
};
mod common;

use solana_streamer_sdk::streaming::{
    event_parser::{
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
    println!("开始 Meteora DLMM 账户数据订阅示例...");
    subscribe_meteora_dlmm_accounts().await?;
    Ok(())
}

async fn subscribe_meteora_dlmm_accounts() -> Result<(), Box<dyn std::error::Error>> {
    println!("订阅 Meteora DLMM 账户数据...");

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

    // 账户过滤器 - 只订阅 LbPair 和 BinArrayBitmapExtension，不订阅 BinArray
    // 使用 Memcmp 过滤器在 gRPC 层面过滤，只匹配 LbPair 和 BinArrayBitmapExtension 的 discriminator
    // 这样可以减小 gRPC streaming 压力，避免接收 BinArray 账户

    // 创建 LbPair 账户过滤器
    let lb_pair_filter = AccountFilter {
        account: vec![],
        owner: vec![METEORA_DLMM_PROGRAM_ID.to_string()],
        filters: vec![SubscribeRequestFilterAccountsFilter {
            filter: Some(Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp {
                // discriminator 在账户数据的前 8 字节
                offset: 0,
                data: Some(Data::Bytes(discriminators::LB_PAIR.to_vec())),
            })),
        }],
        cuckoo_accounts_filter: None,
    };

    // 创建 BinArrayBitmapExtension 账户过滤器
    let bin_array_bitmap_extension_filter = AccountFilter {
        account: vec![],
        owner: vec![METEORA_DLMM_PROGRAM_ID.to_string()],
        filters: vec![SubscribeRequestFilterAccountsFilter {
            filter: Some(Filter::Memcmp(SubscribeRequestFilterAccountsFilterMemcmp {
                // discriminator 在账户数据的前 8 字节
                offset: 0,
                data: Some(Data::Bytes(discriminators::BIN_ARRAY_BITMAP_EXTENSION.to_vec())),
            })),
        }],
        cuckoo_accounts_filter: None,
    };

    // 事件类型过滤器 - 只订阅账户事件
    use solana_streamer_sdk::streaming::event_parser::common::filter::EventTypeFilter;
    use solana_streamer_sdk::streaming::event_parser::common::EventType;
    let event_type_filter = Some(EventTypeFilter {
        include: vec![
            EventType::AccountMeteoraDlmmLbPair,
            EventType::AccountMeteoraDlmmBinArrayBitmapExtension,
        ],
    });

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
    request.accounts = vec![lb_pair_filter, bin_array_bitmap_extension_filter];
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
            // println!(
            //     "🎉 事件接收! 类型: {:?}, slot: {:?}",
            //     event.metadata().event_type,
            //     event.metadata().slot
            // );
            match event {
                AccountEvent::MeteoraDlmmLbPairAccountEvent(e) => {
                    println!("=== Meteora DLMM LbPair 账户更新 ===");
                    println!("账户地址: {}", e.pubkey);
                    println!("Token X Mint: {}", e.lb_pair.token_x_mint);
                    println!("Token Y Mint: {}", e.lb_pair.token_y_mint);
                    println!("Active ID: {}", e.lb_pair.active_id);
                    println!("Bin Step: {}", e.lb_pair.bin_step);
                    println!("Status: {}", e.lb_pair.status);
                    println!("Reserve X: {}", e.lb_pair.reserve_x);
                    println!("Reserve Y: {}", e.lb_pair.reserve_y);
                    println!("Protocol Fee X: {}", e.lb_pair.protocol_fee.amount_x);
                    println!("Protocol Fee Y: {}", e.lb_pair.protocol_fee.amount_y);
                    println!("Last Updated At: {}", e.lb_pair.last_updated_at);
                    println!("=====================================");
                }
                AccountEvent::MeteoraDlmmBinArrayBitmapExtensionAccountEvent(e) => {
                    println!("=== Meteora DLMM BinArrayBitmapExtension 账户更新 ===");
                    println!("账户地址: {}", e.pubkey);
                    println!(
                        "Bin Array Bitmap: {:?}",
                        e.bin_array_bitmap_extension.positive_bin_array_bitmap
                    );
                    println!("=====================================");
                }
                _ => {
                    //println!("其他事件: {:?}", event);
                }
            }
        }
    }
}
