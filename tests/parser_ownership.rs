use solana_sdk::{
    message::{compiled_instruction::CompiledInstruction as SdkInstruction, v0, VersionedMessage},
    pubkey::Pubkey,
    signature::Signature,
    transaction::VersionedTransaction,
};
use solana_streamer_sdk::streaming::{
    event_parser::{
        common::{SwapCuInstructionMatcher, SwapCuParseConfig, SwapCuTarget},
        core::event_parser::EventParser,
        protocols::whirlpool::{parser::WHIRLPOOL_PROGRAM_ID, WhirlpoolInstructionKind},
        DexEvent, Protocol, TxExecutionStatus,
    },
    grpc::{
        pool::{factory, AccountPrettyPool, BlockMetaPrettyPool, TransactionPrettyPool},
        AccountPretty, BlockMetaPretty, TransactionPretty,
    },
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    future::Future,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};
use yellowstone_grpc_proto::{prelude::*, prost_types::Timestamp};

const COMPUTE_BUDGET: Pubkey = solana_sdk::pubkey!("ComputeBudget111111111111111111111111111111");
const JITO_TIP: Pubkey = solana_sdk::pubkey!("96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5");
const INITIALIZE_POOL: [u8; 8] = [95, 180, 10, 172, 84, 174, 232, 40];
fn signature() -> Signature {
    Signature::from([7; 64])
}
const BLOCK_TIME: Option<Timestamp> =
    Some(Timestamp { seconds: 1_700_000_000, nanos: 123_000_000 });

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

// Parsing does no I/O. Polling directly keeps runtime initialization out of the measurements.
fn parse_ready<F: Future>(future: F) -> F::Output {
    let mut context = Context::from_waker(Waker::noop());
    match std::pin::pin!(future).poll(&mut context) {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("transaction parsing unexpectedly suspended"),
    }
}

fn instruction(accounts: Vec<u8>, marker: u8) -> CompiledInstruction {
    let mut data = INITIALIZE_POOL.to_vec();
    data.extend_from_slice(&[marker, marker + 1]);
    CompiledInstruction { program_id_index: 3, accounts, data }
}

