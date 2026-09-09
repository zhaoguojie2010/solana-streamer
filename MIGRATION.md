# Migrating to transaction batches

This guide describes the unreleased API in the current repository. Use a git/path dependency when following these examples. Consumers must migrate with the library; the old metadata-expanding and event-copying adapters have been removed.

| Previous API | Current API |
| --- | --- |
| `DexEvent` | `TxEvent`, `AccountEvent`, separate block/slot events |
| `TxDexEvents` | Owned `TxBatch` or borrowed `TxView` |
| Async `EventParser` static methods | Reusable, synchronous `TxParser::visit` / `parse_owned` |
| `subscribe_events_immediate` / transaction callbacks | `subscribe(SubscriptionRequest, FnMut(StreamEvent<'_>))` |
| Cloning events before async work | `subscribe_queued(request, QueueConfig)` |
| Per-event signature, slot, transaction index and time | `batch.meta`; account update metadata lives in `view.frame` |
| `raw_dex_instructions`, modeled event `accounts` / `data` | `batch.instruction(event.metadata().instruction_index)` |
| `remaining_accounts: Vec<Pubkey>` | `remaining_account_indices: Vec<u8>`, indexing `batch.keys` |
| `AccountPretty`, account `Vec<u8>` | `AccountFrame`, shared `Bytes` |
| Global metrics, transaction caches and object pools | Per-client metrics, worker-local scratch, direct payload moves |

## Borrowed delivery

```rust
use solana_streamer_sdk::streaming::{
    event_parser::{common::EventType, ParseOptions, ParsePlan, Protocol, TxEvent},
    yellowstone_grpc::{StreamEvent, SubscriptionRequest, TransactionFilter},
};
let plan = ParsePlan::new(&[Protocol::PumpFun],
    Some(&[EventType::PumpFunBuy, EventType::PumpFunSell]), ParseOptions::default());
let mut request = SubscriptionRequest::new(plan);
request.transactions = vec![TransactionFilter {
    account_include: Protocol::PumpFun.get_program_id().iter().map(ToString::to_string).collect(),
    ..TransactionFilter::default()
}];
grpc.subscribe(request, |message| {
    if let StreamEvent::Transaction(batch) = message {
        for event in batch.events {
            if let TxEvent::PumpFunTradeEvent(trade) = event {
                println!("{} {} {}", batch.meta.signature, batch.meta.slot, trade.sol_amount);
            }
        }
    }
}).await?;
```

A callback borrows one update and may mutate its own state without a `Sync` bound. It cannot retain the view across an await. Keep synchronous callbacks short; use the owned queue for database/network work. `TxView::to_owned()` explicitly deep-copies a view; `parse_owned` and queued delivery move buffers instead.

## Plans and source data

`ParsePlan::new(protocols, None, options)` selects all event types; `Some(&[])` selects none. Filtering precedes event construction. Create instructions needed for developer flags remain dependencies even when excluded from output. State is reset between transactions, and later creates do not retroactively mark earlier trades.

Batches with no events are still delivered when instruction/log retention, auditing or transaction summaries are requested. Otherwise empty batches are skipped.

`ParseOptions::default()` enables instruction decoding, necessary CPI merging and transaction-local context. Log enrichment, log/raw retention, CU extraction, balance auditing, compute budget summaries, Jito detection and swap classification are opt-in:

- `enrich_logs`, `retain_logs`, `retain_instructions` are independent.
- `swap_cu: SwapCuParseConfig::default_enabled()` extracts selected swap CU from the same invocation index as log enrichment.
- `balance_audit` replaces `ClientConfig.tx_exec_meta_audit`.
- `compute_budget`, `detect_jito`, `classify_swaps` populate optional fields in `batch.summary`.
- Classification uses the complete event sequence of selected protocols, including events excluded from output.

`instruction_index` addresses the flattened sequence of each outer instruction followed by its CPIs, including non-DEX instructions. `outer_index` and `inner_index` preserve their original positions. Raw views include the discriminator and project account indices over the batch key table.

