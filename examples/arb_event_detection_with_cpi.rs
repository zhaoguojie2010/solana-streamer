use solana_streamer_sdk::streaming::event_parser::{
    common::EventType,
    protocols::{
        bonk::parser::BONK_PROGRAM_ID, meteora_damm_v2::parser::METEORA_DAMM_V2_PROGRAM_ID,
        meteora_dlmm::parser::METEORA_DLMM_PROGRAM_ID, pumpfun::parser::PUMPFUN_PROGRAM_ID,
        pumpswap::parser::PUMPSWAP_PROGRAM_ID, raydium_amm_v4::parser::RAYDIUM_AMM_V4_PROGRAM_ID,
        raydium_clmm::parser::RAYDIUM_CLMM_PROGRAM_ID,
        raydium_cpmm::parser::RAYDIUM_CPMM_PROGRAM_ID, whirlpool::parser::WHIRLPOOL_PROGRAM_ID,
    },
    DexEvent, Protocol,
};
use solana_streamer_sdk::streaming::{
    grpc::ClientConfig,
    yellowstone_grpc::{AccountFilter, TransactionFilter},
    YellowstoneGrpc,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct InnerSwapLeg {
    inner_ix: i64,
    dex_program: String,
    pool_id: String,
    event_type: EventType,
    from_mint: String,
    to_mint: String,
    is_arb_leg: bool,
}

#[derive(Default)]
struct ArbTraceState {
    // (signature, outer_ix) -> outer program id
    outer_program_by_ix: HashMap<(String, i64), String>,
    // (signature, outer_ix) -> all inner swap legs
    swap_legs_by_ix: HashMap<(String, i64), Vec<InnerSwapLeg>>,
    // (signature, outer_ix) -> last printed leg count
    printed_group_leg_count: HashMap<(String, i64), usize>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting arb-event detection example...");
    subscribe_arb_events().await
}

async fn subscribe_arb_events() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ClientConfig::default();
    config.enable_metrics = true;

    let grpc = YellowstoneGrpc::new_with_config(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        Some("3ec495919af1e20d458053d07565e8c785d10b17c0a33d7ed9e4e0a9df05b8ff".to_string()),
        config,
    )?;

    let protocols = vec![
        Protocol::PumpFun,
        Protocol::PumpSwap,
        Protocol::Bonk,
        Protocol::RaydiumCpmm,
        Protocol::RaydiumClmm,
        Protocol::RaydiumAmmV4,
        Protocol::MeteoraDammV2,
        Protocol::MeteoraDlmm,
        Protocol::Whirlpool,
    ];

    let account_include = vec![
        PUMPFUN_PROGRAM_ID.to_string(),
        PUMPSWAP_PROGRAM_ID.to_string(),
        BONK_PROGRAM_ID.to_string(),
        RAYDIUM_CPMM_PROGRAM_ID.to_string(),
        RAYDIUM_CLMM_PROGRAM_ID.to_string(),
        RAYDIUM_AMM_V4_PROGRAM_ID.to_string(),
        METEORA_DAMM_V2_PROGRAM_ID.to_string(),
        METEORA_DLMM_PROGRAM_ID.to_string(),
        WHIRLPOOL_PROGRAM_ID.to_string(),
    ];

    let tx_filter = TransactionFilter {
        account_include: account_include.clone(),
        account_exclude: vec![],
        account_required: vec![],
    };
    let account_filter = AccountFilter { account: vec![], owner: account_include, filters: vec![] };

    let callback = create_arb_callback();

    println!("Subscribing... press Ctrl+C to stop.");
    grpc.subscribe_events_immediate(
        protocols,
        None,
        vec![tx_filter],
        vec![account_filter],
        None,
        None,
        callback,
    )
    .await?;

    tokio::signal::ctrl_c().await?;
    Ok(())
}

