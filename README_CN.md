<div align="center">
    <h1>🌊 Solana Streamer</h1>
    <h3><em>从 Solana DEX 交易程序实时流式传输事件。</em></h3>
</div>

<p align="center">
    <strong>一个轻量级的 Rust 库，为 PumpFun、PumpSwap、Bonk 和 Raydium 协议提供高效的事件解析和订阅功能。</strong>
</p>

<p align="center">
    <a href="https://crates.io/crates/solana-streamer-sdk">
        <img src="https://img.shields.io/crates/v/solana-streamer-sdk.svg" alt="Crates.io">
    </a>
    <a href="https://docs.rs/solana-streamer-sdk">
        <img src="https://docs.rs/solana-streamer-sdk/badge.svg" alt="Documentation">
    </a>
    <a href="https://github.com/0xfnzero/solana-streamer/blob/main/LICENSE">
        <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License">
    </a>
    <a href="https://github.com/0xfnzero/solana-streamer">
        <img src="https://img.shields.io/github/stars/0xfnzero/solana-streamer?style=social" alt="GitHub stars">
    </a>
    <a href="https://github.com/0xfnzero/solana-streamer/network">
        <img src="https://img.shields.io/github/forks/0xfnzero/solana-streamer?style=social" alt="GitHub forks">
    </a>
</p>

<p align="center">
    <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust">
    <img src="https://img.shields.io/badge/Solana-9945FF?style=for-the-badge&logo=solana&logoColor=white" alt="Solana">
    <img src="https://img.shields.io/badge/Streaming-FF6B6B?style=for-the-badge&logo=livestream&logoColor=white" alt="Real-time Streaming">
    <img src="https://img.shields.io/badge/gRPC-4285F4?style=for-the-badge&logo=grpc&logoColor=white" alt="gRPC">
</p>

<p align="center">
    <a href="README_CN.md">中文</a> | 
    <a href="README.md">English</a> | 
    <a href="https://fnzero.dev/">Website</a> |
    <a href="https://t.me/fnzero_group">Telegram</a>
</p>

---

## 目录

