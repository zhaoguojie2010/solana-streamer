mod support;
use support::*;

use solana_sdk::pubkey::Pubkey;
use solana_streamer_sdk::streaming::{
    event_parser::{
        common::{EventType, SwapCuInstructionMatcher, SwapCuParseConfig, SwapCuTarget},
        protocols::whirlpool::{parser::WHIRLPOOL_PROGRAM_ID, WhirlpoolInstructionKind},
        AccountEvent, ParseOptions, ParsePlan, Protocol, TxEvent, TxExecutionStatus, TxFrame,
        TxParser,
    },
    grpc::{AccountFrame, BlockMetaPretty, TransactionPretty},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
use yellowstone_grpc_proto::prelude::*;
// Count only allocations made by the current test thread inside an explicit scope.
// Other tests and the test runner can allocate concurrently without skewing measurements.
#[derive(Clone, Copy, Debug, Default)]
struct Allocations {
    calls: usize,
    bytes: usize,
}

thread_local! {
    static ALLOCATIONS: Cell<Option<Allocations>> = const { Cell::new(None) };
}

struct CountingAllocator;

fn record_allocation(bytes: usize) {
    let _ = ALLOCATIONS.try_with(|counter| {
        if let Some(mut allocations) = counter.get() {
            allocations.calls += 1;
            allocations.bytes += bytes;
            counter.set(Some(allocations));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record_allocation(new_size);
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn measure<T>(operation: impl FnOnce() -> T) -> (T, Allocations) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ALLOCATIONS.with(|counter| counter.set(None));
        }
    }
    ALLOCATIONS.with(|counter| assert!(counter.replace(Some(Allocations::default())).is_none()));
    let _reset = Reset;
    let result = operation();
    let allocations = ALLOCATIONS.with(|counter| counter.get().unwrap());
    (result, allocations)
}

#[test]
fn batch_preserves_order_common_metadata_raw_buffers_and_execution_audit() {
    let info = fixture();
    let raw_pointer =
        info.transaction.as_ref().unwrap().message.as_ref().unwrap().instructions[1].data.as_ptr();
    let frame = frame(info);
    assert_eq!(frame.instruction(1).unwrap().data.as_ptr(), raw_pointer);
    let mut parser = TxParser::default();
    let plan = plan(ParseOptions {
        retain_instructions: true,
        balance_audit: true,
        compute_budget: true,
        detect_jito: true,
        ..ParseOptions::default()
    });
    let batch = parser.parse_owned(frame, &plan).unwrap().unwrap();
    assert_eq!(batch.meta.signature, signature());
    assert_eq!(batch.meta.slot, 123);
    assert_eq!(batch.meta.transaction_index, Some(9));
    assert_eq!(batch.meta.block_time_ms, 1_700_000_000_123);
    assert_eq!(batch.meta.execution_status, TxExecutionStatus::Success);
    assert_eq!(
        batch
            .events
            .iter()
            .map(|e| (e.metadata().outer_index, e.metadata().inner_index))
            .collect::<Vec<_>>(),
        [(0, None), (1, None), (1, Some(0)), (1, Some(2)), (2, None)]
    );
    assert_eq!(batch.summary.compute_unit_limit, Some(200_000));
    assert_eq!(batch.summary.compute_unit_price, Some(42));
    assert_eq!(batch.summary.is_jito, Some(true));
    assert_eq!(batch.instruction(1).unwrap().data.as_ptr(), raw_pointer);
    for (event, marker) in batch.events[1..4].iter().zip([11, 22, 33]) {
        let TxEvent::WhirlpoolInstructionEvent(e) = event else { panic!("wrong event") };
        assert_eq!(e.kind, WhirlpoolInstructionKind::InitializePool);
        let raw = batch.instruction(e.metadata.instruction_index).unwrap();
        assert_eq!(raw.data[8..], [marker, marker + 1]);
        assert_eq!(raw.accounts[2], Pubkey::new_from_array([1; 32]));
    }
    let raw = batch.instruction(1).unwrap();
    assert_eq!(raw.account_indices, [5, 4, 0]);
    assert_eq!(raw.accounts[0], Pubkey::new_from_array([3; 32]));
    let audit = batch.audit.unwrap();
    assert!(audit.parse_errors.is_empty());
    assert_eq!(audit.token_balance_changes.len(), 1);
    assert_eq!(audit.token_balance_changes[0].account, Some(Pubkey::new_from_array([2; 32])));
    assert_eq!(audit.token_balance_changes[0].pre_amount, Some(10));
    assert_eq!(audit.token_balance_changes[0].post_amount, Some(20));
}

#[test]
fn borrowed_and_owned_paths_match_without_cloning_the_source() {
    let input = frame(fixture());
    let plan = plan(ParseOptions::default());
    let mut parser = TxParser::default();
    let mut copy = None;
    parser
        .visit(&input, &plan, |view| {
            assert_eq!(view.keys.as_ptr(), input.keys().as_ptr());
            assert_eq!(
                view.instruction(1).unwrap().data.as_ptr(),
                input.instruction(1).unwrap().data.as_ptr()
            );
            copy = Some(view.to_owned());
        })
        .unwrap();
    let owned = parser.parse_owned(input, &plan).unwrap();
    assert_eq!(owned, copy);
}

#[test]
fn versioned_and_grpc_adapters_share_order_and_source_semantics() {
    use solana_streamer_sdk::streaming::event_parser::TxMetadata;
    let (tx, keys, meta) = versioned_fixture();
    let frame = TxFrame::from_versioned(
        tx,
        keys,
        meta.inner_instructions,
        vec![],
        TxMetadata {
            slot: 123,
            block_time: 1_700_000_000,
            block_time_ms: 1_700_000_000_123,
            transaction_index: Some(9),
            ..TxMetadata::default()
        },
    )
    .unwrap();
    let mut parser = TxParser::default();
    let versioned = parser.parse_owned(frame, &plan(ParseOptions::default())).unwrap().unwrap();
    let grpc = parser
        .parse_owned(crate::frame(fixture()), &plan(ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(versioned.meta.execution_status, TxExecutionStatus::Unknown);
    assert_eq!(versioned.events, grpc.events);
    assert_eq!(versioned.keys, grpc.keys);
    for (a, b) in versioned.view().instructions().zip(grpc.view().instructions()) {
        assert_eq!(a.data, b.data);
        assert_eq!(a.account_indices, b.account_indices);
    }
}

#[test]
fn warm_borrowed_parser_allocates_nothing_for_modeled_events_or_unmatched_cpi() {
    let mut input = fixture();
    input.meta.as_mut().unwrap().inner_instructions[0].instructions.extend((0..100).map(|_| {
        InnerInstruction {
            program_id_index: 1,
            accounts: vec![0],
            data: vec![0; 64],
            stack_height: Some(2),
        }
    }));
    let input = frame(input);
    let mut parser = TxParser::default();
    let plan = plan(ParseOptions::default());
    parser.visit(&input, &plan, |_| {}).unwrap();
    let (result, allocations) =
        measure(|| parser.visit(&input, &plan, |batch| assert_eq!(batch.events.len(), 5)));
    result.unwrap();
    assert_eq!(allocations.calls, 0, "{allocations:?}");
}

#[test]
fn owned_batches_allocate_once_after_learning_output_size() {
    let mut parser = TxParser::default();
    let plan = plan(ParseOptions::default());
    let held = parser.parse_owned(frame(fixture()), &plan).unwrap().unwrap();
    let input = frame(fixture());
    let (output, allocations) = measure(|| parser.parse_owned(input, &plan).unwrap().unwrap());
    assert_eq!(output.events, held.events);
    assert_ne!(output.events.as_ptr(), held.events.as_ptr());
    assert_eq!(allocations.calls, 1);
    assert_eq!(allocations.bytes, output.events.len() * size_of::<TxEvent>());
}

#[test]
fn excluded_events_are_skipped_before_allocating() {
    let input = frame(fixture());
    let plan = ParsePlan::new(&[Protocol::Whirlpool], Some(&[]), ParseOptions::default());
    let mut parser = TxParser::default();
    let (result, allocations) =
        measure(|| parser.visit(&input, &plan, |_| panic!("filtered batch")));
    result.unwrap();
    assert_eq!(allocations.calls, 0);
}

#[test]
fn invalid_keys_and_instruction_indices_are_errors() {
    let mut input = fixture();
    input.meta.as_mut().unwrap().loaded_writable_addresses[0].pop();
    assert!(TxFrame::try_from(TransactionPretty {
        grpc_tx: input,
        ..TransactionPretty::default()
    })
    .is_err());
    for program in [false, true] {
        let mut input = fixture();
        let ix = &mut input.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[1];
        if program {
            ix.program_id_index = 255;
        } else {
            ix.accounts[0] = 255;
        }
        assert!(TxFrame::try_from(TransactionPretty {
            grpc_tx: input,
            ..TransactionPretty::default()
        })
        .is_err());
    }
    let mut input = fixture();
    let group = input.meta.as_ref().unwrap().inner_instructions[0].clone();
    input.meta.as_mut().unwrap().inner_instructions.push(group);
    assert!(TxFrame::try_from(TransactionPretty {
        grpc_tx: input,
        ..TransactionPretty::default()
    })
    .is_err());
}

#[test]
fn account_table_has_one_allocation_for_static_and_loaded_keys() {
    let input = SubscribeUpdateTransactionInfo {
        signature: signature().as_ref().to_vec(),
        transaction: Some(Transaction {
            message: Some(Message { account_keys: vec![vec![1; 32]; 32], ..Message::default() }),
            ..Transaction::default()
        }),
        meta: Some(TransactionStatusMeta {
            loaded_writable_addresses: vec![vec![2; 32]; 8],
            loaded_readonly_addresses: vec![vec![3; 32]; 8],
            ..TransactionStatusMeta::default()
        }),
        ..SubscribeUpdateTransactionInfo::default()
    };
    let (_, allocations) = measure(|| frame(input));
    assert_eq!(allocations.calls, 1, "{allocations:?}");
    assert_eq!(allocations.bytes, 48 * size_of::<Pubkey>());
}

#[test]
fn wrappers_move_payloads_without_allocations() {
    // Initialize the shared clock outside the allocation scope.
    solana_streamer_sdk::streaming::event_parser::common::high_performance_clock::get_high_perf_clock();
    let update = SubscribeUpdateAccount {
        slot: 12,
        is_startup: true,
        account: Some(SubscribeUpdateAccountInfo {
            pubkey: vec![1; 32],
            owner: vec![2; 32],
            data: vec![9; 1024].into(),
            txn_signature: Some(signature().as_ref().to_vec()),
            write_version: 34,
            executable: true,
            lamports: 56,
            rent_epoch: 78,
        }),
    };
    let pointer = update.account.as_ref().unwrap().data.as_ptr();
    let (account, allocations) = measure(|| AccountFrame::try_from(update).unwrap());
    assert_eq!(allocations.calls, 0);
    assert_eq!(account.data.as_ptr(), pointer);
    assert_eq!((account.slot, account.write_version, account.lamports), (12, 34, 56));
    let block = SubscribeUpdateBlockMeta {
        slot: 12,
        blockhash: "blockhash".into(),
        ..SubscribeUpdateBlockMeta::default()
    };
    let pointer = block.blockhash.as_ptr();
    let (block, allocations) = measure(|| BlockMetaPretty::from((block, BLOCK_TIME)));
    assert_eq!(allocations.calls, 0);
    assert_eq!(block.block_hash.as_ptr(), pointer);
    let update = SubscribeUpdateTransaction { transaction: Some(fixture()), slot: 123 };
    let pointer = update.transaction.as_ref().unwrap().signature.as_ptr();
    let (tx, allocations) = measure(|| TransactionPretty::try_from((update, BLOCK_TIME)).unwrap());
    assert_eq!(allocations.calls, 0);
    assert_eq!(tx.grpc_tx.signature.as_ptr(), pointer);
}

#[test]
fn nested_cu_belongs_to_the_exact_invocation() {
    let mut transaction = fixture();
    let logs = &mut transaction.meta.as_mut().unwrap().log_messages;
    *logs = vec![
        format!("Program {COMPUTE_BUDGET} invoke [1]"),
        format!("Program {COMPUTE_BUDGET} success"),
        format!("Program {WHIRLPOOL_PROGRAM_ID} invoke [1]"),
        format!("Program {WHIRLPOOL_PROGRAM_ID} invoke [2]"),
        format!("Program {WHIRLPOOL_PROGRAM_ID} consumed 101 of 200000 compute units"),
        format!("Program {WHIRLPOOL_PROGRAM_ID} success"),
        format!("Program {} invoke [2]", Pubkey::default()),
        format!("Program {} success", Pubkey::default()),
        format!("Program {WHIRLPOOL_PROGRAM_ID} invoke [2]"),
        format!("Program {WHIRLPOOL_PROGRAM_ID} consumed 202 of 200000 compute units"),
        format!("Program {WHIRLPOOL_PROGRAM_ID} success"),
        format!("Program {WHIRLPOOL_PROGRAM_ID} consumed 404 of 200000 compute units"),
        format!("Program {WHIRLPOOL_PROGRAM_ID} success"),
        format!("Program {COMPUTE_BUDGET} invoke [1]"),
        format!("Program {COMPUTE_BUDGET} success"),
    ];
    let config = SwapCuParseConfig {
        enabled: true,
        targets: vec![SwapCuTarget {
            protocol: Protocol::Whirlpool,
            program_id: WHIRLPOOL_PROGRAM_ID,
            matcher: SwapCuInstructionMatcher::Discriminator8(vec![&INITIALIZE_POOL]),
        }],
    };

    let mut parser = TxParser::default();
    let batch = parser
        .parse_owned(
            frame(transaction),
            &plan(ParseOptions { swap_cu: config, ..ParseOptions::default() }),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        batch.events.iter().map(|e| e.metadata().swap_compute_units).collect::<Vec<_>>(),
        [None, Some(404), Some(101), Some(202), None]
    );
}

#[test]
fn account_filter_precedes_snapshot_decode_and_snapshots_share_bytes() {
    use solana_streamer_sdk::streaming::event_parser::protocols::pumpfun::{
        discriminators, parser::PUMPFUN_PROGRAM_ID, types::BONDING_CURVE_SIZE,
    };
    let mut bytes = vec![0; 8 + BONDING_CURVE_SIZE];
    bytes[..8].copy_from_slice(discriminators::BONDING_CURVE_ACCOUNT);
    bytes[8..16].copy_from_slice(&123u64.to_le_bytes());
    let account = AccountFrame {
        data: bytes.into(),
        owner: PUMPFUN_PROGRAM_ID,
        slot: 999,
        write_version: 123,
        ..AccountFrame::default()
    };
    let excluded = ParsePlan::new(
        &[Protocol::PumpFun],
        Some(&[EventType::AccountPumpFunGlobal]),
        ParseOptions::default(),
    );
    let (_, allocations) = measure(|| assert!(account.view(&excluded).is_none()));
    assert_eq!(allocations.calls, 0);
    let plan = ParsePlan::all(&[Protocol::PumpFun]);
    let view = account.view(&plan).unwrap();
    assert_eq!(view.u64_at(8), Some(123));
    assert_eq!(view.u64_at(usize::MAX), None);
    assert_eq!(view.u128_at(usize::MAX), None);
    assert_eq!(view.pubkey_at(usize::MAX), None);
    let AccountEvent::PumpFunBondingCurveAccountEvent(snapshot) = view.decode().unwrap() else {
        panic!("wrong snapshot")
    };
    assert_eq!(snapshot.raw_account_data.as_ptr(), account.data.as_ptr());
    assert_eq!(snapshot.bonding_curve.virtual_token_reserves, 123);
    drop(account);
    assert_eq!(snapshot.raw_account_data[8..16], 123u64.to_le_bytes());
}

fn positioned_log(marker: u8) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let data = [
        &[237, 175, 243, 230, 147, 117, 101, 121][..],
        &[marker; 32],
        &[marker + 1; 32],
        &1i32.to_le_bytes(),
        &2i32.to_le_bytes(),
    ]
    .concat();
    format!("Program data: {}", STANDARD.encode(data))
}
fn logged_fixture() -> SubscribeUpdateTransactionInfo {
    let mut tx = fixture();
    tx.meta.as_mut().unwrap().log_messages = vec![
        format!("Program {WHIRLPOOL_PROGRAM_ID} invoke [1]"),
        positioned_log(1),
        format!("Program {WHIRLPOOL_PROGRAM_ID} invoke [2]"),
        positioned_log(2),
        format!("Program {WHIRLPOOL_PROGRAM_ID} consumed 101 of 200000 compute units"),
        format!("Program {WHIRLPOOL_PROGRAM_ID} success"),
        format!("Program {} invoke [2]", Pubkey::default()),
        format!("Program {} success", Pubkey::default()),
        format!("Program {WHIRLPOOL_PROGRAM_ID} invoke [2]"),
        positioned_log(3),
        format!("Program {WHIRLPOOL_PROGRAM_ID} consumed 202 of 200000 compute units"),
        format!("Program {WHIRLPOOL_PROGRAM_ID} success"),
        positioned_log(4),
        format!("Program {WHIRLPOOL_PROGRAM_ID} consumed 404 of 200000 compute units"),
        format!("Program {WHIRLPOOL_PROGRAM_ID} success"),
    ];
    tx
}
fn log_plan() -> ParsePlan {
    plan(ParseOptions {
        enrich_logs: true,
        swap_cu: SwapCuParseConfig {
            enabled: true,
            targets: vec![SwapCuTarget {
                protocol: Protocol::Whirlpool,
                program_id: WHIRLPOOL_PROGRAM_ID,
                matcher: SwapCuInstructionMatcher::Discriminator8(vec![&INITIALIZE_POOL]),
            }],
        },
        ..ParseOptions::default()
    })
}
fn execution_markers(event: &TxEvent) -> Vec<u8> {
    use solana_streamer_sdk::streaming::event_parser::protocols::whirlpool::WhirlpoolExecutionEvent;
    let TxEvent::WhirlpoolInstructionEvent(event) = event else { return vec![] };
    event
        .execution_events
        .iter()
        .map(|event| match event {
            WhirlpoolExecutionEvent::PositionOpened { whirlpool, .. } => whirlpool.to_bytes()[0],
            _ => panic!("wrong execution event"),
        })
        .collect()
}
#[test]
fn one_log_index_handles_reentry_all_data_items_and_omitted_builtin_logs() {
    let batch =
        TxParser::default().parse_owned(frame(logged_fixture()), &log_plan()).unwrap().unwrap();
    assert_eq!(execution_markers(&batch.events[1]), [1, 4]);
    assert_eq!(execution_markers(&batch.events[2]), [2]);
    assert_eq!(execution_markers(&batch.events[3]), [3]);
    assert_eq!(
        batch.events.iter().map(|e| e.metadata().swap_compute_units).collect::<Vec<_>>(),
        [None, Some(404), Some(101), Some(202), None]
    );
}
#[test]
fn truncated_or_misaligned_logs_never_supply_sibling_observations() {
    let mut tx = logged_fixture();
    tx.meta.as_mut().unwrap().log_messages.truncate(13);
    tx.meta.as_mut().unwrap().log_messages.push("Log truncated".into());
    let batch = TxParser::default().parse_owned(frame(tx), &log_plan()).unwrap().unwrap();
    assert!(execution_markers(&batch.events[1]).is_empty());
    assert_eq!(batch.events[1].metadata().swap_compute_units, None);
    assert_eq!(execution_markers(&batch.events[2]), [2]); // completed child is still evidence
    let mut tx = logged_fixture();
    tx.meta.as_mut().unwrap().log_messages.remove(5); // missing child exit before sibling invoke
    let batch = TxParser::default().parse_owned(frame(tx), &log_plan()).unwrap().unwrap();
    assert!(batch.events.iter().all(|e| execution_markers(e).is_empty()));
    assert!(batch.events.iter().all(|e| e.metadata().swap_compute_units.is_none()));
}
#[test]
fn disabled_log_work_ignores_bad_base64_and_keeps_source_optional() {
    let batch = TxParser::default()
        .parse_owned(frame(logged_fixture()), &plan(ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert!(batch.events.iter().all(|e| execution_markers(e).is_empty()));
    assert!(batch.events.iter().all(|e| e.metadata().swap_compute_units.is_none()));
    assert!(batch.logs.is_none());
}
#[test]
fn filtered_create_dependencies_are_transaction_local_and_ordered() {
    use solana_streamer_sdk::streaming::event_parser::{
        protocols::pumpfun::{discriminators, parser::PUMPFUN_PROGRAM_ID},
        TxMetadata,
    };
    let mut keys: Vec<_> = (1..=16).map(|i| Pubkey::new_from_array([i; 32])).collect();
    keys.push(PUMPFUN_PROGRAM_ID);
    let create = CompiledInstruction {
        program_id_index: 16,
        accounts: (0..16).collect(),
        data: [discriminators::CREATE_TOKEN_IX, &[0; 512]].concat(),
    };
    let mut buy = CompiledInstruction {
        program_id_index: 16,
        accounts: (0..16).collect(),
        data: [discriminators::BUY_IX, &[0; 16]].concat(),
    };
    buy.accounts[6] = create.accounts[7];
    let plan = ParsePlan::new(
        &[Protocol::PumpFun],
        Some(&[EventType::PumpFunBuy]),
        ParseOptions::default(),
    );
    let mut parser = TxParser::default();
    for (instructions, expect_dev) in [
        (vec![create.clone(), buy.clone()], true),
        (vec![buy.clone()], false),
        (vec![buy, create], false),
    ] {
        let input = TxFrame::new(TxMetadata::default(), keys.clone(), instructions, vec![], vec![])
            .unwrap();
        let batch = parser.parse_owned(input, &plan).unwrap().unwrap();
        assert_eq!(batch.events.len(), 1);
        let TxEvent::PumpFunTradeEvent(event) = &batch.events[0] else { panic!("wrong event") };
        assert_eq!(event.is_dev_create_token_trade, expect_dev);
    }
}
#[test]
fn compute_summary_is_available_without_compute_events() {
    let plan = ParsePlan::new(
        &[Protocol::Whirlpool],
        Some(&[EventType::WhirlpoolInstruction]),
        ParseOptions { compute_budget: true, ..ParseOptions::default() },
    );
    let batch = TxParser::default().parse_owned(frame(fixture()), &plan).unwrap().unwrap();
    assert_eq!(batch.events.len(), 3);
    assert_eq!(batch.summary.compute_unit_limit, Some(200_000));
    assert_eq!(batch.summary.compute_unit_price, Some(42));
}

#[test]
fn cpi_merging_checks_program_and_stops_at_sibling_calls() {
    use solana_streamer_sdk::streaming::event_parser::{
        protocols::pumpfun::{discriminators, parser::PUMPFUN_PROGRAM_ID},
        TxMetadata,
    };
    let mut keys: Vec<_> = (1..=16).map(|i| Pubkey::new_from_array([i; 32])).collect();
    keys.push(Pubkey::default());
    keys.push(PUMPFUN_PROGRAM_ID);
    let buy = || InnerInstruction {
        program_id_index: 17,
        accounts: (0..16).collect(),
        data: [discriminators::BUY_IX, &[0; 16]].concat(),
        stack_height: Some(2),
    };
    let cpi = |amount: u64, program_id_index| {
        let mut data = vec![0; 250];
        data[32..40].copy_from_slice(&amount.to_le_bytes());
        data[40..48].copy_from_slice(&456u64.to_le_bytes());
        data[48] = 1;
        InnerInstruction {
            program_id_index,
            accounts: vec![],
            data: [discriminators::TRADE_EVENT, data.as_slice()].concat(),
            stack_height: Some(3),
        }
    };
    let plan = ParsePlan::new(
        &[Protocol::PumpFun],
        Some(&[EventType::PumpFunBuy]),
        ParseOptions::default(),
    );
    for has_first in [true, false] {
        let mut inner = vec![buy(), cpi(999, 16)]; // matching bytes from a different program must be ignored
        if has_first {
            inner.push(cpi(111, 17));
        }
        inner.extend([buy(), cpi(222, 17)]);
        let input = TxFrame::new(
            TxMetadata::default(),
            keys.clone(),
            vec![CompiledInstruction { program_id_index: 16, ..CompiledInstruction::default() }],
            vec![InnerInstructions { index: 0, instructions: inner }],
            vec![],
        )
        .unwrap();
        let batch = TxParser::default().parse_owned(input, &plan).unwrap().unwrap();
        let amounts: Vec<_> = batch
            .events
            .iter()
            .map(|e| match e {
                TxEvent::PumpFunTradeEvent(e) => e.sol_amount,
                _ => panic!("wrong event"),
            })
            .collect();
        assert_eq!(amounts, [if has_first { 111 } else { 0 }, 222]);
    }
}

#[test]
fn failed_execution_and_bad_balances_are_preserved_for_audit() {
    let mut info = fixture();
    let meta = info.meta.as_mut().unwrap();
    meta.err = Some(TransactionError { err: vec![1] });
    meta.pre_token_balances[0].ui_token_amount.as_mut().unwrap().amount = "bad amount".into();
    let batch = TxParser::default()
        .parse_owned(
            frame(info),
            &plan(ParseOptions { balance_audit: true, ..ParseOptions::default() }),
        )
        .unwrap()
        .unwrap();
    assert_eq!(batch.meta.execution_status, TxExecutionStatus::Failed);
    let audit = batch.audit.unwrap();
    assert_eq!(audit.parse_errors.len(), 1);
    assert_eq!(audit.token_balance_changes[0].pre_amount, None);
    assert_eq!(audit.token_balance_changes[0].post_amount, Some(20));
}

#[test]
fn fully_filtered_transactions_do_not_build_even_enabled_log_indexes() {
    let input = frame(logged_fixture());
    let plan = ParsePlan::new(
        &[Protocol::Whirlpool],
        Some(&[]),
        ParseOptions {
            enrich_logs: true,
            swap_cu: SwapCuParseConfig::default_enabled(),
            ..ParseOptions::default()
        },
    );
    let mut parser = TxParser::default();
    let (result, allocations) =
        measure(|| parser.visit(&input, &plan, |_| panic!("filtered output")));
    result.unwrap();
    assert_eq!(allocations.calls, 0);
}

#[test]
fn summary_only_batches_and_serialized_sources_remain_usable() {
    let plan = ParsePlan::new(
        &[],
        Some(&[]),
        ParseOptions {
            compute_budget: true,
            detect_jito: true,
            retain_instructions: true,
            ..ParseOptions::default()
        },
    );
    let batch = TxParser::default().parse_owned(frame(fixture()), &plan).unwrap().unwrap();
    assert!(batch.events.is_empty());
    assert_eq!(batch.summary.compute_unit_price, Some(42));
    assert_eq!(batch.summary.is_jito, Some(true));
    let restored: solana_streamer_sdk::streaming::event_parser::TxBatch =
        serde_json::from_value(serde_json::to_value(&batch).unwrap()).unwrap();
    assert_eq!(batch, restored);
    assert_eq!(restored.instruction(1).unwrap().account_indices, [5, 4, 0]);
    let summary = ParsePlan::new(
        &[],
        Some(&[]),
        ParseOptions { compute_budget: true, ..ParseOptions::default() },
    );
    assert!(TxParser::default()
        .parse_owned(frame(fixture()), &summary)
        .unwrap()
        .unwrap()
        .events
        .is_empty());
}

#[test]
fn parent_swap_does_not_merge_a_reentrant_child_swaps_result() {
    use solana_streamer_sdk::streaming::event_parser::{
        protocols::pumpfun::{discriminators, parser::PUMPFUN_PROGRAM_ID},
        TxMetadata,
    };
    let mut keys: Vec<_> = (1..=16).map(|i| Pubkey::new_from_array([i; 32])).collect();
    keys.push(PUMPFUN_PROGRAM_ID);
    let buy = [discriminators::BUY_IX, &[0; 16]].concat();
    let event = |amount: u64, height| {
        let mut body = vec![0; 250];
        body[32..40].copy_from_slice(&amount.to_le_bytes());
        body[48] = 1;
        InnerInstruction {
            program_id_index: 16,
            accounts: vec![],
            data: [discriminators::TRADE_EVENT, body.as_slice()].concat(),
            stack_height: Some(height),
        }
    };
    let input = TxFrame::new(
        TxMetadata::default(),
        keys,
        vec![CompiledInstruction {
            program_id_index: 16,
            accounts: (0..16).collect(),
            data: buy.clone(),
        }],
        vec![InnerInstructions {
            index: 0,
            instructions: vec![
                InnerInstruction {
                    program_id_index: 16,
                    accounts: (0..16).collect(),
                    data: buy,
                    stack_height: Some(2),
                },
                event(222, 3),
                event(111, 2),
            ],
        }],
        vec![],
    )
    .unwrap();
    let batch = TxParser::default()
        .parse_owned(input, &ParsePlan::all(&[Protocol::PumpFun]))
        .unwrap()
        .unwrap();
    let amounts: Vec<_> = batch
        .events
        .iter()
        .map(|e| match e {
            TxEvent::PumpFunTradeEvent(e) => e.sol_amount,
            _ => panic!("wrong event"),
        })
        .collect();
    assert_eq!(amounts, [111, 222]);
}