fn fixture() -> SubscribeUpdateTransactionInfo {
    let payer = Pubkey::new_from_array([1; 32]);
    let token = Pubkey::new_from_array([2; 32]);
    let pool = Pubkey::new_from_array([3; 32]);
    let balance = |amount: &str| TokenBalance {
        account_index: 4,
        mint: Pubkey::new_from_array([4; 32]).to_string(),
        owner: payer.to_string(),
        program_id: spl_token::ID.to_string(),
        ui_token_amount: Some(UiTokenAmount {
            decimals: 6,
            amount: amount.to_owned(),
            ..Default::default()
        }),
    };
    let inner = instruction(vec![4, 5, 0], 22);
    let second_inner = instruction(vec![5, 4, 0], 33);
    SubscribeUpdateTransactionInfo {
        signature: signature().as_ref().to_vec(),
        index: 9,
        transaction: Some(Transaction {
            message: Some(Message {
                account_keys: [payer, Pubkey::default(), COMPUTE_BUDGET, WHIRLPOOL_PROGRAM_ID]
                    .iter()
                    .map(|key| key.to_bytes().to_vec())
                    .collect(),
                instructions: vec![
                    CompiledInstruction {
                        program_id_index: 2,
                        data: [&[2][..], &200_000u32.to_le_bytes()].concat(),
                        ..Default::default()
                    },
                    instruction(vec![5, 4, 0], 11),
                    CompiledInstruction {
                        program_id_index: 2,
                        data: [&[3][..], &42u64.to_le_bytes()].concat(),
                        ..Default::default()
                    },
                ],
                versioned: true,
                ..Default::default()
            }),
            ..Default::default()
        }),
        meta: Some(TransactionStatusMeta {
            loaded_writable_addresses: vec![token.to_bytes().to_vec(), pool.to_bytes().to_vec()],
            loaded_readonly_addresses: vec![JITO_TIP.to_bytes().to_vec()],
            inner_instructions: vec![InnerInstructions {
                index: 1,
                instructions: vec![
                    InnerInstruction {
                        program_id_index: inner.program_id_index,
                        accounts: inner.accounts,
                        data: inner.data,
                        stack_height: Some(2),
                    },
                    InnerInstruction {
                        program_id_index: 1,
                        accounts: vec![0, 6],
                        data: [&2u32.to_le_bytes()[..], &5_000u64.to_le_bytes()].concat(),
                        stack_height: Some(2),
                    },
                    InnerInstruction {
                        program_id_index: second_inner.program_id_index,
                        accounts: second_inner.accounts,
                        data: second_inner.data,
                        stack_height: Some(2),
                    },
                ],
            }],
            pre_token_balances: vec![balance("10")],
            post_token_balances: vec![balance("20")],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn grpc_events(transaction: SubscribeUpdateTransactionInfo) -> Vec<DexEvent> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    parse_ready(EventParser::parse_grpc_transaction(
        &[Protocol::Whirlpool],
        None,
        transaction,
        signature(),
        Some(123),
        BLOCK_TIME,
        0,
        None,
        Some(9),
        None,
        Arc::new(move |event| sink.lock().unwrap().push(event)),
    ))
    .unwrap();
    Arc::try_unwrap(events).unwrap().into_inner().unwrap()
}

fn normalize_times(events: &mut [DexEvent]) {
    for event in events {
        event.metadata_mut().handle_us = 0;
    }
}

#[test]
fn grpc_batch_preserves_events_loaded_keys_raw_instructions_and_audit() {
    let mut single = grpc_events(fixture());
    let mut batch = parse_ready(EventParser::parse_grpc_transaction_to_events(
        &[Protocol::Whirlpool],
        None,
        fixture(),
        signature(),
        Some(123),
        BLOCK_TIME,
        0,
        None,
        Some(9),
        None,
        true,
    ))
    .unwrap()
    .unwrap();
    normalize_times(&mut single);
    normalize_times(&mut batch.events);
    assert_eq!(single, batch.events);
    let indices: Vec<_> = single
        .iter()
        .map(|event| {
            let metadata = event.metadata();
            assert_eq!(metadata.signature, signature());
            assert_eq!(metadata.slot, 123);
            assert_eq!(metadata.block_time_ms, 1_700_000_000_123);
            assert_eq!(metadata.transaction_index, Some(9));
            (metadata.outer_index, metadata.inner_index)
        })
        .collect();
    assert_eq!(indices, [(0, None), (1, None), (1, Some(0)), (1, Some(2)), (2, None)]);
    assert_eq!(batch.execution_status, TxExecutionStatus::Success);
    assert_eq!(batch.compute_unit_limit, Some(200_000));
    assert_eq!(batch.compute_unit_price_micro_lamports, 42);
    assert!(batch.compute_unit_price_set);
    assert!(batch.has_jito_tip);
    assert_eq!(batch.block_time, Some(1_700_000_000));
    assert_eq!(batch.raw_dex_instructions.len(), 3);
    for ((event, raw), marker) in
        single[1..4].iter().zip(&batch.raw_dex_instructions).zip([11, 22, 33])
    {
        let DexEvent::WhirlpoolInstructionEvent(event) = event else { panic!("wrong event") };
        assert_eq!(event.kind, WhirlpoolInstructionKind::InitializePool);
        assert_eq!(event.accounts, raw.accounts);
        assert_eq!(event.data, [marker, marker + 1]);
        assert_eq!(&raw.data[..8], &INITIALIZE_POOL);
        assert_eq!(&raw.data[8..], &event.data);
        assert_eq!(raw.outer_index, event.metadata.outer_index as u32);
        assert_eq!(raw.inner_index, event.metadata.inner_index.map(|index| index as u32));
        assert_eq!(raw.stack_height, raw.inner_index.map(|_| 2));
        assert_eq!(raw.accounts[2], Pubkey::new_from_array([1; 32]));
    }
    assert_eq!(batch.raw_dex_instructions[0].account_indices, [5, 4, 0]);
    assert_eq!(
        batch.raw_dex_instructions[0].accounts[..2],
        [Pubkey::new_from_array([3; 32]), Pubkey::new_from_array([2; 32])]
    );
    let audit = batch.tx_exec_meta.unwrap();
    assert!(audit.parse_errors.is_empty());
    assert_eq!(audit.token_balance_changes.len(), 1);
    let change = &audit.token_balance_changes[0];
    assert_eq!(change.account_index, 4);
    assert_eq!(change.account, Some(Pubkey::new_from_array([2; 32])));
    assert_eq!(change.pre_amount, Some(10));
    assert_eq!(change.post_amount, Some(20));
}

#[test]
fn versioned_batch_matches_grpc_order_and_preserves_borrowed_accounts() {
    let source = fixture();
    let message = source.transaction.as_ref().unwrap().message.as_ref().unwrap();
    let meta = source.meta.as_ref().unwrap();
    let accounts: Vec<Pubkey> = message
        .account_keys
        .iter()
        .chain(&meta.loaded_writable_addresses)
        .chain(&meta.loaded_readonly_addresses)
        .map(|key| Pubkey::try_from(key.as_slice()).unwrap())
        .collect();
    let transaction = VersionedTransaction {
        signatures: vec![signature()],
        message: VersionedMessage::V0(v0::Message {
            account_keys: accounts[..4].to_vec(),
            instructions: message
                .instructions
                .iter()
                .map(|ix| SdkInstruction {
                    program_id_index: ix.program_id_index as u8,
                    accounts: ix.accounts.clone(),
                    data: ix.data.clone(),
                })
                .collect(),
            ..Default::default()
        }),
    };
    let inner: Vec<_> = meta
        .inner_instructions
        .iter()
        .map(|group| solana_transaction_status::InnerInstructions {
            index: group.index as u8,
            instructions: group
                .instructions
                .iter()
                .map(|ix| solana_transaction_status::InnerInstruction {
                    instruction: SdkInstruction {
                        program_id_index: ix.program_id_index as u8,
                        accounts: ix.accounts.clone(),
                        data: ix.data.clone(),
                    },
                    stack_height: ix.stack_height,
                })
                .collect(),
        })
        .collect();
    let mut batch = parse_ready(EventParser::parse_versioned_transaction_to_events(
        &[Protocol::Whirlpool],
        None,
        &transaction,
        signature(),
        Some(123),
        BLOCK_TIME,
        0,
        &accounts,
        &inner,
        None,
        Some(9),
        Some(2),
        Some(3),
        None,
    ))
    .unwrap()
    .unwrap();
    let sink = Arc::new(Mutex::new(Vec::new()));
    let callback_sink = Arc::clone(&sink);
    parse_ready(EventParser::parse_instruction_events_from_versioned_transaction(
        &[Protocol::Whirlpool],
        None,
        &transaction,
        signature(),
        Some(123),
        BLOCK_TIME,
        0,
        &accounts,
        &inner,
        None,
        Some(9),
        None,
        Arc::new(move |event| callback_sink.lock().unwrap().push(event)),
    ))
    .unwrap();
    let mut single = Arc::try_unwrap(sink).unwrap().into_inner().unwrap();
    let mut grpc = grpc_events(source);
    normalize_times(&mut grpc);
    normalize_times(&mut single);
    normalize_times(&mut batch.events);
    assert_eq!(grpc, single);
    assert_eq!(single, batch.events);
    assert!(batch.has_jito_tip);
    assert_eq!(batch.entry_index, Some(2));
    assert_eq!(batch.tx_index_in_entry, Some(3));
    assert_eq!(batch.execution_status, TxExecutionStatus::Unknown);
    assert_eq!(accounts.len(), 7);

    // Legacy behavior pads unresolved outer account indices with default keys.
    // The caller's borrowed static account table must remain intact.
    let static_keys = &accounts[..4];
    let padded = parse_ready(EventParser::parse_versioned_transaction_to_events(
        &[Protocol::Whirlpool],
        None,
        &transaction,
        signature(),
        None,
        None,
        0,
        static_keys,
        &[],
        None,
        None,
        None,
        None,
        None,
    ))
    .unwrap()
    .unwrap();
    let DexEvent::WhirlpoolInstructionEvent(event) = &padded.events[1] else {
        panic!("wrong event")
    };
    assert_eq!(event.accounts[..2], [Pubkey::default(); 2]);
    assert_eq!(static_keys, &accounts[..4]);
}

#[test]
fn borrowed_cpi_retains_compute_unit_log_attribution() {
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
    let batch = parse_ready(EventParser::parse_grpc_transaction_to_events(
        &[Protocol::Whirlpool],
        None,
        transaction,
        signature(),
        None,
        None,
        0,
        None,
        None,
        Some(&config),
        false,
    ))
    .unwrap()
    .unwrap();
    assert_eq!(
        batch.events.iter().map(|event| event.metadata().swap_compute_units).collect::<Vec<_>>(),
        [None, Some(404), Some(101), Some(202), None]
    );
}

#[test]
fn malformed_loaded_key_is_rejected_without_shifting_indices() {
    let mut transaction = fixture();
    transaction.meta.as_mut().unwrap().loaded_writable_addresses[0].pop();
    let error = parse_ready(EventParser::parse_grpc_transaction_to_events(
        &[Protocol::Whirlpool],
        None,
        transaction,
        signature(),
        None,
        None,
        0,
        None,
        None,
        None,
        true,
    ))
    .unwrap_err();
    assert!(error.to_string().contains("account key at index 4"));
}

#[test]
fn account_table_uses_one_allocation_for_static_and_loaded_keys() {
    let transaction = SubscribeUpdateTransactionInfo {
        transaction: Some(Transaction {
            message: Some(Message { account_keys: vec![vec![1; 32]; 32], ..Default::default() }),
            ..Default::default()
        }),
        meta: Some(TransactionStatusMeta {
            loaded_writable_addresses: vec![vec![2; 32]; 8],
            loaded_readonly_addresses: vec![vec![3; 32]; 8],
            ..Default::default()
        }),
        ..Default::default()
    };
    let (result, allocations) = measure(|| {
        parse_ready(EventParser::parse_grpc_transaction_to_events(
            &[],
            None,
            transaction,
            signature(),
            None,
            None,
            0,
            None,
            None,
            None,
            true,
        ))
    });
    assert!(result.unwrap().is_none());
    assert_eq!(allocations.calls, 1, "{allocations:?}");
    assert_eq!(allocations.bytes, 48 * size_of::<Pubkey>());
}

#[test]
fn cpi_buffer_count_does_not_add_allocations() {
    for count in [1, 20] {
        let mut transaction = fixture();
        transaction.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions =
            vec![CompiledInstruction { program_id_index: 1, ..Default::default() }];
        let meta = transaction.meta.as_mut().unwrap();
        meta.inner_instructions = vec![InnerInstructions {
            index: 0,
            instructions: vec![
                InnerInstruction {
                    program_id_index: 1,
                    accounts: vec![0, 6],
                    data: vec![0; 64],
                    stack_height: Some(2),
                };
                count
            ],
        }];
        let callback: Arc<dyn Fn(DexEvent) + Send + Sync> =
            Arc::new(|_| panic!("no DEX instruction"));
        let (result, allocations) = measure(|| {
            parse_ready(EventParser::parse_grpc_transaction(
                &[],
                None,
                transaction,
                signature(),
                None,
                None,
                0,
                None,
                None,
                None,
                callback,
            ))
        });
        result.unwrap();
        // One account table and one inner-event buffer, independent of CPI count.
        assert_eq!(allocations.calls, 2, "{count} CPI instructions: {allocations:?}");
    }
}

#[test]
fn wrappers_move_payloads_without_allocating_even_through_compatibility_factories() {
    let account = SubscribeUpdateAccount {
        slot: 12,
        is_startup: true,
        account: Some(SubscribeUpdateAccountInfo {
            pubkey: vec![1; 32],
            owner: vec![2; 32],
            data: vec![9; 1024],
            txn_signature: Some(signature().as_ref().to_vec()),
            write_version: 34,
            executable: true,
            lamports: 56,
            rent_epoch: 78,
        }),
    };
    let pointer = account.account.as_ref().unwrap().data.as_ptr();
    let (account, allocations) = measure(|| factory::create_account_pretty_pooled(account));
    assert_eq!(allocations.calls, 0, "{allocations:?}");
    assert_eq!(account.data.as_ptr(), pointer);
    assert_eq!(
        (account.slot, account.write_version, account.lamports, account.rent_epoch),
        (12, 34, 56, 78)
    );
    assert!(account.is_startup && account.executable);
    assert_eq!(account.signature, signature());
    assert_eq!(account.pubkey, Pubkey::new_from_array([1; 32]));
    assert_eq!(account.owner, Pubkey::new_from_array([2; 32]));
    let block = SubscribeUpdateBlockMeta {
        slot: 12,
        blockhash: "blockhash".to_owned(),
        ..Default::default()
    };
    let pointer = block.blockhash.as_ptr();
    let (block, allocations) =
        measure(|| factory::create_block_meta_pretty_pooled(block, BLOCK_TIME));
    assert_eq!(allocations.calls, 0, "{allocations:?}");
    assert_eq!(block.block_hash.as_ptr(), pointer);
    assert_eq!(block.block_time, BLOCK_TIME);
    let transaction = SubscribeUpdateTransaction { transaction: Some(fixture()), slot: 123 };
    let pointer = transaction.transaction.as_ref().unwrap().signature.as_ptr();
    let (transaction, allocations) =
        measure(|| factory::create_transaction_pretty_pooled(transaction, BLOCK_TIME));
    assert_eq!(allocations.calls, 0, "{allocations:?}");
    assert_eq!(transaction.grpc_tx.signature.as_ptr(), pointer);
    assert_eq!(transaction.signature, signature());
    assert_eq!(transaction.transaction_index, Some(9));
    assert_eq!(transaction.block_time, BLOCK_TIME);
}

#[test]
fn returning_legacy_pool_objects_reuses_the_original_box_without_allocation() {
    let accounts = AccountPrettyPool::new(1, 1);
    let blocks = BlockMetaPrettyPool::new(1, 1);
    let transactions = TransactionPrettyPool::new(1, 1);
    let mut account = accounts.acquire();
    account.data = vec![1; 8];
    account.signature = signature();
    let account_pointer = &*account as *const AccountPretty;
    let block = blocks.acquire();
    let block_pointer = &*block as *const BlockMetaPretty;
    let transaction = transactions.acquire();
    let transaction_pointer = &*transaction as *const TransactionPretty;
    let (_, allocations) = measure(|| {
        drop(account);
        drop(block);
        drop(transaction);
    });
    assert_eq!(allocations.calls, 0, "{allocations:?}");
    let account = accounts.acquire();
    assert_eq!(&*account as *const _, account_pointer);
    assert!(account.data.is_empty());
    assert_eq!(account.signature, Signature::default());
    assert_eq!(&*blocks.acquire() as *const _, block_pointer);
    assert_eq!(&*transactions.acquire() as *const _, transaction_pointer);
}

#[test]
fn owned_event_callbacks_and_batches_do_not_clone_event_buffers() {
    let accounts = [
        Pubkey::new_from_array([1; 32]),
        Pubkey::new_from_array([2; 32]),
        Pubkey::new_from_array([3; 32]),
        WHIRLPOOL_PROGRAM_ID,
    ];
    let ix = instruction(vec![0, 1, 2], 11);
    let transaction = VersionedTransaction {
        signatures: vec![signature()],
        message: VersionedMessage::V0(v0::Message {
            account_keys: accounts.to_vec(),
            instructions: vec![SdkInstruction {
                program_id_index: 3,
                accounts: ix.accounts,
                data: ix.data,
            }],
            ..Default::default()
        }),
    };
    let sink = Arc::new(Mutex::new(None));
    let callback_sink = Arc::clone(&sink);
    let callback: Arc<dyn Fn(DexEvent) + Send + Sync> =
        Arc::new(move |event| *callback_sink.lock().unwrap() = Some(event));
    let (result, single_allocations) = measure(|| {
        parse_ready(EventParser::parse_instruction_events_from_versioned_transaction(
            &[Protocol::Whirlpool],
            None,
            &transaction,
            signature(),
            None,
            None,
            0,
            &accounts,
            &[],
            None,
            None,
            None,
            callback,
        ))
    });
    result.unwrap();
    // Resolved instruction accounts and the two owned event buffers are necessary.
    // Borrowing the caller's account table and moving the event add no allocations.
    assert!(single_allocations.calls <= 3, "{single_allocations:?}");
    let mut single = Arc::try_unwrap(sink).unwrap().into_inner().unwrap().unwrap();
    let (batch, batch_allocations) = measure(|| {
        parse_ready(EventParser::parse_versioned_transaction_to_events(
            &[Protocol::Whirlpool],
            None,
            &transaction,
            signature(),
            None,
            None,
            0,
            &accounts,
            &[],
            None,
            None,
            None,
            None,
            None,
        ))
    });
    let mut batch = batch.unwrap().unwrap();
    // Batch adds a result Vec plus the raw instruction and its three owned buffers.
    assert!(batch_allocations.calls <= 8, "{batch_allocations:?}");
    single.metadata_mut().handle_us = 0;
    normalize_times(&mut batch.events);
    assert_eq!(batch.events, [single]);
}
