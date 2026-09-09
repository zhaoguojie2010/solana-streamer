# 迁移到交易批次解析接口

本文对应当前仓库的未发布变更。新接口允许使用方一起调整，不保留会重新复制元数据、账户公钥和原始指令的兼容适配层。请使用当前仓库的 git/path 依赖，并参考对应版本的示例。

## 接口变化

| 原接口 / 数据 | 新接口 / 数据 |
| --- | --- |
| `DexEvent` | 交易 `TxEvent`、账户 `AccountEvent`、独立区块/slot 事件 |
| `TxDexEvents` | `TxBatch`（拥有数据）或 `TxView`（同步借用） |
| 异步 `EventParser` 静态方法 | 同步、可复用的 `TxParser::visit` / `parse_owned` |
| 逐事件 `Arc<dyn Fn(...)>` | 每条更新一次 `FnMut(StreamEvent<'_>)`，交易包含有序事件切片 |
| `subscribe_events_immediate` / `subscribe_tx_events` | `subscribe(SubscriptionRequest, callback)` |
| 异步消费者自行复制事件 | `subscribe_queued(request, QueueConfig)` |
| 事件内 signature / slot / time / 交易索引 | `batch.meta`；账户从 `view.frame` 读取 |
| `raw_dex_instructions`、事件的 `accounts` / `data` | `batch.instruction(event.metadata().instruction_index)` |
| `remaining_accounts: Vec<Pubkey>` | `remaining_account_indices: Vec<u8>`，索引指向 `batch.keys` |
| `AccountPretty`、`Vec<u8>` 原始账户数据 | `AccountFrame`、`Bytes` |
| `ClientConfig.swap_cu_parse_config` / `tx_exec_meta_audit` | `ParseOptions.swap_cu` / `balance_audit` |
| 全局 metrics / 交易缓存 / 对象池 | 每个客户端的指标、本地解析上下文、直接移动载荷 |

`EventMetadata` 只保留协议、类型、程序 ID、指令位置、可选 CU 和可选 swap_data。`instruction_index` 是 outer 后跟对应 inner 的扁平指令序号，包含非 DEX 指令和 CPI 事件指令；不是旧 raw DEX 列表中的下标。`outer_index` / `inner_index` 保持原始链上位置。

## 同步借用消费

