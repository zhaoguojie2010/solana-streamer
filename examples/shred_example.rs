mod common;

use solana_streamer_sdk::streaming::{
    event_parser::{DexEvent, Protocol},
    shred::StreamClientConfig,
    ShredStreamGrpc,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    println!("Starting ShredStream Streamer...");
    test_shreds().await?;
    Ok(())
}

async fn test_shreds() -> Result<(), Box<dyn std::error::Error>> {
    println!("Subscribing to ShredStream events...");

    // Create low-latency configuration
    let mut config = StreamClientConfig::default();
    // Enable performance monitoring, has performance overhead, disabled by default
    config.enable_metrics = true;
    let shred_stream = ShredStreamGrpc::new_with_config(
        common::env_or_default("SHRED_ENDPOINT", "http://127.0.0.1:10800")?,
        config,
    )
    .await?;

    let callback = create_event_callback();
    let protocols = vec![
        Protocol::PumpFun,
        Protocol::PumpSwap,
        Protocol::Bonk,
        Protocol::RaydiumCpmm,
        Protocol::RaydiumClmm,
        Protocol::RaydiumAmmV4,
    ];

    // Event filtering
    // No event filtering, includes all events
    let event_type_filter = None;
    // Only include PumpSwapBuy events and PumpSwapSell events
    // let event_type_filter =
    //     EventTypeFilter { include: vec![EventType::PumpSwapBuy, EventType::PumpSwapSell] };

    println!("Listening for events, press Ctrl+C to stop...");
    shred_stream.shredstream_subscribe(protocols, None, event_type_filter, callback).await?;

    println!("Waiting for Ctrl+C to stop...");
    let shutdown = common::wait_for_shutdown(&shred_stream.subscription_handle).await;
    shred_stream.stop().await;
    shutdown?;

    Ok(())
}

fn create_event_callback() -> impl Fn(DexEvent) {
    |event: DexEvent| {
        println!(
            "🎉 Event received! Type: {:?}, transaction_index: {:?}",
            event.metadata().event_type,
            event.metadata().transaction_index
        );
        match event {
            DexEvent::BlockMetaEvent(e) => {
                println!("BlockMetaEvent: {:?}", e.metadata.handle_us);
            }
            _ => {}
        }
    }
}