fn create_arb_callback() -> impl Fn(DexEvent) + Send + Sync + 'static {
    let state = Arc::new(Mutex::new(ArbTraceState::default()));

    move |event: DexEvent| {
        let metadata = event.metadata();
        let signature = metadata.signature.to_string();
        let key = (signature.clone(), metadata.outer_index);

        if metadata.inner_index.is_none() {
            if let Ok(mut guard) = state.lock() {
                guard.outer_program_by_ix.insert(key, metadata.program_id.to_string());
                if guard.outer_program_by_ix.len() > 20_000
                    || guard.swap_legs_by_ix.len() > 20_000
                    || guard.printed_group_leg_count.len() > 20_000
                {
                    guard.outer_program_by_ix.clear();
                    guard.swap_legs_by_ix.clear();
                    guard.printed_group_leg_count.clear();
                }
            }
            return;
        }

        let Some(pool_id) = extract_pool_id(&event) else {
            return;
        };
        let (from_mint, to_mint) = metadata
            .swap_data
            .as_ref()
            .map(|s| (s.from_mint.to_string(), s.to_mint.to_string()))
            .unwrap_or_else(|| ("UNKNOWN".to_string(), "UNKNOWN".to_string()));
        if !is_swap_event_type(&metadata.event_type) {
            return;
        }

        println!(
            "[INNER_SWAP] sig={} outer_ix={} inner_ix={} dex_program={} pool_id={} event_type={:?} route={} -> {} arb={}",
            signature,
            metadata.outer_index,
            metadata.inner_index.unwrap_or_default(),
            metadata.program_id,
            pool_id,
            metadata.event_type,
            from_mint,
            to_mint,
            metadata.is_arb_leg
        );

        let leg = InnerSwapLeg {
            inner_ix: metadata.inner_index.unwrap_or_default(),
            dex_program: metadata.program_id.to_string(),
            pool_id,
            event_type: metadata.event_type.clone(),
            from_mint: from_mint.clone(),
            to_mint: to_mint.clone(),
            is_arb_leg: metadata.is_arb_leg,
        };

        let mut should_print_group = false;
        let mut group_snapshot: Vec<InnerSwapLeg> = Vec::new();
        let mut entry_program = "UNKNOWN_OUTER_PROGRAM".to_string();

        if let Ok(mut guard) = state.lock() {
            if let Some(program) = guard.outer_program_by_ix.get(&key) {
                entry_program = program.clone();
            }

            let last_printed_len = guard.printed_group_leg_count.get(&key).copied().unwrap_or(0);
            let current_len = {
                let legs = guard.swap_legs_by_ix.entry(key.clone()).or_default();
                legs.push(leg);
                let current_len = legs.len();
                if current_len >= 2 && current_len > last_printed_len {
                    group_snapshot = legs.clone();
                }
                current_len
            };

            if current_len >= 2 && current_len > last_printed_len {
                should_print_group = true;
                guard.printed_group_leg_count.insert(key.clone(), current_len);
            }
        }

        if !should_print_group {
            return;
        }

        group_snapshot.sort_by_key(|item| item.inner_ix);
        let first_from = group_snapshot
            .first()
            .map(|x| x.from_mint.clone())
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let last_to = group_snapshot
            .last()
            .map(|x| x.to_mint.clone())
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let mut unique_pools: Vec<String> = Vec::new();
        let mut arb_leg_count = 0usize;
        for leg in group_snapshot.iter() {
            if !unique_pools.iter().any(|pool_id| pool_id == &leg.pool_id) {
                unique_pools.push(leg.pool_id.clone());
            }
            if leg.is_arb_leg {
                arb_leg_count += 1;
            }
        }

        println!("=== INNER SWAP GROUP ===");
        println!("signature: {}", signature);
        println!("entry_program: {}", entry_program);
        println!("outer_ix: {}", metadata.outer_index);
        println!("hops: {}", group_snapshot.len());
        println!("unique_pool_count: {}", unique_pools.len());
        println!("arb_leg_count: {}", arb_leg_count);
        println!("group_route: {} -> {}", first_from, last_to);
        println!("legs:");
        for leg in group_snapshot.iter() {
            println!(
                "  - inner_ix={} dex_program={} pool_id={} event_type={:?} route={} -> {} arb={}",
                leg.inner_ix,
                leg.dex_program,
                leg.pool_id,
                leg.event_type,
                leg.from_mint,
                leg.to_mint,
                leg.is_arb_leg
            );
        }
        println!("========================\n");
    }
}

#[inline]
fn is_swap_event_type(event_type: &EventType) -> bool {
    matches!(
        event_type,
        EventType::PumpSwapBuy
            | EventType::PumpSwapSell
            | EventType::PumpFunBuy
            | EventType::PumpFunSell
            | EventType::BonkBuyExactIn
            | EventType::BonkBuyExactOut
            | EventType::BonkSellExactIn
            | EventType::BonkSellExactOut
            | EventType::RaydiumCpmmSwapBaseInput
            | EventType::RaydiumCpmmSwapBaseOutput
            | EventType::RaydiumClmmSwap
            | EventType::RaydiumClmmSwapV2
            | EventType::RaydiumAmmV4SwapBaseIn
            | EventType::RaydiumAmmV4SwapBaseOut
            | EventType::MeteoraDammV2Swap
            | EventType::MeteoraDammV2Swap2
            | EventType::MeteoraDlmmSwap
            | EventType::MeteoraDlmmSwap2
            | EventType::WhirlpoolSwap
            | EventType::WhirlpoolSwapV2
    )
}

#[inline]
fn extract_pool_id(event: &DexEvent) -> Option<String> {
    let pool = match event {
        DexEvent::PumpSwapBuyEvent(e) => e.pool,
        DexEvent::PumpSwapSellEvent(e) => e.pool,
        DexEvent::PumpFunTradeEvent(e) => e.bonding_curve,
        DexEvent::BonkTradeEvent(e) => e.pool_state,
        DexEvent::RaydiumCpmmSwapEvent(e) => e.pool_state,
        DexEvent::RaydiumClmmSwapEvent(e) => e.pool_state,
        DexEvent::RaydiumClmmSwapV2Event(e) => e.pool_state,
        DexEvent::RaydiumAmmV4SwapEvent(e) => e.amm,
        DexEvent::MeteoraDammV2SwapEvent(e) => e.pool,
        DexEvent::MeteoraDammV2Swap2Event(e) => e.pool,
        DexEvent::MeteoraDlmmSwapEvent(e) => e.lb_pair,
        DexEvent::MeteoraDlmmSwap2Event(e) => e.lb_pair,
        DexEvent::WhirlpoolSwapEvent(e) => e.whirlpool,
        DexEvent::WhirlpoolSwapV2Event(e) => e.whirlpool,
        _ => return None,
    };
    Some(pool.to_string())
}
