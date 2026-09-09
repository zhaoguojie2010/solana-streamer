<div align="center">
    <h1>🌊 Solana Streamer</h1>
    <h3><em>Real-time event streaming from Solana DEX trading programs.</em></h3>
</div>

<p align="center">
    <strong>A lightweight Rust library providing efficient event parsing and subscription capabilities for PumpFun, PumpSwap, Bonk, and Raydium protocols.</strong>
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
    <a href="https://t.me/fnzero_group">Telegram</a> |
    <a href="https://discord.gg/vuazbGkqQE">Discord</a>
</p>

---

## Table of Contents

- [🚀 Project Features](#-project-features)
- [⚡ Installation](#-installation)
- [🔄 Migration Guide](#-migration-guide)
- [⚙️ Configuration System](#️-configuration-system)
- [📚 Usage Examples](#-usage-examples)
- [🔧 Supported Protocols](#-supported-protocols)
- [🌐 Event Streaming Services](#-event-streaming-services)
- [🏗️ Architecture Features](#️-architecture-features)
- [📁 Project Structure](#-project-structure)
- [⚡ Performance Considerations](#-performance-considerations)
- [📄 License](#-license)
- [📞 Contact](#-contact)
- [⚠️ Important Notes](#️-important-notes)

## 🚀 Project Features

### Core Capabilities
- **Real-time Event Streaming**: Subscribe to live trading events from multiple Solana DEX protocols
- **Yellowstone gRPC Support**: High-performance event subscription using Yellowstone gRPC
- **Unified Event Interface**: Consistent event handling across all supported protocols

### Multi-Protocol Support
- **PumpFun**: Meme coin trading platform events
- **PumpSwap**: PumpFun's swap protocol events
- **Bonk**: Token launch platform events (letsbonk.fun)
- **Raydium CPMM**: Raydium's Concentrated Pool Market Maker events
- **Raydium CLMM**: Raydium's Concentrated Liquidity Market Maker events
- **Raydium AMM V4**: Raydium's Automated Market Maker V4 events

### Advanced Features
- **Event Parsing System**: Automatic parsing and categorization of protocol-specific events
- **Account State Monitoring**: Real-time monitoring of protocol account states and configuration changes
- **Transaction & Account Event Filtering**: Separate filtering for transaction events and account state changes
- **Dynamic Subscription Management**: Runtime filter updates without reconnection, enabling adaptive monitoring strategies
- **Multi-Filter Support**: Support for multiple transaction and account filters in a single subscription
- **Advanced Account Filtering**: Memcmp filters for precise account data matching and monitoring
- **Token2022 Support**: Enhanced support for SPL Token 2022 with extended state parsing

### Performance & Optimization
- **High Performance**: Optimized for low-latency event processing
- **Batch Processing Optimization**: Batch processing events to reduce callback overhead
- **Performance Monitoring**: Built-in performance metrics monitoring, including event processing speed
- **Memory Optimization**: Move event payloads, share transaction account keys, and borrow CPI instruction buffers to reduce copies and allocations
- **Demand-driven Parsing**: ParsePlan selects outputs and optional work; logging and auditing are disabled by default
- **Bounded Delivery**: Limits queue count, charged bytes and delivery age with explicit overload errors
- **Runtime Updates**: Replace complete subscription and parsing plans between messages
- **Graceful Shutdown**: Support for programmatic stop() method for clean shutdown

## ⚡ Installation

### Direct Clone

Clone this project to your project directory:

```bash
cd your_project_root_directory
git clone https://github.com/0xfnzero/solana-streamer
```

Add the dependency to your `Cargo.toml`:

```toml
# Add to your Cargo.toml
solana-streamer-sdk = { path = "./solana-streamer", version = "1.1.5" }
```

### Use crates.io

```toml
# Add to your Cargo.toml
solana-streamer-sdk = "1.1.5"
```

### Optional RPC support

The default build provides gRPC streaming and synchronous parsing without Solana HTTP RPC. Enable `rpc` to adapt responses you already fetched, or `rpc-client` (which includes `rpc`) to query historical transactions:

```toml
solana-streamer-sdk = { path = "./solana-streamer", features = ["rpc-client"] }
```

Use `solana_streamer_sdk::rpc::transaction_frame` for adaptation; the caller owns HTTP queries. `cargo build --examples` builds 19 streaming examples by default. Add `--features rpc-client` to build all 20 examples.

## 🔄 Migration Guide

The current repository introduces an unreleased batch API: separate `TxEvent` / `AccountEvent`, shared metadata in `TxBatch.meta`, borrowed `TxView` callbacks and owned bounded delivery. The old `DexEvent`, `TxDexEvents`, per-event callback and pool APIs have been removed.

Use a git/path dependency for the current APIs and follow the [migration guide](MIGRATION.md). Refer to the matching documentation when using a published version.

## ⚙️ Configuration System

You can customize client configuration:

```rust
use solana_streamer_sdk::streaming::grpc::ClientConfig;

// Use default configuration
let grpc = YellowstoneGrpc::new(endpoint, token)?;

// Or create custom configuration
let mut config = ClientConfig::default();
config.enable_metrics = true;  // Enable performance monitoring
config.connection.connect_timeout = 30;  // 30 seconds
config.connection.request_timeout = 120;  // 120 seconds

let grpc = YellowstoneGrpc::new_with_config(endpoint, token, config)?;
```

**Available Configuration Options:**
- `enable_metrics`: Enable/disable performance monitoring (default: false)
- `connection.connect_timeout`: Connection timeout in seconds (default: 10)
- `connection.request_timeout`: Request timeout in seconds (default: 60)
- `connection.max_decoding_message_size`: Maximum message size in bytes (default: 10MB)

## 📚 Usage Examples

See [Running examples](examples/README.md) for building all examples and configuring endpoints, tokens, and account addresses.

### Usage Examples Summary Table

| Description | Run Command | Source Path |
|------|---------|----------|
| Monitor transaction events using Yellowstone gRPC | `cargo run --example grpc_example` | [examples/grpc_example.rs](examples/grpc_example.rs) |
| Parse Solana mainnet transaction data | `cargo run --features rpc-client --example parse_tx_events -- <signature>` | [examples/parse_tx_events.rs](examples/parse_tx_events.rs) |
| Monitor PancakeSwap V3 swap events (Swap/SwapV2) | `cargo run --example pancakeswap_swap_with_logs` | [examples/pancakeswap_swap_with_logs.rs](examples/pancakeswap_swap_with_logs.rs) |
| Update filters at runtime | `cargo run --example dynamic_subscription` | [examples/dynamic_subscription.rs](examples/dynamic_subscription.rs) |
| Monitor specific token account balance changes | `cargo run --example token_balance_listen_example` | [examples/token_balance_listen_example.rs](examples/token_balance_listen_example.rs) |
| Track nonce account state changes | `cargo run --example nonce_listen_example` | [examples/nonce_listen_example.rs](examples/nonce_listen_example.rs) |
| Monitor PumpSwap pool accounts using memcmp filters | `cargo run --example pumpswap_pool_account_listen_example` | [examples/pumpswap_pool_account_listen_example.rs](examples/pumpswap_pool_account_listen_example.rs) |
| Monitor all associated token accounts for specific mints using memcmp filters | `cargo run --example mint_all_ata_account_listen_example` | [examples/mint_all_ata_account_listen_example.rs](examples/mint_all_ata_account_listen_example.rs) |

For owned async delivery, see [queued_subscription](examples/queued_subscription.rs).

### Event Filtering

`ParsePlan` selects protocols and event types before decoding or allocating events, while retaining dependencies such as developer markers. `None` selects all event types; `Some(&[])` selects none. Log enrichment, CU, raw retention, auditing and classification are explicit `ParseOptions`.

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

Create a `SubscriptionRequest::new(plan)`, set transaction/account wire filters, then call `grpc.subscribe(request, callback).await?`. Accounts are prefiltered by owner/discriminator and decoded explicitly through `AccountView::decode()`.

## Dynamic Subscription Management

```rust
// A complete request includes its plan and transaction/account/slot filters.
grpc.update_subscription(new_request).await?;
```

Each client runs one subscription at a time. The worker replaces the plan between messages and sends its wire request. Success confirms the send, not an exact server-side transaction boundary. See [dynamic_subscription](examples/dynamic_subscription.rs).

## 🔧 Supported Protocols

- **PumpFun**: Primary meme coin trading platform
- **PumpSwap**: PumpFun's swap protocol
- **Bonk**: Token launch platform (letsbonk.fun)
- **Raydium CPMM**: Raydium's Concentrated Pool Market Maker protocol
- **Raydium CLMM**: Raydium's Concentrated Liquidity Market Maker protocol
- **Raydium AMM V4**: Raydium's Automated Market Maker V4 protocol

## 🌐 Event Streaming Services

- **Yellowstone gRPC**: High-performance Solana event streaming

## 🏗️ Architecture Features

- `TxFrame` validates and owns inputs; `InstructionAccounts` projects indices over shared keys.
- Synchronous `TxParser` reuses worker-local scratch with no global transaction cache.
- `TxView` borrows batches; `TxBatch` moves ownership; account snapshots use a separate enum.
- Program Data and CU share one invocation scan; account bytes remain `Bytes` end to end.
- `subscribe_queued` bounds item count, charged bytes and delivery age with explicit overload errors.
- `stop().await` waits for production to stop; accepted queue items remain drainable.

## 📁 Project Structure

```
src/
├── common/           # Common functionality and types
├── streaming/        # Event streaming system
│   ├── event_parser/ # Event parsing system
│   │   ├── common/   # Common event parsing tools
│   │   ├── core/     # Core parsing traits and interfaces
│   │   ├── protocols/# Protocol-specific parsers
│   │   │   ├── bonk/ # Bonk event parsing
│   │   │   ├── pumpfun/ # PumpFun event parsing
│   │   │   ├── pumpswap/ # PumpSwap event parsing
│   │   │   ├── raydium_amm_v4/ # Raydium AMM V4 event parsing
│   │   │   ├── raydium_cpmm/ # Raydium CPMM event parsing
│   │   │   └── raydium_clmm/ # Raydium CLMM event parsing
│   ├── yellowstone_grpc.rs # Yellowstone gRPC client
│   └── yellowstone_sub_system.rs # Yellowstone subsystem
├── lib.rs            # Main library file
└── ../examples/      # Runnable consumers
```

## ⚡ Performance Considerations

The [architecture and validation record](docs/performance-architecture.md) describes the implementation, measurement limits and optional future designs. The [migration guide](MIGRATION.md) explains parsing costs and defaults.

On the measured target, transaction events shrink from 11,744 B to 1,136 B and per-event metadata from 312 B to 80 B. Tests cover allocation-free warmed borrowed paths and early filtering. Layout reductions do not imply the same production throughput improvement.

Use `subscribe` for fast synchronous callbacks and [queued_subscription](examples/queued_subscription.rs) for database/network consumers. Defaults are 256 queued items, 64 MiB of charged payload and a 5-second maximum delivery age. Overload stops the subscription with an error. The byte budget is not an RSS limit.

```bash
cargo test
cargo build --examples
cargo bench --bench parser_replay
```

## 📄 License

MIT License

## 📞 Contact

- **Website**: https://fnzero.dev/
- **Project Repository**: https://github.com/0xfnzero/solana-streamer
- **Telegram Group**: https://t.me/fnzero_group
- **Discord**: https://discord.gg/vuazbGkqQE

## ⚠️ Important Notes

1. **Network Stability**: Ensure stable network connection for continuous event streaming
2. **Rate Limiting**: Be aware of rate limits on public gRPC endpoints
3. **Error Recovery**: Implement proper error handling and reconnection logic
5. **Compliance**: Ensure compliance with relevant laws and regulations

## Language Versions

- [English](README.md)
- [中文](README_CN.md)
