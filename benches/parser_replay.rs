//! Offline, release-mode replay. Fixture construction is excluded from timings.
use serde::Deserialize;
use serde_json::json;
use solana_sdk::pubkey::Pubkey;
use solana_streamer_sdk::streaming::event_parser::{
    common::EventMetadata, AccountEvent, ParseOptions, ParsePlan, Protocol, TxEvent, TxFrame,
    TxMetadata, TxParser,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
    time::Instant,
};
use yellowstone_grpc_proto::prelude::{CompiledInstruction, InnerInstruction, InnerInstructions};

#[derive(Clone, Copy, Default)]
struct AllocationCount {
    calls: u64,
    bytes: u64,
}
thread_local! { static COUNT: Cell<Option<AllocationCount>> = const {Cell::new(None)}; }
struct Allocator;
fn allocated(bytes: usize) {
    let _ = COUNT.try_with(|c| {
        if let Some(mut v) = c.get() {
            v.calls += 1;
            v.bytes += bytes as u64;
            c.set(Some(v));
        }
    });
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        allocated(l.size());
        System.alloc(l)
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        allocated(l.size());
        System.alloc_zeroed(l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        allocated(n);
        System.realloc(p, l, n)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
#[derive(Deserialize)]
struct Case {
    program: [u8; 32],
    keys: Vec<[u8; 32]>,
    discriminator: Vec<u8>,
    data: Vec<u8>,
}
fn fixtures() -> Vec<TxFrame> {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../tests/fixtures/protocol_baseline.json")).unwrap();
    cases
        .into_iter()
        .map(|case| {
            let mut keys: Vec<_> = case.keys.into_iter().map(Pubkey::new_from_array).collect();
            let count = keys.len();
            keys.push(Pubkey::new_from_array(case.program));
            TxFrame::new(
                TxMetadata::default(),
                keys,
                vec![CompiledInstruction {
                    program_id_index: count as u32,
                    accounts: (0..count as u8).collect(),
                    data: [case.discriminator, case.data].concat(),
                }],
                vec![],
                vec![],
            )
            .unwrap()
        })
        .collect()
}
fn cpi_fixture(count: usize) -> TxFrame {
    use solana_streamer_sdk::streaming::event_parser::protocols::pancakeswap::{
        discriminators, parser::PANCAKESWAP_PROGRAM_ID,
    };
    let mut keys: Vec<_> = (0..=13).map(|i| Pubkey::new_from_array([i; 32])).collect();
    keys.push(PANCAKESWAP_PROGRAM_ID);
    TxFrame::new(
        TxMetadata::default(),
        keys,
        vec![CompiledInstruction { program_id_index: 0, ..CompiledInstruction::default() }],
        vec![InnerInstructions {
            index: 0,
            instructions: (0..count)
                .map(|_| InnerInstruction {
                    program_id_index: 14,
                    accounts: (1..14).collect(),
                    data: [discriminators::SWAP_V2, &[0; 33]].concat(),
                    stack_height: Some(2),
                })
                .collect(),
        }],
        vec![],
    )
    .unwrap()
}
fn report(
    name: &str,
    mut timings: Vec<u128>,
    allocations: AllocationCount,
    events: usize,
) -> serde_json::Value {
    timings.sort_unstable();
    let n = timings.len();
    let sum: u128 = timings.iter().sum();
    json!({"case":name,"frames":n,"events":events,"p50_ns":timings[n/2],"p95_ns":timings[(n-1)*95/100],"p99_ns":timings[(n-1)*99/100],
        "mean_ns":sum/n as u128,"allocations_per_frame":allocations.calls as f64/n as f64,"allocated_bytes_per_frame":allocations.bytes as f64/n as f64})
}
fn borrowed(name: &str, frames: &[TxFrame], plan: &ParsePlan, n: usize) -> serde_json::Value {
    let mut parser = TxParser::default();
    for frame in frames {
        parser
            .visit(frame, plan, |v| {
                black_box(v);
            })
            .unwrap();
    }
    let mut timings = Vec::with_capacity(n);
    let mut events = 0;
    for i in 0..n {
        let start = Instant::now();
        parser
            .visit(&frames[i % frames.len()], plan, |v| {
                events += v.events.len();
                black_box(v);
            })
            .unwrap();
        timings.push(start.elapsed().as_nanos());
    }
    // Count allocations in a separate pass so allocator bookkeeping is excluded from timings.
    COUNT.with(|c| c.set(Some(AllocationCount::default())));
    for i in 0..n {
        parser
            .visit(&frames[i % frames.len()], plan, |v| {
                black_box(v);
            })
            .unwrap();
    }
    let allocations = COUNT.with(|c| c.take().unwrap());
    report(name, timings, allocations, events)
}

// Owned parsing consumes its input. Fixture reconstruction is outside both measurements.
fn copy_input(frame: &TxFrame) -> TxFrame {
    let mut outer = Vec::new();
    let mut inner: Vec<InnerInstructions> = Vec::new();
    for ix in frame.instructions() {
        let program_id_index = frame.keys().iter().position(|k| k == ix.program_id).unwrap() as u32;
        if ix.inner_index.is_some() {
            if inner.last().is_none_or(|g| g.index != ix.outer_index) {
                inner.push(InnerInstructions { index: ix.outer_index, instructions: Vec::new() });
            }
            inner.last_mut().unwrap().instructions.push(InnerInstruction {
                program_id_index,
                accounts: ix.account_indices.to_vec(),
                data: ix.data.to_vec(),
                stack_height: ix.stack_height,
            });
        } else {
            outer.push(CompiledInstruction {
                program_id_index,
                accounts: ix.account_indices.to_vec(),
                data: ix.data.to_vec(),
            });
        }
    }
    TxFrame::new(
        frame.metadata().clone(),
        frame.keys().to_vec(),
        outer,
        inner,
        frame.logs().to_vec(),
    )
    .unwrap()
}

fn owned(name: &str, frames: &[TxFrame], plan: &ParsePlan, n: usize) -> serde_json::Value {
    let mut parser = TxParser::default();
    for frame in frames {
        black_box(parser.parse_owned(copy_input(frame), plan).unwrap());
    }
    let mut timings = Vec::with_capacity(n);
    let mut events = 0;
    for i in 0..n {
        let frame = copy_input(&frames[i % frames.len()]);
        let start = Instant::now();
        let batch = parser.parse_owned(frame, plan).unwrap();
        timings.push(start.elapsed().as_nanos());
        events += batch.as_ref().map_or(0, |b| b.events.len());
        black_box(batch); // Output destruction is outside the parse timing.
    }
    let mut allocations = AllocationCount::default();
    for i in 0..n {
        let frame = copy_input(&frames[i % frames.len()]);
        COUNT.with(|c| c.set(Some(allocations)));
        let batch = parser.parse_owned(frame, plan).unwrap();
        allocations = COUNT.with(|c| c.take().unwrap());
        black_box(batch);
    }
    report(name, timings, allocations, events)
}
fn main() {
    let n = std::env::var("REPLAY_ITERS")
        .ok()
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(10_000)
        .max(100);
    let protocols = [
        Protocol::PancakeSwap,
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
    let frames = fixtures();
    let plan = ParsePlan::all(&protocols);
    let excluded = ParsePlan::new(&protocols, Some(&[]), ParseOptions::default());
    let cpi_frames = [cpi_fixture(20)];
    let results = vec![
        borrowed("protocol_mix_borrowed", &frames, &plan, n),
        borrowed("20_cpi_borrowed", &cpi_frames, &plan, n),
        borrowed("protocol_mix_excluded", &frames, &excluded, n),
        owned("protocol_mix_owned", &frames, &plan, n),
        owned("20_cpi_owned", &cpi_frames, &plan, n),
    ];
    println!("{}",serde_json::to_string_pretty(&json!({"profile":"release; normalized inputs; warmed parser; timing and allocation passes separate",
        "layout_bytes":{"TxEvent":size_of::<TxEvent>(),"AccountEvent":size_of::<AccountEvent>(),"EventMetadata":size_of::<EventMetadata>()},"results":results})).unwrap());
}