```rust
use solana_streamer_sdk::streaming::{
    event_parser::{common::EventType, ParseOptions, ParsePlan, Protocol, TxEvent},
    yellowstone_grpc::{StreamEvent, SubscriptionRequest, TransactionFilter},
};

let plan = ParsePlan::new(
    &[Protocol::PumpFun],
    Some(&[EventType::PumpFunBuy, EventType::PumpFunSell]),
    ParseOptions::default(),
);
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

回调只在当前更新期间借用数据，不能跨 `await` 保存。`TxView::to_owned()` 是明确的深拷贝操作；每个批次都需要异步处理时，请选择队列接口。回调中的慢操作仍会延迟收流，`FnMut` 不要求额外的 `Sync` 或用户状态锁。

交易内 create 指令即使不输出，仍可作为 trade 的开发者标记依赖解析；状态在下一笔交易前清空。同一交易先 trade 后 create 不会反向标记先前事件。

## 按需解析与源数据

`ParsePlan::new(protocols, None, options)` 选择全部事件，`Some(&[])` 不选择任何事件。协议和事件选择在订阅时保存为位图；事件在解码和构造之前过滤。交易分类需要选中协议的完整事件序列，其依赖工作不受输出类型过滤影响。

事件切片为空时，若请求了原始指令、日志保留、审计或交易汇总，仍会交付批次；只有没有事件且没有这些输出需求时才跳过交付。

`ParseOptions::default()` 只做指令解码、必要的 CPI 合并与交易内上下文处理，以下选项默认关闭：

| 选项 | 开启后的行为 |
| --- | --- |
| `enrich_logs` | 按调用位置补充执行日志字段 |
| `swap_cu` | 例如 `SwapCuParseConfig::default_enabled()`；从同一日志索引提取目标指令 CU |
| `retain_instructions` | 保留完整指令序列，通过借用视图访问数据和账户 |
| `retain_logs` | 在输出中保留原始日志；与是否解码日志独立 |
| `balance_audit` | 构造 pre/post token balance 审计；格式错误进入 `parse_errors` |
| `compute_budget` | 输出 `summary.compute_unit_limit` / `compute_unit_price`，即使预算事件被过滤 |
| `detect_jito` | 输出 `summary.is_jito` |
| `classify_swaps` | 输出 `summary.swap_kind`，基于选中协议的事件 |

未执行的可选交易总结为 `None`。未获得执行结果的输入为 `TxExecutionStatus::Unknown`；失败交易为 `Failed`。订阅默认排除失败交易，需要时设置 `request.include_failed_transactions = true`。

CLMM、DLMM、DAMM v2、Whirlpool 的通用 instruction 事件依赖原始输入，因此即使 `retain_instructions = false`，包含这些事件的批次也保留完整指令序列。输入只保存一份。只含独立解码事件时，可释放指令和日志，但会保留解析账户索引所需的公钥表。

```rust
let raw = batch.instruction(event.metadata().instruction_index);
if let Some(raw) = raw {
    for key in raw.accounts.iter() { println!("{key}"); }
    println!("{} bytes", raw.data.len()); // 包含 discriminator
}
// remaining_account_indices 的值直接用于 batch.keys[index as usize]。
```

日志关联基于调用顺序、程序与栈深度。缺失退出、截断或不匹配时，不给未完成或归属不明确的调用填充观测值；已完成子调用的证据可保留。gRPC 的 `created_at` 不再误用为区块时间；交易没有实际 block time 时为 0，RPC 的 block time 按返回值保存。

## 账户按需读取

账户类型按 owner 和 discriminator 预过滤。`StreamEvent::Account(view)` 不自动解码整个账户：

```rust
if let StreamEvent::Account(view) = message {
    println!("{} {}", view.frame.slot, view.frame.write_version);
    let field = view.u64_at(8); // 仅在该账户布局明确规定此偏移时读取
    let snapshot = view.decode(); // 需要全量快照时才调用
}
```

`u64_at`、`u128_at`、`pubkey_at` 检查范围和偏移溢出。快照与 frame 共享 `Bytes` 原始载荷，不把账户字节转回 Vec。需要重复解码时由消费者缓存快照，并按账户版本更新；解析器不维护跨交易/版本缓存。

## 有界队列、错误与停止

参见可运行的 [queued_subscription](examples/queued_subscription.rs)：

```rust
use solana_streamer_sdk::streaming::common::{OwnedStreamEvent, QueueConfig};
let mut queue = grpc.subscribe_queued(request, QueueConfig::default()).await?;
while let Some(envelope) = queue.recv().await? {
    if let OwnedStreamEvent::Transaction(batch) = &*envelope {
        // 可以在此 await 数据库/网络操作；处理完成后丢弃 envelope。
        println!("{}", batch.meta.signature);
    }
}
```

默认容量 256 项、计费载荷预算 64 MiB、最大交付年龄 5 秒。队列或字节额度用尽时立即终止订阅并报告错误；已接受的事件按顺序排完后返回该错误，不静默丢弃。过期交付返回错误并关闭、清空队列。消费者丢弃 receiver 也会关闭生产者。

字节额度随 `QueuedEvent` envelope 保留，出队后仍占额度，直到消费者丢弃。计费包括批次 Vec/String 的容量和账户可见字节；`Bytes` 可能保留更大的共享底层缓冲区，分配器、网络解码和通道自身也有开销，因此这是载荷预算，不是进程 RSS 上限。

`grpc.stop().await?` 等待当前同步处理结束并关闭生产者；队列中已接受的数据仍可排空。`last_error()`、`stop()` 与订阅 handle 的 `join()` 保留失败信息。网络失败、过载需要调用方决定重连及补回数据；SDK 不自动重连或跨流恢复顺序。

## RPC、指标和验证

RPC 已移出核心，默认不启用。按消费需求开启 feature：

| feature | 启用的能力 |
| --- | --- |
| 默认（无） | gRPC 订阅、账户解码、纯交易解析 |
| `rpc` | 独立 `rpc` 模块、响应类型和 `rpc::transaction_frame(response, recv_us)`；无 HTTP 客户端 |
| `rpc-client` | 包含 `rpc`，额外提供 `rpc::RpcClient`、`RpcTransactionConfig`、`CommitmentConfig`，用于查询工具 |

`TxFrame::from_rpc` 已迁到 `rpc::transaction_frame`；`common::SolanaRpcClient` 已删除。接收 RPC `InnerInstructions` 的辅助函数 `parse_swap_data_from_next_instructions` 也迁到 `rpc` 模块，核心不再暴露 RPC 交易类型。HTTP 请求由示例或使用方发起。

已有 `VersionedTransaction` 时，仍直接调用默认可用的 `TxFrame::from_versioned`，提供完整的 static + writable ALT + readonly ALT 公钥表；此入口不访问网络。非法公钥/账户索引返回错误，不能跳过或补零改变索引含义。

```bash
cargo run --features rpc-client --example parse_tx_events -- <signature>
cargo test --features rpc --test rpc_adapter
cargo build --examples --features rpc-client
```

默认 `cargo build --examples` 编译 19 个 gRPC 示例，启用 `rpc-client` 后才包括 [parse_tx_events](examples/parse_tx_events.rs)。

每个 worker 拥有一个 `TxParser`，`ScratchLimits` 限制交易结束后保留的事件、日志、解码和开发者暂存容量。`parse_owned` 移交输出 Vec 后，不会声称还能复用消费者持有的内存。

指标通过 `ClientConfig.enable_metrics` 在订阅启动时选择实现。关闭时不创建指标回调或执行指标计数；开启时本地累计、每 128 个交易/账户/区块更新或 1 秒合并到客户端快照。`processing_us` 包括同步交付耗时，不是纯解码延迟。

运行 `cargo test`、`cargo build --examples` 和 `cargo bench --bench parser_replay`。布局、分配与回放边界见[架构与验证记录](docs/performance-architecture.md)。
