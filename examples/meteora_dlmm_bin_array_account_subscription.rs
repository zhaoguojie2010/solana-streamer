use solana_streamer_sdk::streaming::{
    event_parser::{
        common::{EventType, filter::EventTypeFilter},
        protocols::meteora_dlmm::{
            events::discriminators,
            parser::METEORA_DLMM_PROGRAM_ID,
        },
        DexEvent, Protocol,
    },
    grpc::ClientConfig,
    yellowstone_grpc::{AccountFilter, TransactionFilter},
    YellowstoneGrpc,
};
use yellowstone_grpc_proto::geyser::{
    subscribe_request_filter_accounts_filter::Filter,
    subscribe_request_filter_accounts_filter_memcmp::Data, SubscribeRequestFilterAccountsFilter,
    SubscribeRequestFilterAccountsFilterMemcmp,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let grpc = YellowstoneGrpc::new_with_config(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
        config,
    )?;

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
    };

    // 交易过滤器（可选，如果只想订阅账户数据，可以留空）
    let transaction_filter = TransactionFilter {
        account_include: vec![METEORA_DLMM_PROGRAM_ID.to_string()],
        account_exclude: vec![],
        account_required: vec![],
    };

    // 事件类型过滤器 - 只订阅 BinArray 账户事件
    let event_type_filter = Some(EventTypeFilter {
        include: vec![
            EventType::AccountMeteoraDlmmBinArray,
        ],
    });

    println!("开始监听事件，按 Ctrl+C 停止...");
    println!("监控程序: {}", METEORA_DLMM_PROGRAM_ID);

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

    // 支持 stop 方法，测试代码 - 异步1000秒之后停止
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
    |event: DexEvent| {
        match event {
            DexEvent::MeteoraDlmmBinArrayAccountEvent(e) => {
                /*
                println!("=== Meteora DLMM BinArray 账户更新 ===");
                println!("账户地址: {}", e.pubkey);
                println!("BinArray Index: {}", e.bin_array.index);
                println!("版本: {}", e.bin_array.version);
                println!("关联 LbPair: {}", e.bin_array.lb_pair);
                
                // 统计非空的 bin 数量
                let non_empty_bins: usize = e.bin_array.bins.iter()
                    .filter(|bin| bin.amount_x > 0 || bin.amount_y > 0 || bin.liquidity_supply > 0)
                    .count();
                
                println!("非空 Bin 数量: {}/70", non_empty_bins);
                
                // 显示前几个非空 bin 的信息
                let mut shown = 0;
                for (idx, bin) in e.bin_array.bins.iter().enumerate() {
                    if (bin.amount_x > 0 || bin.amount_y > 0 || bin.liquidity_supply > 0) && shown < 5 {
                        println!("  Bin[{}]: X={}, Y={}, Price={}, Liquidity={}", 
                            idx, 
                            bin.amount_x, 
                            bin.amount_y, 
                            bin.price, 
                            bin.liquidity_supply
                        );
                        shown += 1;
                    }
                }
                
                println!("Slot: {}", e.metadata.slot);
                println!("=====================================");
                */
            }
            _ => {
                // 忽略其他事件
            }
        }
    }
}
