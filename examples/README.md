# 运行示例 / Running examples

在项目根目录编译全部示例：

```bash
cargo build --examples
```

Yellowstone 示例从环境变量读取连接配置。默认 PublicNode 服务需要个人 token；
请在运行环境中设置 `GRPC_X_TOKEN`，或设置 `GRPC_ENDPOINT` 使用自己的服务。
所有示例均为只读订阅或查询，不需要钱包私钥。

| 环境变量 | 用途 / 默认值 |
| --- | --- |
| `GRPC_ENDPOINT` | Yellowstone 地址，默认 `https://solana-yellowstone-grpc.publicnode.com:443` |
| `GRPC_X_TOKEN` | 服务的 x-token，未设置时不发送认证头 |
| `SOLANA_RPC_URL` | 交易查询 RPC，默认 `https://api.mainnet-beta.solana.com` |
| `NONCE_ACCOUNT` | `nonce_listen_example` 必填，真实 nonce 账户公钥 |
| `TOKEN_ACCOUNT` | `token_balance_listen_example` 必填，真实 SPL Token 账户公钥（不是钱包或 mint 地址） |
| `TOKEN_MINT` | `token_decimals_listen_example` 的 mint，默认 USDC |
| `RUN_DURATION_SECS` | 流式示例订阅成功后的运行秒数，默认 1000；也可按 Ctrl+C 提前停止 |
| `RUST_LOG` | 日志级别，例如 `error` 或 `debug` |

```bash
# 配置 GRPC_X_TOKEN 后，订阅 10 秒并停止
RUN_DURATION_SECS=10 cargo run --example grpc_example

# 同样适用于 DEX 和账户订阅示例
RUN_DURATION_SECS=10 cargo run --example meteora_dlmm_bin_array_account_subscription

# 设置 NONCE_ACCOUNT / TOKEN_ACCOUNT 后运行对应示例
cargo run --example nonce_listen_example
cargo run --example token_balance_listen_example

# 默认查询内置交易，也可以传入一个或多个交易签名；完成后自动退出
cargo run --example parse_tx_events
cargo run --example parse_tx_events -- <signature> [<signature> ...]

# 动态订阅示例按阶段执行，约一分钟后自动结束，不使用 RUN_DURATION_SECS
cargo run --example dynamic_subscription
```

`PermissionDenied` 表示需检查服务的 token / 权限；RPC `429` 表示服务限流。
账户订阅只在账户更新时输出；mint 的 decimals 通常不变，短时间无输出不代表失败。
Meteora BinArray 等账户更新输出较多，可重定向到日志文件。
流式订阅在后台断开时会报错并以非零状态退出。批量运行示例时，请遵守服务的连接频率限制。

All commands run from the repository root. Configure the environment variables above before
starting an example. The default Yellowstone endpoint requires a personal token. Streaming
examples stop on Ctrl+C or after `RUN_DURATION_SECS` (1000 seconds by default). The dynamic
subscription example runs its own phases; transaction parsing exits when all signatures finish.
Account examples print updates only when the selected accounts change. Connection,
authentication and RPC failures are reported in the logs.