- [🚀 项目特性](#-项目特性)
- [⚡ 安装](#-安装)
- [🔄 迁移指南](#-迁移指南)
- [⚙️ 配置系统](#️-配置系统)
- [📚 使用示例](#-使用示例)
- [🔧 支持的协议](#-支持的协议)
- [🌐 事件流服务](#-事件流服务)
- [🏗️ 架构特性](#️-架构特性)
- [📁 项目结构](#-项目结构)
- [⚡ 性能考虑](#-性能考虑)
- [📄 许可证](#-许可证)
- [📞 联系方式](#-联系方式)
- [⚠️ 重要注意事项](#️-重要注意事项)

## 🚀 项目特性

### 核心功能
- **实时事件流**: 订阅多个 Solana DEX 协议的实时交易事件
- **Yellowstone gRPC 支持**: 使用 Yellowstone gRPC 进行高性能事件订阅
- **统一事件接口**: 在所有支持的协议中保持一致的事件处理

### 多协议支持
- **PumpFun**: 迷因币交易平台事件
- **PumpSwap**: PumpFun 的交换协议事件
- **Bonk**: 代币发布平台事件 (letsbonk.fun)
- **Raydium CPMM**: Raydium 集中池做市商事件
- **Raydium CLMM**: Raydium 集中流动性做市商事件
- **Raydium AMM V4**: Raydium 自动做市商 V4 事件

### 高级功能
- **事件解析系统**: 自动解析和分类协议特定事件
- **账户状态监控**: 实时监控协议账户状态和配置变更
- **交易与账户事件过滤**: 分别过滤交易事件和账户状态变化
- **动态订阅管理**: 运行时过滤器更新而无需重新连接，支持自适应监控策略
- **多重过滤器支持**: 在单个订阅中支持多个交易和账户过滤器
- **高级账户过滤**: 使用 memcmp 过滤器进行精确的账户数据匹配和监控
- **Token2022 支持**: 增强对 SPL Token 2022 的支持，包含扩展状态解析

### 性能与优化
- **高性能**: 针对低延迟事件处理进行优化
- **批处理优化**: 批量处理事件以减少回调开销
- **性能监控**: 内置性能指标监控，包括事件处理速度
- **内存优化**: 移动事件所有权、共享交易公钥表、借用 CPI 指令缓冲区，减少拷贝与内存分配
- **按需解析**: 通过 ParsePlan 选择输出和可选解析，默认跳过日志与审计开销
- **有界交付**: 限制队列数量、计费字节和交付年龄，超限显式失败
- **运行时更新**: 在消息间替换完整订阅与解析计划
- **优雅关闭**: 支持编程式 stop() 方法进行干净的关闭

## ⚡ 安装

### 直接克隆

将项目克隆到您的项目目录：

```bash
cd your_project_root_directory
git clone https://github.com/0xfnzero/solana-streamer
```

在您的 `Cargo.toml` 中添加依赖：

```toml
# 添加到您的 Cargo.toml
solana-streamer-sdk = { path = "./solana-streamer", version = "1.1.5" }
```

### 使用 crates.io

```toml
# 添加到您的 Cargo.toml
solana-streamer-sdk = "1.1.5"
```

### RPC 为可选功能

默认构建提供 gRPC 订阅与同步解析，不启用 Solana HTTP RPC。需要适配已有 RPC 响应时开启 `rpc`；需要按签名查询历史交易时开启 `rpc-client`（包含 `rpc`）：

```toml
solana-streamer-sdk = { path = "./solana-streamer", features = ["rpc-client"] }
```

适配入口为 `solana_streamer_sdk::rpc::transaction_frame`，HTTP 客户端由调用方使用。默认 `cargo build --examples` 编译 19 个流式示例；加 `--features rpc-client` 编译全部 20 个示例。

## 🔄 迁移指南

当前仓库引入未发布的批次接口变更：`TxEvent` 与 `AccountEvent` 分离，公共元数据移到 `TxBatch.meta`，同步消费使用 `TxView`，异步消费使用有界队列。旧的 `DexEvent`、`TxDexEvents`、逐事件回调和对象池接口已移除。

请使用当前仓库的 git/path 依赖，并按[中文迁移指南](MIGRATION_CN.md)调整使用方；发布版本应参考其对应文档。

## ⚙️ 配置系统

您可以自定义客户端配置：

```rust
use solana_streamer_sdk::streaming::grpc::ClientConfig;

// 使用默认配置
let grpc = YellowstoneGrpc::new(endpoint, token)?;

// 或创建自定义配置
let mut config = ClientConfig::default();
config.enable_metrics = true;  // 启用性能监控
config.connection.connect_timeout = 30;  // 30 秒
config.connection.request_timeout = 120;  // 120 秒

let grpc = YellowstoneGrpc::new_with_config(endpoint, token, config)?;
```

**可用配置选项：**
- `enable_metrics`: 启用/禁用性能监控（默认：false）
- `connection.connect_timeout`: 连接超时（秒）（默认：10）
- `connection.request_timeout`: 请求超时（秒）（默认：60）
- `connection.max_decoding_message_size`: 最大消息大小（字节）（默认：10MB）

## 📚 使用示例

编译全部示例、配置服务地址 / token 和账户地址的方法见 [示例运行说明](examples/README.md)。

### 使用示例概览表

| 描述 | 运行命令 | 源码路径 |
|------|---------|----------|
| 使用 Yellowstone gRPC 监控交易事件 | `cargo run --example grpc_example` | [examples/grpc_example.rs](examples/grpc_example.rs) |
| 解析 Solana 主网交易数据 | `cargo run --features rpc-client --example parse_tx_events -- <signature>` | [examples/parse_tx_events.rs](examples/parse_tx_events.rs) |
| 监控 PancakeSwap V3 交换事件（Swap/SwapV2） | `cargo run --example pancakeswap_swap_with_logs` | [examples/pancakeswap_swap_with_logs.rs](examples/pancakeswap_swap_with_logs.rs) |
| 运行时更新过滤器 | `cargo run --example dynamic_subscription` | [examples/dynamic_subscription.rs](examples/dynamic_subscription.rs) |
| 监控特定代币账户余额变化 | `cargo run --example token_balance_listen_example` | [examples/token_balance_listen_example.rs](examples/token_balance_listen_example.rs) |
| 跟踪 nonce 账户状态变化 | `cargo run --example nonce_listen_example` | [examples/nonce_listen_example.rs](examples/nonce_listen_example.rs) |
| 使用 memcmp 过滤器监控 PumpSwap 池账户 | `cargo run --example pumpswap_pool_account_listen_example` | [examples/pumpswap_pool_account_listen_example.rs](examples/pumpswap_pool_account_listen_example.rs) |
| 使用 memcmp 过滤器监控特定代币的所有关联代币账户 | `cargo run --example mint_all_ata_account_listen_example` | [examples/mint_all_ata_account_listen_example.rs](examples/mint_all_ata_account_listen_example.rs) |

拥有批次的异步消费见 [queued_subscription](examples/queued_subscription.rs).

### 事件过滤

`ParsePlan` 在事件解码和分配之前选择协议及事件类型，保留开发者标记等必要依赖。`None` 表示全部事件类型，`Some(&[])` 表示不输出事件；显式请求的汇总或原始数据仍可形成空事件批次。日志、CU、raw 指令、审计和交易分类通过 `ParseOptions` 显式开启。

```rust
use solana_streamer_sdk::streaming::event_parser::{
    common::EventType, ParseOptions, ParsePlan, Protocol,
};
let plan = ParsePlan::new(
    &[Protocol::PumpFun],
    Some(&[EventType::PumpFunBuy, EventType::PumpFunSell]),
    ParseOptions::default(),
);
```

将 plan 放入 `SubscriptionRequest::new(plan)`，设置网络交易/账户过滤器，再调用 `grpc.subscribe(request, callback).await?`。账户订阅先按 owner/discriminator 过滤，回调中通过 `AccountView::decode()` 按需解码。

## 动态订阅管理

```rust
// 包含完整解析计划及交易、账户、slot 过滤配置。
grpc.update_subscription(new_request).await?;
```

一个客户端同一时间只运行一个订阅。worker 在消息间替换完整计划并发送网络请求；返回成功表示请求已发送，不承诺服务端精确生效的交易边界。参见 [dynamic_subscription](examples/dynamic_subscription.rs)。

## 🔧 支持的协议

- **PumpFun**: 主要迷因币交易平台
- **PumpSwap**: PumpFun 的交换协议
- **Bonk**: 代币发布平台 (letsbonk.fun)
- **Raydium CPMM**: Raydium 集中池做市商协议
- **Raydium CLMM**: Raydium 集中流动性做市商协议
- **Raydium AMM V4**: Raydium 自动做市商 V4 协议

## 🌐 事件流服务

- **Yellowstone gRPC**: 高性能 Solana 事件流

## 🏗️ 架构特性

- `TxFrame` 校验并拥有输入；`InstructionAccounts` 直接投影账户索引。
- `TxParser` 同步解析，复用 worker 本地空间；无全局交易缓存。
- `TxView` 借用批次；`TxBatch` 移动交付；账户快照使用独立枚举。
- Program Data 和 CU 共用一次调用日志扫描，原始账户字节端到端使用 `Bytes`。
- `subscribe_queued` 提供数量、计费字节和交付年龄限制；超限显式报错。
- `stop().await` 等待生产者停止，已接受的队列数据可继续排空。

## 📁 项目结构

```
src/
├── common/           # 通用功能和类型
├── streaming/        # 事件流系统
│   ├── event_parser/ # 事件解析系统
│   │   ├── common/   # 通用事件解析工具
│   │   ├── core/     # 核心解析特征和接口
│   │   ├── protocols/# 协议特定解析器
│   │   │   ├── bonk/ # Bonk 事件解析
│   │   │   ├── pumpfun/ # PumpFun 事件解析
│   │   │   ├── pumpswap/ # PumpSwap 事件解析
│   │   │   ├── raydium_amm_v4/ # Raydium AMM V4 事件解析
│   │   │   ├── raydium_cpmm/ # Raydium CPMM 事件解析
│   │   │   └── raydium_clmm/ # Raydium CLMM 事件解析
│   ├── yellowstone_grpc.rs # Yellowstone gRPC 客户端
│   └── yellowstone_sub_system.rs # Yellowstone 子系统
└── lib.rs            # 主库文件
```

## ⚡ 性能考虑

[架构与验证记录](docs/performance-architecture.md)说明实现、测量范围和后续可选设计。[迁移指南](MIGRATION_CN.md)说明各项解析成本和默认选项。

当前环境中交易枚举由 11,744 B 降为 1,136 B，每事件元数据由 312 B 降为 80 B。预热借用路径与提前过滤的分配回归见测试；类型尺寸不能用于推算生产吞吐百分比。

快速同步回调使用 `subscribe`；数据库/网络消费使用 [queued_subscription](examples/queued_subscription.rs)。队列默认 256 项、64 MiB 计费载荷、5 秒交付年龄；超限停止订阅并报告错误。额度不是进程 RSS 上限。

```bash
cargo test
cargo build --examples
cargo bench --bench parser_replay
```

## 📄 许可证

MIT 许可证

## 📞 联系方式

- **网站**: https://fnzero.dev/
- **项目仓库**: https://github.com/0xfnzero/solana-streamer
- **Telegram 群组**: https://t.me/fnzero_group

## ⚠️ 重要注意事项

1. **网络稳定性**: 确保稳定的网络连接以进行连续的事件流传输
2. **速率限制**: 注意公共 gRPC 端点的速率限制
3. **错误恢复**: 实现适当的错误处理和重连逻辑
5. **合规性**: 确保遵守相关法律法规

## 语言版本

- [English](README.md)
- [中文](README_CN.md)