Generic CLMM, DLMM, DAMM v2 and Whirlpool instruction events require their source. Their batches therefore keep the complete instruction sequence even when raw retention is off. Other decoded events remain usable without retaining instruction/log buffers; remaining-account indices still require `batch.keys`.

Log association checks program, invocation order and stack depth. Incomplete or ambiguous invocations receive no inferred CU/execution observations. Completed children may still provide evidence. gRPC `created_at` is no longer treated as a block timestamp; transaction block time is 0 when unavailable. RPC execution metadata preserves Success/Failed; inputs without execution evidence use Unknown. Set `request.include_failed_transactions = true` to receive failed transactions.

## Account views

`StreamEvent::Account(view)` prefilters by owner/discriminator and does not decode a full snapshot automatically. Use checked `u64_at`, `u128_at` or `pubkey_at` with a known account layout, or explicitly call `view.decode()`. Snapshots share the original `Bytes`. Consumers may cache decoded snapshots by account version; the parser holds no cross-version account cache.

## Owned queues and shutdown

```rust
use solana_streamer_sdk::streaming::common::{OwnedStreamEvent, QueueConfig};
let mut queue = grpc.subscribe_queued(request, QueueConfig::default()).await?;
while let Some(envelope) = queue.recv().await? {
    if let OwnedStreamEvent::Transaction(batch) = &*envelope {
        println!("{}", batch.meta.signature);
        // The envelope can remain alive across awaits.
    }
}
```

The defaults are 256 items, 64 MiB of charged payload capacity and a 5-second maximum delivery age. Capacity/byte overload stops the subscription immediately. Accepted items drain in order before the error is returned. Stale delivery closes and clears the queue with an explicit error. Dropping the receiver closes production.

An envelope keeps its byte permit after dequeue until it is dropped. Charges include Vec/String capacity and visible account bytes, excluding allocator/channel overhead and potentially larger shared `Bytes` backing allocations. This is a payload budget, not an RSS limit.

`grpc.stop().await?` waits for current synchronous processing and closes production; accepted queue items remain drainable. `last_error()`, `stop()` and handle `join()` expose failures. The caller decides how to reconnect and recover missing updates; the SDK does not automatically reconnect or reorder across streams.

## Offline parsing and metrics

RPC lives outside the core and is disabled by default:

| Feature | Capabilities |
| --- | --- |
| Default (none) | gRPC streaming, account decoding and pure transaction parsing |
| `rpc` | The `rpc` module, response types and `rpc::transaction_frame(response, recv_us)`, without an HTTP client |
| `rpc-client` | Includes `rpc` plus `rpc::RpcClient`, `RpcTransactionConfig` and `CommitmentConfig` for query tools |

Replace `TxFrame::from_rpc` with `rpc::transaction_frame`. The `common::SolanaRpcClient` alias is removed. The helper `parse_swap_data_from_next_instructions`, which accepts RPC `InnerInstructions`, also moves to `rpc`. The caller or example owns HTTP queries; the core exposes no RPC transaction types.

`TxFrame::from_versioned` remains available by default for an owned SDK transaction and its resolved static + writable ALT + readonly ALT key table. It performs no network access. Invalid keys/indices are errors.

```bash
cargo run --features rpc-client --example parse_tx_events -- <signature>
cargo test --features rpc --test rpc_adapter
cargo build --examples --features rpc-client
```

The default examples build includes 19 gRPC examples. The [historical transaction example](examples/parse_tx_events.rs) requires `rpc-client`.

Keep one parser per worker. `ScratchLimits` bounds retained scratch capacity after large inputs; owned output buffers cannot be reused while a consumer owns them.

`ClientConfig.enable_metrics` selects metrics at subscription startup. Disabled streams allocate no metrics callbacks and perform no metrics counting. Enabled workers merge local counts every 128 transaction/account/block updates or one second. `processing_us` includes synchronous delivery and is not a pure decoder latency measurement.

See [grpc_example](examples/grpc_example.rs), [queued_subscription](examples/queued_subscription.rs), [dynamic_subscription](examples/dynamic_subscription.rs), the [Chinese migration guide](MIGRATION_CN.md) and the [architecture/validation record](docs/performance-architecture.md).
