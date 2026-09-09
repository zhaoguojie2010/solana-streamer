use crate::streaming::event_parser::{
    common::{
        build_program_data_index, build_swap_cu_index, filter::EventTypeFilter,
        high_performance_clock::elapsed_micros_since, EventMetadata, ProgramDataIndex, SwapCuIndex,
        SwapCuParseConfig,
    },
    core::{
        dispatcher::EventDispatcher,
        global_state::{
            add_bonk_dev_address, add_dev_address, is_bonk_dev_address_in_signature,
            is_dev_address_in_signature,
        },
        merger_event::merge,
    },
    protocols::raydium_amm_v4::parser::RAYDIUM_AMM_V4_PROGRAM_ID,
    DexEvent, Protocol, ResolvedDexInstruction, TxDexEvents, TxExecutionMetaAudit,
    TxExecutionStatus, TxSwapKind, TxTokenBalanceChange,
};
use prost_types::Timestamp;
use solana_sdk::{
    message::compiled_instruction::CompiledInstruction, pubkey::Pubkey, signature::Signature,
    transaction::VersionedTransaction,
};
use solana_transaction_status::InnerInstructions;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use yellowstone_grpc_proto::geyser::SubscribeUpdateTransactionInfo;

const SYSTEM_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("11111111111111111111111111111111");
/// 参与套利/路由检测时视为"稳定币锚点"的 mint。
/// 同 mint 跨池（Route）检测中排除这些 mint，避免把一笔
/// "用 USDC 买了两个不同币"的普通 tx 误判为路由。
const STABLECOIN_MINTS: &[Pubkey] = &[
    solana_sdk::pubkey!("So11111111111111111111111111111111111111112"), // WSOL
    solana_sdk::pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"), // USDC
    solana_sdk::pubkey!("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"), // USDT
    solana_sdk::pubkey!("USDJ8ZpEwKSoaiTRbZLPSqVvvtRKUTHXFMTvkEBXcYh"), // USD1
];
const JITO_TIP_ACCOUNTS: &[Pubkey] = &[
    solana_sdk::pubkey!("96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5"),
    solana_sdk::pubkey!("HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe"),
    solana_sdk::pubkey!("Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY"),
    solana_sdk::pubkey!("ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49"),
    solana_sdk::pubkey!("DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh"),
    solana_sdk::pubkey!("ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt"),
    solana_sdk::pubkey!("DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL"),
    solana_sdk::pubkey!("3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT"),
];

pub struct EventParser {}

/// Borrow protobuf instruction buffers without allocating a temporary instruction.
#[derive(Clone, Copy)]
struct InstructionView<'a> {
    program_id_index: u32,
    accounts: &'a [u8],
    data: &'a [u8],
}

impl<'a> From<&'a yellowstone_grpc_proto::prelude::CompiledInstruction> for InstructionView<'a> {
    fn from(instruction: &'a yellowstone_grpc_proto::prelude::CompiledInstruction) -> Self {
        Self {
            program_id_index: instruction.program_id_index,
            accounts: &instruction.accounts,
            data: &instruction.data,
        }
    }
}

impl<'a> From<&'a yellowstone_grpc_proto::prelude::InnerInstruction> for InstructionView<'a> {
    fn from(instruction: &'a yellowstone_grpc_proto::prelude::InnerInstruction) -> Self {
        Self {
            program_id_index: instruction.program_id_index,
            accounts: &instruction.accounts,
            data: &instruction.data,
        }
    }
}

#[derive(Clone, Copy)]
struct MintLeg {
    from_mint: Pubkey,
    to_mint: Pubkey,
}

#[derive(Clone, Copy)]
struct AccountLeg {
    from_account: Pubkey,
    to_account: Pubkey,
}

impl EventParser {
    fn summarize_compute_budget(events: &[DexEvent]) -> (u64, Option<u32>, bool) {
        let mut price = 0;
        let mut limit = None;
        let mut price_set = false;
        for event in events {
            match event {
                DexEvent::SetComputeUnitPriceEvent(event) => {
                    price = event.micro_lamports;
                    price_set = true;
                }
                DexEvent::SetComputeUnitLimitEvent(event) => limit = Some(event.units),
                _ => {}
            }
        }
        (price, limit, price_set)
    }

    fn is_system_transfer_to_jito(
        program_id_index: usize,
        account_indices: &[u8],
        data: &[u8],
        accounts: &[Pubkey],
    ) -> bool {
        accounts.get(program_id_index) == Some(&SYSTEM_PROGRAM_ID)
            && data.get(..4) == Some(2u32.to_le_bytes().as_slice())
            && account_indices
                .get(1)
                .and_then(|index| accounts.get(*index as usize))
                .is_some_and(|account| JITO_TIP_ACCOUNTS.contains(account))
    }

    // ================================================================================================
    // Public API - Entry Points
    // ================================================================================================

    /// Parse transaction from gRPC stream
    ///
    /// This is the main entry point for parsing transactions received from gRPC streams.
    /// It extracts account keys, inner instructions, and delegates to instruction parsing.
    pub async fn parse_grpc_transaction(
        protocols: &[Protocol],
        event_type_filter: Option<&EventTypeFilter>,
        grpc_tx: SubscribeUpdateTransactionInfo,
        signature: Signature,
        slot: Option<u64>,
        block_time: Option<Timestamp>,
        recv_us: i64,
        bot_wallet: Option<Pubkey>,
        transaction_index: Option<u64>,
        swap_cu_parse_config: Option<&SwapCuParseConfig>,
        callback: Arc<dyn Fn(DexEvent) + Send + Sync>,
    ) -> anyhow::Result<()> {
        let accounts = Self::grpc_account_keys(&grpc_tx)?;
        Self::parse_grpc_transaction_inner(
            protocols,
            event_type_filter,
            &grpc_tx,
            signature,
            slot,
            block_time,
            recv_us,
            &accounts,
            bot_wallet,
            transaction_index,
            swap_cu_parse_config,
            &mut |event| callback(event),
        )
    }

    /// Construct the transaction account table once, preserving protobuf index order.
    fn grpc_account_keys(grpc_tx: &SubscribeUpdateTransactionInfo) -> anyhow::Result<Vec<Pubkey>> {
        let Some(message) = grpc_tx.transaction.as_ref().and_then(|tx| tx.message.as_ref()) else {
            return Ok(Vec::new());
        };
        let (writable, readonly) = grpc_tx
            .meta
            .as_ref()
            .map(|meta| {
                (
                    meta.loaded_writable_addresses.as_slice(),
                    meta.loaded_readonly_addresses.as_slice(),
                )
            })
            .unwrap_or_default();
        let mut accounts =
            Vec::with_capacity(message.account_keys.len() + writable.len() + readonly.len());
        for (index, raw) in message.account_keys.iter().chain(writable).chain(readonly).enumerate()
        {
            // Dropping malformed keys would shift every subsequent instruction index.
            let key = Pubkey::try_from(raw.as_slice()).map_err(|error| {
                anyhow::anyhow!("invalid transaction account key at index {index}: {error}")
            })?;
            accounts.push(key);
        }
        Ok(accounts)
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_grpc_transaction_inner(
        protocols: &[Protocol],
        event_type_filter: Option<&EventTypeFilter>,
        grpc_tx: &SubscribeUpdateTransactionInfo,
        signature: Signature,
        slot: Option<u64>,
        block_time: Option<Timestamp>,
        recv_us: i64,
        accounts: &[Pubkey],
        bot_wallet: Option<Pubkey>,
        transaction_index: Option<u64>,
        swap_cu_parse_config: Option<&SwapCuParseConfig>,
        callback: &mut impl FnMut(DexEvent),
    ) -> anyhow::Result<()> {
        let Some(message) = grpc_tx.transaction.as_ref().and_then(|tx| tx.message.as_ref()) else {
            return Ok(());
        };
        let (inner_instructions, log_messages) = grpc_tx
            .meta
            .as_ref()
            .map(|meta| (meta.inner_instructions.as_slice(), meta.log_messages.as_slice()))
            .unwrap_or_default();
        Self::parse_instruction_events_from_grpc_transaction(
            protocols,
            event_type_filter,
            &message.instructions,
            signature,
            slot,
            block_time,
            recv_us,
            accounts,
            inner_instructions,
            log_messages,
            bot_wallet,
            transaction_index,
            swap_cu_parse_config,
            callback,
        )
    }

    /// Collect all DEX events parsed from one gRPC transaction without reordering them.
    #[allow(clippy::too_many_arguments)]
    pub async fn parse_grpc_transaction_to_events(
        protocols: &[Protocol],
        event_type_filter: Option<&EventTypeFilter>,
        grpc_tx: SubscribeUpdateTransactionInfo,
        signature: Signature,
        slot: Option<u64>,
        block_time: Option<Timestamp>,
        recv_us: i64,
        bot_wallet: Option<Pubkey>,
        transaction_index: Option<u64>,
        swap_cu_parse_config: Option<&SwapCuParseConfig>,
        tx_exec_meta_audit: bool,
    ) -> anyhow::Result<Option<TxDexEvents>> {
        let block_unix_timestamp = block_time.as_ref().map(|time| time.seconds);
        let accounts = Self::grpc_account_keys(&grpc_tx)?;
        let has_jito_tip = Self::grpc_transaction_has_jito_tip(&grpc_tx, &accounts);
        let tx_exec_meta =
            tx_exec_meta_audit.then(|| Self::collect_tx_exec_meta(&grpc_tx, &accounts));
        let execution_status = match grpc_tx.meta.as_ref() {
            Some(meta) if meta.err.is_none() => TxExecutionStatus::Success,
            Some(_) => TxExecutionStatus::Failed,
            None => TxExecutionStatus::Unknown,
        };
        let raw_dex_instructions =
            Self::collect_grpc_raw_dex_instructions(protocols, &grpc_tx, &accounts);
        let mut events = Vec::new();

        Self::parse_grpc_transaction_inner(
            protocols,
            event_type_filter,
            &grpc_tx,
            signature,
            slot,
            block_time,
            recv_us,
            &accounts,
            bot_wallet,
            transaction_index,
            swap_cu_parse_config,
            &mut |event| events.push(event),
        )?;
        if events.is_empty() && raw_dex_instructions.is_empty() {
            return Ok(None);
        }
        let is_arb = Self::is_arb_inner_swap_events(&events);
        let tx_kind = Self::swap_kind_from_is_arb(is_arb, &events);
        let (compute_unit_price_micro_lamports, compute_unit_limit, compute_unit_price_set) =
            Self::summarize_compute_budget(&events);

        Ok(Some(TxDexEvents {
            signature,
            slot: slot.unwrap_or(0),
            block_time: block_unix_timestamp,
            transaction_index,
            entry_index: None,
            tx_index_in_entry: None,
            recv_us,
            is_arb,
            tx_kind,
            compute_unit_price_micro_lamports,
            compute_unit_limit,
            compute_unit_price_set,
            has_jito_tip,
            tx_exec_meta,
            execution_status,
            raw_dex_instructions,
            events,
        }))
    }

    /// Parse transaction from VersionedTransaction
    ///
    /// This is the entry point for parsing VersionedTransaction objects.
    /// It's used when working with RPC responses or historical data.
    #[allow(clippy::too_many_arguments)]
    pub async fn parse_instruction_events_from_versioned_transaction(
        protocols: &[Protocol],
        event_type_filter: Option<&EventTypeFilter>,
        transaction: &VersionedTransaction,
        signature: Signature,
        slot: Option<u64>,
        block_time: Option<Timestamp>,
        recv_us: i64,
        accounts: &[Pubkey],
        inner_instructions: &[InnerInstructions],
        bot_wallet: Option<Pubkey>,
        transaction_index: Option<u64>,
        _swap_cu_parse_config: Option<&SwapCuParseConfig>,
        callback: Arc<dyn Fn(DexEvent) + Send + Sync>,
    ) -> anyhow::Result<()> {
        Self::parse_versioned_instruction_events(
            protocols,
            event_type_filter,
            transaction,
            signature,
            slot,
            block_time,
            recv_us,
            accounts,
            inner_instructions,
            bot_wallet,
            transaction_index,
            &mut |event| callback(event),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_versioned_instruction_events(
        protocols: &[Protocol],
        event_type_filter: Option<&EventTypeFilter>,
        transaction: &VersionedTransaction,
        signature: Signature,
        slot: Option<u64>,
        block_time: Option<Timestamp>,
        recv_us: i64,
        accounts: &[Pubkey],
        inner_instructions: &[InnerInstructions],
        bot_wallet: Option<Pubkey>,
        transaction_index: Option<u64>,
        callback: &mut impl FnMut(DexEvent),
    ) -> anyhow::Result<()> {
        // 获取交易的指令和账户
        let compiled_instructions = transaction.message.instructions();
        let mut accounts = Cow::Borrowed(accounts);
        // 检查交易中是否包含程序
        let has_program = accounts
            .iter()
            .any(|account| Self::should_handle(protocols, event_type_filter, account));
        if has_program {
            // 解析每个指令
            for (index, instruction) in compiled_instructions.iter().enumerate() {
                if let Some(program_id) = accounts.get(instruction.program_id_index as usize) {
                    let program_id = *program_id; // 克隆程序ID，避免借用冲突
                    let inner_instructions = inner_instructions
                        .iter()
                        .find(|inner_instruction| inner_instruction.index == index as u8);
                    if Self::should_handle(protocols, event_type_filter, &program_id) {
                        let max_idx = instruction.accounts.iter().max().unwrap_or(&0);
                        // 补齐accounts(使用Pubkey::default())
                        if *max_idx as usize >= accounts.len() {
                            accounts.to_mut().resize(*max_idx as usize + 1, Pubkey::default());
                        }
                        Self::parse_events_from_instruction(
                            protocols,
                            event_type_filter,
                            instruction,
                            &accounts,
                            signature,
                            slot.unwrap_or(0),
                            block_time,
                            recv_us,
                            index as i64,
                            None,
                            bot_wallet,
                            transaction_index,
                            inner_instructions,
                            callback,
                        )?;
                    }
                    // Immediately process inner instructions for correct ordering
                    if let Some(inner_instructions) = inner_instructions {
                        for (inner_index, inner_instruction) in
                            inner_instructions.instructions.iter().enumerate()
                        {
                            Self::parse_events_from_instruction(
                                protocols,
                                event_type_filter,
                                &inner_instruction.instruction,
                                &accounts,
                                signature,
                                slot.unwrap_or(0),
                                block_time,
                                recv_us,
                                index as i64,
                                Some(inner_index as i64),
                                bot_wallet,
                                transaction_index,
                                Some(&inner_instructions),
                                callback,
                            )?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Collect all DEX events parsed from one VersionedTransaction without reordering them.
    #[allow(clippy::too_many_arguments)]
    pub async fn parse_versioned_transaction_to_events(
        protocols: &[Protocol],
        event_type_filter: Option<&EventTypeFilter>,
        transaction: &VersionedTransaction,
        signature: Signature,
        slot: Option<u64>,
        block_time: Option<Timestamp>,
        recv_us: i64,
        accounts: &[Pubkey],
        inner_instructions: &[InnerInstructions],
        bot_wallet: Option<Pubkey>,
        transaction_index: Option<u64>,
        entry_index: Option<u64>,
        tx_index_in_entry: Option<u64>,
        _swap_cu_parse_config: Option<&SwapCuParseConfig>,
    ) -> anyhow::Result<Option<TxDexEvents>> {
        let block_unix_timestamp = block_time.as_ref().map(|time| time.seconds);
        let has_jito_tip =
            Self::versioned_transaction_has_jito_tip(transaction, accounts, inner_instructions);
        let raw_dex_instructions = Self::collect_versioned_raw_dex_instructions(
            protocols,
            transaction,
            accounts,
            inner_instructions,
        );
        let mut events = Vec::new();

        Self::parse_versioned_instruction_events(
            protocols,
            event_type_filter,
            transaction,
            signature,
            slot,
            block_time,
            recv_us,
            accounts,
            inner_instructions,
            bot_wallet,
            transaction_index,
            &mut |event| events.push(event),
        )?;
        if events.is_empty() && raw_dex_instructions.is_empty() {
            return Ok(None);
        }
        let is_arb = Self::is_arb_inner_swap_events(&events);
        let tx_kind = Self::swap_kind_from_is_arb(is_arb, &events);
        let (compute_unit_price_micro_lamports, compute_unit_limit, compute_unit_price_set) =
            Self::summarize_compute_budget(&events);

        Ok(Some(TxDexEvents {
            signature,
            slot: slot.unwrap_or(0),
            block_time: block_unix_timestamp,
            transaction_index,
            entry_index,
            tx_index_in_entry,
            recv_us,
            is_arb,
            tx_kind,
            compute_unit_price_micro_lamports,
            compute_unit_limit,
            compute_unit_price_set,
            has_jito_tip,
            tx_exec_meta: None,
            execution_status: TxExecutionStatus::Unknown,
            raw_dex_instructions,
            events,
        }))
    }

    fn collect_grpc_raw_dex_instructions(
        protocols: &[Protocol],
        grpc_tx: &SubscribeUpdateTransactionInfo,
        accounts: &[Pubkey],
    ) -> Vec<ResolvedDexInstruction> {
        let Some(transaction) = grpc_tx.transaction.as_ref() else {
            return Vec::new();
        };
        let Some(message) = transaction.message.as_ref() else {
            return Vec::new();
        };
        let mut resolved = Vec::new();
        for (outer_index, instruction) in message.instructions.iter().enumerate() {
            if let Some(item) = Self::resolve_raw_dex_instruction(
                protocols,
                instruction.program_id_index,
                &instruction.accounts,
                &instruction.data,
                accounts,
                outer_index as u32,
                None,
                None,
            ) {
                resolved.push(item);
            }
            let Some(inner_group) = grpc_tx.meta.as_ref().and_then(|meta| {
                meta.inner_instructions.iter().find(|group| group.index == outer_index as u32)
            }) else {
                continue;
            };
            for (inner_index, instruction) in inner_group.instructions.iter().enumerate() {
                if let Some(item) = Self::resolve_raw_dex_instruction(
                    protocols,
                    instruction.program_id_index,
                    &instruction.accounts,
                    &instruction.data,
                    accounts,
                    outer_index as u32,
                    Some(inner_index as u32),
                    instruction.stack_height,
                ) {
                    resolved.push(item);
                }
            }
        }
        resolved
    }

    fn collect_versioned_raw_dex_instructions(
        protocols: &[Protocol],
        transaction: &VersionedTransaction,
        accounts: &[Pubkey],
        inner_instructions: &[InnerInstructions],
    ) -> Vec<ResolvedDexInstruction> {
        let mut resolved = Vec::new();
        for (outer_index, instruction) in transaction.message.instructions().iter().enumerate() {
            if let Some(item) = Self::resolve_raw_dex_instruction(
                protocols,
                u32::from(instruction.program_id_index),
                &instruction.accounts,
                &instruction.data,
                accounts,
                outer_index as u32,
                None,
                None,
            ) {
                resolved.push(item);
            }
            let Some(inner_group) =
                inner_instructions.iter().find(|group| usize::from(group.index) == outer_index)
            else {
                continue;
            };
            for (inner_index, instruction) in inner_group.instructions.iter().enumerate() {
                let compiled = &instruction.instruction;
                if let Some(item) = Self::resolve_raw_dex_instruction(
                    protocols,
                    u32::from(compiled.program_id_index),
                    &compiled.accounts,
                    &compiled.data,
                    accounts,
                    outer_index as u32,
                    Some(inner_index as u32),
                    instruction.stack_height,
                ) {
                    resolved.push(item);
                }
            }
        }
        resolved
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_raw_dex_instruction<I: Copy + Into<u32>>(
        protocols: &[Protocol],
        program_id_index: u32,
        account_indices: &[I],
        data: &[u8],
        accounts: &[Pubkey],
        outer_index: u32,
        inner_index: Option<u32>,
        stack_height: Option<u32>,
    ) -> Option<ResolvedDexInstruction> {
        let program_id = accounts.get(usize::try_from(program_id_index).ok()?).copied()?;
        let protocol = EventDispatcher::match_protocol_by_program_id(&program_id)?;
        if !protocols.contains(&protocol) {
            return None;
        }
        let account_indices = account_indices.iter().copied().map(Into::into).collect::<Vec<_>>();
        let instruction_accounts = account_indices
            .iter()
            .map(|index| accounts.get(usize::try_from(*index).ok()?).copied())
            .collect::<Option<Vec<_>>>()?;
        Some(ResolvedDexInstruction {
            program_id,
            accounts: instruction_accounts,
            account_indices,
            data: data.to_vec(),
            outer_index,
            inner_index,
            stack_height,
        })
    }

    fn collect_tx_exec_meta(
        grpc_tx: &SubscribeUpdateTransactionInfo,
        accounts: &[Pubkey],
    ) -> TxExecutionMetaAudit {
        let mut audit = TxExecutionMetaAudit::default();
        let Some(transaction) = grpc_tx.transaction.as_ref() else {
            audit.parse_errors.push("transaction missing".to_string());
            return audit;
        };
        let Some(_) = transaction.message.as_ref() else {
            audit.parse_errors.push("transaction message missing".to_string());
            return audit;
        };
        let Some(meta) = grpc_tx.meta.as_ref() else {
            audit.parse_errors.push("transaction meta missing".to_string());
            return audit;
        };
        let mut indices = HashSet::new();
        indices.extend(meta.pre_token_balances.iter().map(|balance| balance.account_index));
        indices.extend(meta.post_token_balances.iter().map(|balance| balance.account_index));
        for account_index in indices {
            let pre = meta
                .pre_token_balances
                .iter()
                .find(|balance| balance.account_index == account_index);
            let post = meta
                .post_token_balances
                .iter()
                .find(|balance| balance.account_index == account_index);
            let template = post.or(pre);
            let Some(template) = template else {
                continue;
            };
            let parse_amount = |side: &str,
                                balance: Option<
                &yellowstone_grpc_proto::solana::storage::confirmed_block::TokenBalance,
            >,
                                errors: &mut Vec<String>|
             -> Option<u64> {
                let balance = balance?;
                let Some(ui_amount) = balance.ui_token_amount.as_ref() else {
                    errors.push(format!(
                        "token balance ui amount missing: account_index={account_index} side={side}"
                    ));
                    return None;
                };
                let amount = ui_amount.amount.as_str();
                match amount.parse::<u64>() {
                    Ok(value) => Some(value),
                    Err(error) => {
                        errors.push(format!(
                            "token balance amount parse failed: account_index={account_index} side={side} amount={amount} error={error}"
                        ));
                        None
                    }
                }
            };
            audit.token_balance_changes.push(TxTokenBalanceChange {
                account_index,
                account: accounts
                    .get(usize::try_from(account_index).unwrap_or(usize::MAX))
                    .copied(),
                mint: template.mint.clone(),
                owner: template.owner.clone(),
                program_id: template.program_id.clone(),
                decimals: template
                    .ui_token_amount
                    .as_ref()
                    .map(|amount| amount.decimals)
                    .unwrap_or_default(),
                pre_amount: parse_amount("pre", pre, &mut audit.parse_errors),
                post_amount: parse_amount("post", post, &mut audit.parse_errors),
            });
        }
        audit
    }

    fn grpc_transaction_has_jito_tip(
        grpc_tx: &SubscribeUpdateTransactionInfo,
        accounts: &[Pubkey],
    ) -> bool {
        let Some(transaction) = grpc_tx.transaction.as_ref() else {
            return false;
        };
        let Some(message) = transaction.message.as_ref() else {
            return false;
        };
        if message.instructions.iter().any(|instruction| {
            Self::is_system_transfer_to_jito(
                instruction.program_id_index as usize,
                &instruction.accounts,
                &instruction.data,
                accounts,
            )
        }) {
            return true;
        }
        grpc_tx.meta.as_ref().is_some_and(|meta| {
            meta.inner_instructions.iter().any(|group| {
                group.instructions.iter().any(|instruction| {
                    Self::is_system_transfer_to_jito(
                        instruction.program_id_index as usize,
                        &instruction.accounts,
                        &instruction.data,
                        accounts,
                    )
                })
            })
        })
    }

    fn versioned_transaction_has_jito_tip(
        transaction: &VersionedTransaction,
        accounts: &[Pubkey],
        inner_instructions: &[InnerInstructions],
    ) -> bool {
        if transaction.message.instructions().iter().any(|instruction| {
            Self::is_system_transfer_to_jito(
                instruction.program_id_index as usize,
                &instruction.accounts,
                &instruction.data,
                accounts,
            )
        }) {
            return true;
        }
        inner_instructions.iter().any(|group| {
            group.instructions.iter().any(|instruction| {
                Self::is_system_transfer_to_jito(
                    instruction.instruction.program_id_index as usize,
                    &instruction.instruction.accounts,
                    &instruction.instruction.data,
                    accounts,
                )
            })
        })
    }

    // ================================================================================================
    // gRPC Transaction Processing
    // ================================================================================================

    /// Parse instruction events from gRPC transaction format
    ///
    /// Iterates through all instructions in a gRPC transaction, checks if they should be handled,
    /// and delegates to instruction-level parsing for both outer and inner instructions.
    #[allow(clippy::too_many_arguments)]
    fn parse_instruction_events_from_grpc_transaction(
        protocols: &[Protocol],
        event_type_filter: Option<&EventTypeFilter>,
        compiled_instructions: &[yellowstone_grpc_proto::prelude::CompiledInstruction],
        signature: Signature,
        slot: Option<u64>,
        block_time: Option<Timestamp>,
        recv_us: i64,
        accounts: &[Pubkey],
        all_inner_instructions: &[yellowstone_grpc_proto::prelude::InnerInstructions],
        log_messages: &[String],
        bot_wallet: Option<Pubkey>,
        transaction_index: Option<u64>,
        swap_cu_parse_config: Option<&SwapCuParseConfig>,
        callback: &mut impl FnMut(DexEvent),
    ) -> anyhow::Result<()> {
        // 获取交易的指令和账户
        let mut accounts = Cow::Borrowed(accounts);
        // 检查交易中是否包含程序
        let has_program = accounts
            .iter()
            .any(|account| Self::should_handle(protocols, event_type_filter, account));
        if has_program {
            // 解析每个指令
            let mut program_data_index: Option<ProgramDataIndex> = None;
            let mut swap_cu_index: Option<SwapCuIndex> = None;
            for (index, instruction) in compiled_instructions.iter().enumerate() {
                if let Some(program_id) = accounts.get(instruction.program_id_index as usize) {
                    let program_id = *program_id; // 克隆程序ID，避免借用冲突
                    if program_data_index.is_none() && !log_messages.is_empty() {
                        if let Some(protocol) =
                            EventDispatcher::match_protocol_by_program_id(&program_id)
                        {
                            if Self::instruction_needs_program_data(&protocol, &instruction.data) {
                                program_data_index = Some(build_program_data_index(
                                    log_messages,
                                    compiled_instructions.len(),
                                    all_inner_instructions,
                                ));
                            }
                        }
                    }
                    let inner_instructions = all_inner_instructions
                        .iter()
                        .find(|inner_instruction| inner_instruction.index == index as u32);
                    let max_idx = instruction.accounts.iter().max().unwrap_or(&0);
                    // 补齐accounts(使用Pubkey::default())
                    if *max_idx as usize >= accounts.len() {
                        accounts.to_mut().resize(*max_idx as usize + 1, Pubkey::default());
                    }
                    if Self::should_handle(protocols, event_type_filter, &program_id) {
                        Self::parse_events_from_grpc_instruction(
                            protocols,
                            event_type_filter,
                            instruction.into(),
                            &accounts,
                            signature,
                            slot.unwrap_or(0),
                            block_time,
                            recv_us,
                            index as i64,
                            None,
                            bot_wallet,
                            transaction_index,
                            inner_instructions,
                            program_data_index.as_ref(),
                            swap_cu_parse_config,
                            &mut swap_cu_index,
                            log_messages,
                            compiled_instructions,
                            all_inner_instructions,
                            callback,
                        )?;
                    }
                    // Immediately process inner instructions for correct ordering
                    if let Some(inner_instructions) = inner_instructions {
                        let mut inner_events: Vec<DexEvent> =
                            Vec::with_capacity(inner_instructions.instructions.len());
                        for (inner_index, inner_instruction) in
                            inner_instructions.instructions.iter().enumerate()
                        {
                            let instruction = InstructionView::from(inner_instruction);
                            if program_data_index.is_none() && !log_messages.is_empty() {
                                if let Some(program_id) =
                                    accounts.get(instruction.program_id_index as usize)
                                {
                                    if let Some(protocol) =
                                        EventDispatcher::match_protocol_by_program_id(program_id)
                                    {
                                        if Self::instruction_needs_program_data(
                                            &protocol,
                                            &instruction.data,
                                        ) {
                                            program_data_index = Some(build_program_data_index(
                                                log_messages,
                                                compiled_instructions.len(),
                                                all_inner_instructions,
                                            ));
                                        }
                                    }
                                }
                            }
                            if let Some(inner_event) = Self::parse_event_from_grpc_instruction(
                                protocols,
                                event_type_filter,
                                instruction,
                                &accounts,
                                signature,
                                slot.unwrap_or(0),
                                block_time,
                                recv_us,
                                inner_instructions.index as i64,
                                Some(inner_index as i64),
                                bot_wallet,
                                transaction_index,
                                Some(&inner_instructions),
                                program_data_index.as_ref(),
                                swap_cu_parse_config,
                                &mut swap_cu_index,
                                log_messages,
                                compiled_instructions,
                                all_inner_instructions,
                            )? {
                                inner_events.push(inner_event);
                            }
                        }

                        for inner_event in inner_events {
                            callback(inner_event);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Parse events from gRPC instruction
    ///
    /// Core parsing logic for a single gRPC instruction. Extracts discriminator, dispatches
    /// to protocol-specific parsers, handles inner instructions, and processes swap data.
    #[allow(clippy::too_many_arguments)]
    fn parse_events_from_grpc_instruction(
        protocols: &[Protocol],
        event_type_filter: Option<&EventTypeFilter>,
        instruction: InstructionView<'_>,
        accounts: &[Pubkey],
        signature: Signature,
        slot: u64,
        block_time: Option<Timestamp>,
        recv_us: i64,
        outer_index: i64,
        inner_index: Option<i64>,
        bot_wallet: Option<Pubkey>,
        transaction_index: Option<u64>,
        inner_instructions: Option<&yellowstone_grpc_proto::prelude::InnerInstructions>,
        program_data_index: Option<&ProgramDataIndex>,
        swap_cu_parse_config: Option<&SwapCuParseConfig>,
        swap_cu_index: &mut Option<SwapCuIndex>,
        log_messages: &[String],
        compiled_instructions: &[yellowstone_grpc_proto::prelude::CompiledInstruction],
        all_inner_instructions: &[yellowstone_grpc_proto::prelude::InnerInstructions],
        callback: &mut impl FnMut(DexEvent),
    ) -> anyhow::Result<()> {
        if let Some(event) = Self::parse_event_from_grpc_instruction(
            protocols,
            event_type_filter,
            instruction,
            accounts,
            signature,
            slot,
            block_time,
            recv_us,
            outer_index,
            inner_index,
            bot_wallet,
            transaction_index,
            inner_instructions,
            program_data_index,
            swap_cu_parse_config,
            swap_cu_index,
            log_messages,
            compiled_instructions,
            all_inner_instructions,
        )? {
            callback(event);
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_event_from_grpc_instruction(
        protocols: &[Protocol],
        event_type_filter: Option<&EventTypeFilter>,
        instruction: InstructionView<'_>,
        accounts: &[Pubkey],
        signature: Signature,
        slot: u64,
        block_time: Option<Timestamp>,
        recv_us: i64,
        outer_index: i64,
        inner_index: Option<i64>,
        bot_wallet: Option<Pubkey>,
        transaction_index: Option<u64>,
        inner_instructions: Option<&yellowstone_grpc_proto::prelude::InnerInstructions>,
        program_data_index: Option<&ProgramDataIndex>,
        swap_cu_parse_config: Option<&SwapCuParseConfig>,
        swap_cu_index: &mut Option<SwapCuIndex>,
        log_messages: &[String],
        compiled_instructions: &[yellowstone_grpc_proto::prelude::CompiledInstruction],
        all_inner_instructions: &[yellowstone_grpc_proto::prelude::InnerInstructions],
    ) -> anyhow::Result<Option<DexEvent>> {
        // 添加边界检查以防止越界访问
        let program_id_index = instruction.program_id_index as usize;
        if program_id_index >= accounts.len() {
            return Ok(None);
        }
        let program_id = accounts[program_id_index];
        if !Self::should_handle(protocols, event_type_filter, &program_id) {
            return Ok(None);
        }

        let is_cu_program = EventDispatcher::is_compute_budget_program(&program_id);

        let disc_len = match program_id {
            RAYDIUM_AMM_V4_PROGRAM_ID => 1,
            _ => 8,
        };

        // 检查指令数据长度（至少需要 disc_len 字节的 discriminator）
        if !is_cu_program && instruction.data.len() < disc_len {
            return Ok(None);
        }
        // 创建元数据
        let timestamp = block_time.unwrap_or(Timestamp { seconds: 0, nanos: 0 });
        let block_time_ms = timestamp.seconds * 1000 + (timestamp.nanos as i64) / 1_000_000;
        let metadata = EventMetadata::new(
            signature,
            slot,
            timestamp.seconds,
            block_time_ms,
            Default::default(), // protocol will be set by dispatcher
            Default::default(), // event_type will be set by dispatcher
            program_id,
            outer_index,
            inner_index,
            recv_us,
            transaction_index,
        );

        if is_cu_program {
            return Ok(EventDispatcher::dispatch_compute_budget_instruction(
                &instruction.data,
                metadata.clone(),
            ));
        }

        // 使用 EventDispatcher 匹配协议
        let protocol = match EventDispatcher::match_protocol_by_program_id(&program_id) {
            Some(p) => p,
            None => return Ok(None),
        };

        // 提取 discriminator 和数据
        let instruction_discriminator = &instruction.data[..disc_len];
        let instruction_data = &instruction.data[disc_len..];

        // 构建账户公钥列表
        let account_pubkeys: Vec<Pubkey> = instruction
            .accounts
            .iter()
            .filter_map(|&idx| accounts.get(idx as usize).copied())
            .collect();

        // 使用 EventDispatcher 解析 instruction 事件
        let mut event = match EventDispatcher::dispatch_instruction(
            protocol.clone(),
            instruction_discriminator,
            instruction_data,
            &account_pubkeys,
            metadata.clone(),
        ) {
            Some(e) => e,
            None => return Ok(None),
        };

        if let Some(config) = swap_cu_parse_config.filter(|config| {
            config.enabled
                && config.is_target_swap(&protocol, &program_id, &instruction.data)
                && !log_messages.is_empty()
        }) {
            if swap_cu_index.is_none() {
                *swap_cu_index = Some(build_swap_cu_index(
                    config,
                    log_messages,
                    compiled_instructions,
                    accounts,
                    all_inner_instructions,
                ));
            }
            if let Some(cu) =
                swap_cu_index.as_ref().and_then(|index| index.get(outer_index, inner_index))
            {
                event.metadata_mut().swap_compute_units = Some(cu);
            }
        }

        enrich_event_from_program_data(
            &mut event,
            &protocol,
            program_data_index,
            outer_index,
            inner_index,
        );

        // 处理 inner instructions（默认不提取 swap_data，保持 metadata.swap_data=None）
        let mut inner_instruction_event: Option<DexEvent> = None;
        if let Some(inner_instructions_ref) = inner_instructions {
            let start_idx = inner_index
                .and_then(|i| if i >= 0 { Some((i as usize).saturating_add(1)) } else { None })
                .unwrap_or(0);
            for inner_instruction in inner_instructions_ref.instructions.iter().skip(start_idx) {
                let inner_data = &inner_instruction.data;
                // 检查长度（需要 16 字节的 discriminator）
                if inner_data.len() < 16 {
                    continue;
                }
                let inner_discriminator = &inner_data[..16];
                let inner_instruction_data = &inner_data[16..];
                if let Some(inner_event) = EventDispatcher::dispatch_inner_instruction(
                    protocol.clone(),
                    inner_discriminator,
                    inner_instruction_data,
                    metadata.clone(),
                ) {
                    inner_instruction_event = Some(inner_event);
                    break;
                }
            }
        }

        // 特殊处理: PumpFun MIGRATE 指令需要 inner instruction data
        if matches!(protocol, Protocol::PumpFun) {
            const PUMPFUN_MIGRATE_IX: &[u8] = &[155, 234, 231, 146, 236, 158, 162, 30];
            if instruction_discriminator == PUMPFUN_MIGRATE_IX && inner_instruction_event.is_none()
            {
                return Ok(None);
            }
        }

        // 合并事件
        if let Some(inner_instruction_event) = inner_instruction_event {
            merge(&mut event, inner_instruction_event);
        }

        // 设置处理时间（使用高性能时钟）
        event.metadata_mut().handle_us = elapsed_micros_since(recv_us);
        event = Self::process_event(event, bot_wallet);
        Ok(Some(event))
    }

    // ================================================================================================
    // Standard Instruction Processing
    // ================================================================================================

    /// Parse events from standard Solana instruction
    ///
    /// Similar to gRPC instruction parsing but works with standard CompiledInstruction format.
    /// Used when parsing VersionedTransaction or RPC data.
    #[allow(clippy::too_many_arguments)]
    fn parse_events_from_instruction(
        protocols: &[Protocol],
        event_type_filter: Option<&EventTypeFilter>,
        instruction: &CompiledInstruction,
        accounts: &[Pubkey],
        signature: Signature,
        slot: u64,
        block_time: Option<Timestamp>,
        recv_us: i64,
        outer_index: i64,
        inner_index: Option<i64>,
        bot_wallet: Option<Pubkey>,
        transaction_index: Option<u64>,
        inner_instructions: Option<&InnerInstructions>,
        callback: &mut impl FnMut(DexEvent),
    ) -> anyhow::Result<()> {
        // 添加边界检查以防止越界访问
        let program_id_index = instruction.program_id_index as usize;
        if program_id_index >= accounts.len() {
            return Ok(());
        }
        let program_id = accounts[program_id_index];
        if !Self::should_handle(protocols, event_type_filter, &program_id) {
            return Ok(());
        }

        let is_cu_program = EventDispatcher::is_compute_budget_program(&program_id);

        let disc_len = match program_id {
            RAYDIUM_AMM_V4_PROGRAM_ID => 1,
            _ => 8,
        };

        // 检查指令数据长度（至少需要 8 字节的 discriminator）
        if !is_cu_program && instruction.data.len() < disc_len {
            return Ok(());
        }

        // 创建元数据
        let timestamp = block_time.unwrap_or(Timestamp { seconds: 0, nanos: 0 });
        let block_time_ms = timestamp.seconds * 1000 + (timestamp.nanos as i64) / 1_000_000;
        let metadata = EventMetadata::new(
            signature,
            slot,
            timestamp.seconds,
            block_time_ms,
            Default::default(), // protocol will be set by dispatcher
            Default::default(), // event_type will be set by dispatcher
            program_id,
            outer_index,
            inner_index,
            recv_us,
            transaction_index,
        );

        if is_cu_program {
            if let Some(event) = EventDispatcher::dispatch_compute_budget_instruction(
                &instruction.data,
                metadata.clone(),
            ) {
                callback(event);
            }
            return Ok(());
        }

        // 使用 EventDispatcher 匹配协议
        let protocol = match EventDispatcher::match_protocol_by_program_id(&program_id) {
            Some(p) => p,
            None => return Ok(()),
        };

        // 提取 discriminator 和数据
        let instruction_discriminator = &instruction.data[..disc_len];
        let instruction_data = &instruction.data[disc_len..];

        // 构建账户公钥列表
        let account_pubkeys: Vec<Pubkey> = instruction
            .accounts
            .iter()
            .filter_map(|&idx| accounts.get(idx as usize).copied())
            .collect();

        // 使用 EventDispatcher 解析 instruction 事件
        let mut event = match EventDispatcher::dispatch_instruction(
            protocol.clone(),
            instruction_discriminator,
            instruction_data,
            &account_pubkeys,
            metadata.clone(),
        ) {
            Some(e) => e,
            None => return Ok(()),
        };

        // 处理 inner instructions（默认不提取 swap_data，保持 metadata.swap_data=None）
        let mut inner_instruction_event: Option<DexEvent> = None;
        if let Some(inner_instructions_ref) = inner_instructions {
            let start_idx = inner_index
                .and_then(|i| if i >= 0 { Some((i as usize).saturating_add(1)) } else { None })
                .unwrap_or(0);
            for inner_instruction in inner_instructions_ref.instructions.iter().skip(start_idx) {
                let inner_data = &inner_instruction.instruction.data;
                // 检查长度（需要 16 字节的 discriminator）
                if inner_data.len() < 16 {
                    continue;
                }
                let inner_discriminator = &inner_data[..16];
                let inner_instruction_data = &inner_data[16..];
                if let Some(inner_event) = EventDispatcher::dispatch_inner_instruction(
                    protocol.clone(),
                    inner_discriminator,
                    inner_instruction_data,
                    metadata.clone(),
                ) {
                    inner_instruction_event = Some(inner_event);
                    break;
                }
            }
        }

        // 特殊处理: PumpFun MIGRATE 指令需要 inner instruction data
        if matches!(protocol, Protocol::PumpFun) {
            const PUMPFUN_MIGRATE_IX: &[u8] = &[155, 234, 231, 146, 236, 158, 162, 30];
            if instruction_discriminator == PUMPFUN_MIGRATE_IX && inner_instruction_event.is_none()
            {
                return Ok(());
            }
        }

        // 合并事件
        if let Some(inner_instruction_event) = inner_instruction_event {
            merge(&mut event, inner_instruction_event);
        }

        // 设置处理时间（使用高性能时钟）
        event.metadata_mut().handle_us = elapsed_micros_since(recv_us);
        event = Self::process_event(event, bot_wallet);
        callback(event);

        Ok(())
    }

    // ================================================================================================
    // Helper Functions
    // ================================================================================================

    /// Check if instruction should be processed based on protocol filter
    ///
    /// Determines whether a program_id matches any of the protocols we're interested in.
    fn should_handle(
        protocols: &[Protocol],
        _event_type_filter: Option<&EventTypeFilter>,
        program_id: &Pubkey,
    ) -> bool {
        // 使用 EventDispatcher 来匹配协议
        if let Some(protocol) = EventDispatcher::match_protocol_by_program_id(program_id) {
            protocols.contains(&protocol)
        } else if EventDispatcher::is_compute_budget_program(program_id) {
            return true;
        } else {
            false
        }
    }

    #[inline]
    fn extract_swap_mints(event: &DexEvent) -> Option<(Pubkey, Pubkey)> {
        let (from_mint, to_mint) = match event {
            DexEvent::PumpSwapBuyEvent(e) => (e.quote_mint, e.base_mint),
            DexEvent::PumpSwapBuyExactQuoteInEvent(e) => (e.quote_mint, e.base_mint),
            DexEvent::PumpSwapSellEvent(e) => (e.base_mint, e.quote_mint),
            DexEvent::PancakeSwapSwapV2Event(e) => (e.input_mint, e.output_mint),
            DexEvent::BonkTradeEvent(e) => match e.trade_direction {
                crate::streaming::event_parser::protocols::bonk::types::TradeDirection::Buy => {
                    (e.quote_token_mint, e.base_token_mint)
                }
                crate::streaming::event_parser::protocols::bonk::types::TradeDirection::Sell => {
                    (e.base_token_mint, e.quote_token_mint)
                }
            },
            DexEvent::RaydiumCpmmSwapEvent(e) => (e.input_token_mint, e.output_token_mint),
            DexEvent::RaydiumClmmSwapV2Event(e) => (e.input_vault_mint, e.output_vault_mint),
            DexEvent::MeteoraDlmmSwapEvent(e) => {
                if e.swap_for_y {
                    (e.token_x_mint?, e.token_y_mint?)
                } else {
                    (e.token_y_mint?, e.token_x_mint?)
                }
            }
            DexEvent::MeteoraDlmmSwap2Event(e) => {
                if e.swap_for_y {
                    (e.token_x_mint?, e.token_y_mint?)
                } else {
                    (e.token_y_mint?, e.token_x_mint?)
                }
            }
            DexEvent::WhirlpoolSwapV2Event(e) => {
                if e.a_to_b {
                    (e.token_mint_a, e.token_mint_b)
                } else {
                    (e.token_mint_b, e.token_mint_a)
                }
            }
            DexEvent::MeteoraDammV2SwapEvent(e) => (e.token_a_mint, e.token_b_mint),
            DexEvent::MeteoraDammV2Swap2Event(e) => (e.token_a_mint, e.token_b_mint),
            _ => return None,
        };
        if from_mint == Pubkey::default() || to_mint == Pubkey::default() {
            None
        } else {
            Some((from_mint, to_mint))
        }
    }

    /// 提取一笔 swap 事件对应的池子地址。
    #[inline]
    fn extract_swap_pool_id(event: &DexEvent) -> Option<Pubkey> {
        match event {
            DexEvent::PumpSwapBuyEvent(e) => Some(e.pool),
            DexEvent::PumpSwapBuyExactQuoteInEvent(e) => Some(e.pool),
            DexEvent::PumpSwapSellEvent(e) => Some(e.pool),
            DexEvent::PancakeSwapSwapEvent(e) => Some(e.pool_state),
            DexEvent::PancakeSwapSwapV2Event(e) => Some(e.pool_state),
            DexEvent::RaydiumClmmSwapEvent(e) => Some(e.pool_state),
            DexEvent::RaydiumClmmSwapV2Event(e) => Some(e.pool_state),
            DexEvent::MeteoraDammV2SwapEvent(e) => Some(e.pool),
            DexEvent::MeteoraDammV2Swap2Event(e) => Some(e.pool),
            DexEvent::RaydiumCpmmSwapEvent(e) => Some(e.pool_state),
            DexEvent::MeteoraDlmmSwapEvent(e) => Some(e.lb_pair),
            DexEvent::MeteoraDlmmSwap2Event(e) => Some(e.lb_pair),
            DexEvent::WhirlpoolSwapEvent(e) => Some(e.whirlpool),
            DexEvent::WhirlpoolSwapV2Event(e) => Some(e.whirlpool),
            DexEvent::BonkTradeEvent(e) => Some(e.pool_state),
            _ => None,
        }
    }

    /// 判断一个 mint 是否属于稳定币锚点（Route 检测中排除）。
    #[inline]
    fn is_stablecoin_mint(mint: &Pubkey) -> bool {
        STABLECOIN_MINTS.contains(mint)
    }

    /// 整 tx 级"同 mint 跨池/路由拆单"检测：同一非稳定币 mint 出现在 ≥2 个不同池子。
    /// 与 `is_arb_inner_swap_events` 不同，这里跨指令边界聚合，不需要 leg 链。
    fn is_multi_pool_route_events(events: &[DexEvent]) -> bool {
        // mint -> 触碰该 mint 的 distinct pool 集合
        let mut mint_pools: HashMap<Pubkey, HashSet<[u8; 32]>> = HashMap::new();
        for event in events {
            let Some(pool_id) = Self::extract_swap_pool_id(event) else {
                continue;
            };
            let Some((mint_a, mint_b)) = Self::extract_swap_mints(event) else {
                continue;
            };
            for mint in [mint_a, mint_b] {
                if Self::is_stablecoin_mint(&mint) {
                    continue;
                }
                let pools = mint_pools.entry(mint).or_default();
                pools.insert(pool_id.to_bytes());
                if pools.len() >= 2 {
                    return true;
                }
            }
        }
        false
    }

    /// 由 is_arb 布尔与整 tx 事件推导交易级 swap 形态（Arb > Route > SimpleSwap）。
    #[inline]
    fn swap_kind_from_is_arb(is_arb: bool, events: &[DexEvent]) -> TxSwapKind {
        if is_arb {
            TxSwapKind::Arb
        } else if Self::is_multi_pool_route_events(events) {
            TxSwapKind::Route
        } else {
            TxSwapKind::SimpleSwap
        }
    }

    #[inline]
    fn extract_swap_token_accounts(event: &DexEvent) -> Option<(Pubkey, Pubkey)> {
        let (from_account, to_account) = match event {
            DexEvent::BonkTradeEvent(e) => match e.trade_direction {
                crate::streaming::event_parser::protocols::bonk::types::TradeDirection::Buy => {
                    (e.user_quote_token, e.user_base_token)
                }
                crate::streaming::event_parser::protocols::bonk::types::TradeDirection::Sell => {
                    (e.user_base_token, e.user_quote_token)
                }
            },
            DexEvent::PumpSwapBuyEvent(e) => {
                (e.user_quote_token_account, e.user_base_token_account)
            }
            DexEvent::PumpSwapBuyExactQuoteInEvent(e) => {
                (e.user_quote_token_account, e.user_base_token_account)
            }
            DexEvent::PumpSwapSellEvent(e) => {
                (e.user_base_token_account, e.user_quote_token_account)
            }
            DexEvent::PancakeSwapSwapEvent(e) => (e.input_token_account, e.output_token_account),
            DexEvent::PancakeSwapSwapV2Event(e) => (e.input_token_account, e.output_token_account),
            DexEvent::RaydiumAmmV4SwapEvent(e) => {
                (e.user_source_token_account, e.user_destination_token_account)
            }
            DexEvent::RaydiumClmmSwapEvent(e) => (e.input_token_account, e.output_token_account),
            DexEvent::RaydiumClmmSwapV2Event(e) => (e.input_token_account, e.output_token_account),
            DexEvent::MeteoraDlmmSwapEvent(e) => (e.user_token_in?, e.user_token_out?),
            DexEvent::MeteoraDlmmSwap2Event(e) => (e.user_token_in?, e.user_token_out?),
            DexEvent::WhirlpoolSwapEvent(e) => {
                if e.a_to_b {
                    (e.token_owner_account_a, e.token_owner_account_b)
                } else {
                    (e.token_owner_account_b, e.token_owner_account_a)
                }
            }
            DexEvent::WhirlpoolSwapV2Event(e) => {
                if e.a_to_b {
                    (e.token_owner_account_a, e.token_owner_account_b)
                } else {
                    (e.token_owner_account_b, e.token_owner_account_a)
                }
            }
            _ => return None,
        };
        if from_account == Pubkey::default() || to_account == Pubkey::default() {
            None
        } else {
            Some((from_account, to_account))
        }
    }

    #[inline]
    fn is_arb_inner_swap_events(events: &[DexEvent]) -> bool {
        let mut account_legs: Vec<AccountLeg> = Vec::new();
        let mut mint_legs: Vec<MintLeg> = Vec::new();
        let mut outer_index = None;

        for event in events {
            let metadata = event.metadata();
            if outer_index != Some(metadata.outer_index) {
                if Self::is_arb_account_segment(&account_legs)
                    || Self::is_arb_mint_segment(&mint_legs)
                {
                    return true;
                }
                account_legs.clear();
                mint_legs.clear();
                outer_index = Some(metadata.outer_index);
            }

            if metadata.inner_index.is_none() {
                continue;
            }

            if let Some((from_account, to_account)) = Self::extract_swap_token_accounts(event) {
                if Self::is_arb_mint_segment(&mint_legs) {
                    return true;
                }
                mint_legs.clear();

                let next_leg = AccountLeg { from_account, to_account };

                if let Some(last_leg) = account_legs.last() {
                    if last_leg.to_account != next_leg.from_account {
                        if Self::is_arb_account_segment(&account_legs) {
                            return true;
                        }
                        account_legs.clear();
                    }
                }
                account_legs.push(next_leg);
                continue;
            }

            if Self::is_arb_account_segment(&account_legs) {
                return true;
            }
            account_legs.clear();

            let Some((from_mint, to_mint)) = Self::extract_swap_mints(event) else {
                if Self::is_arb_mint_segment(&mint_legs) {
                    return true;
                }
                mint_legs.clear();
                continue;
            };

            let next_leg = MintLeg { from_mint, to_mint };

            if let Some(last_leg) = mint_legs.last() {
                if last_leg.to_mint != next_leg.from_mint {
                    if Self::is_arb_mint_segment(&mint_legs) {
                        return true;
                    }
                    mint_legs.clear();
                }
            }
            mint_legs.push(next_leg);
        }

        Self::is_arb_account_segment(&account_legs) || Self::is_arb_mint_segment(&mint_legs)
    }

    #[inline]
    fn is_arb_mint_segment(legs: &[MintLeg]) -> bool {
        legs.len() >= 2
            && legs
                .first()
                .zip(legs.last())
                .is_some_and(|(first, last)| first.from_mint == last.to_mint)
    }

    #[inline]
    fn is_arb_account_segment(legs: &[AccountLeg]) -> bool {
        legs.len() >= 2
            && legs
                .first()
                .zip(legs.last())
                .is_some_and(|(first, last)| first.from_account == last.to_account)
    }

    fn instruction_needs_program_data(protocol: &Protocol, data: &[u8]) -> bool {
        if data.len() < 8 {
            return false;
        }
        match protocol {
            // Liquidity-only transactions also need their invocation-scoped execution logs.
            Protocol::RaydiumCpmm | Protocol::RaydiumClmm | Protocol::Whirlpool => true,
            Protocol::PancakeSwap => crate::streaming::event_parser::protocols::pancakeswap::parser::is_pancakeswap_swap_instruction(&data[..8]),
            _ => false,
        }
    }

    // ================================================================================================
    // Event Post-Processing
    // ================================================================================================

    /// Process and enrich parsed event with additional context
    ///
    /// Handles protocol-specific post-processing:
    /// - PumpFun: Tracks dev addresses and marks dev trades
    /// - PumpSwap: Fills swap data amounts
    /// - Bonk: Tracks pool creators and marks dev trades
    /// - General: Marks bot wallet trades
    fn process_event(event: DexEvent, bot_wallet: Option<Pubkey>) -> DexEvent {
        let signature = event.metadata().signature; // Copy the signature to avoid borrowing issues
        match event {
            DexEvent::PumpFunCreateTokenEvent(token_info) => {
                add_dev_address(&signature, token_info.user);
                if token_info.creator != Pubkey::default() && token_info.creator != token_info.user
                {
                    add_dev_address(&signature, token_info.creator);
                }
                DexEvent::PumpFunCreateTokenEvent(token_info)
            }
            DexEvent::PumpFunCreateV2TokenEvent(token_info) => {
                add_dev_address(&signature, token_info.user);
                if token_info.creator != Pubkey::default() && token_info.creator != token_info.user
                {
                    add_dev_address(&signature, token_info.creator);
                }
                DexEvent::PumpFunCreateV2TokenEvent(token_info)
            }
            DexEvent::PumpFunTradeEvent(mut trade_info) => {
                trade_info.is_dev_create_token_trade =
                    is_dev_address_in_signature(&signature, &trade_info.user)
                        || is_dev_address_in_signature(&signature, &trade_info.creator);
                trade_info.is_bot = Some(trade_info.user) == bot_wallet;

                if let Some(swap_data) = trade_info.metadata.swap_data.as_mut() {
                    swap_data.from_amount = if trade_info.is_buy {
                        trade_info.sol_amount
                    } else {
                        trade_info.token_amount
                    };
                    swap_data.to_amount = if trade_info.is_buy {
                        trade_info.token_amount
                    } else {
                        trade_info.sol_amount
                    };
                }
                DexEvent::PumpFunTradeEvent(trade_info)
            }
            DexEvent::PumpSwapBuyEvent(mut trade_info) => {
                if let Some(swap_data) = trade_info.metadata.swap_data.as_mut() {
                    swap_data.from_amount = trade_info.user_quote_amount_in;
                    swap_data.to_amount = trade_info.base_amount_out;
                }
                DexEvent::PumpSwapBuyEvent(trade_info)
            }
            DexEvent::PumpSwapBuyExactQuoteInEvent(mut trade_info) => {
                if let Some(swap_data) = trade_info.metadata.swap_data.as_mut() {
                    swap_data.from_amount = if trade_info.user_quote_amount_in > 0 {
                        trade_info.user_quote_amount_in
                    } else {
                        trade_info.quote_amount_in
                    };
                    swap_data.to_amount = if trade_info.actual_base_amount_out > 0 {
                        trade_info.actual_base_amount_out
                    } else {
                        trade_info.min_base_amount_out
                    };
                }
                DexEvent::PumpSwapBuyExactQuoteInEvent(trade_info)
            }
            DexEvent::PumpSwapSellEvent(mut trade_info) => {
                if let Some(swap_data) = trade_info.metadata.swap_data.as_mut() {
                    swap_data.from_amount = trade_info.base_amount_in;
                    swap_data.to_amount = trade_info.user_quote_amount_out;
                }
                DexEvent::PumpSwapSellEvent(trade_info)
            }
            DexEvent::PancakeSwapSwapEvent(mut trade_info) => {
                if let Some(swap_data) = trade_info.metadata.swap_data.as_mut() {
                    if trade_info.amount_0 > 0 || trade_info.amount_1 > 0 {
                        if trade_info.zero_for_one {
                            swap_data.from_amount = trade_info.amount_0;
                            swap_data.to_amount = trade_info.amount_1;
                        } else {
                            swap_data.from_amount = trade_info.amount_1;
                            swap_data.to_amount = trade_info.amount_0;
                        }
                    } else {
                        swap_data.from_amount = trade_info.amount;
                    }
                }
                DexEvent::PancakeSwapSwapEvent(trade_info)
            }
            DexEvent::PancakeSwapSwapV2Event(mut trade_info) => {
                if let Some(swap_data) = trade_info.metadata.swap_data.as_mut() {
                    if trade_info.amount_0 > 0 || trade_info.amount_1 > 0 {
                        if trade_info.zero_for_one {
                            swap_data.from_amount = trade_info.amount_0;
                            swap_data.to_amount = trade_info.amount_1;
                        } else {
                            swap_data.from_amount = trade_info.amount_1;
                            swap_data.to_amount = trade_info.amount_0;
                        }
                    } else {
                        swap_data.from_amount = trade_info.amount;
                    }
                }
                DexEvent::PancakeSwapSwapV2Event(trade_info)
            }
            DexEvent::BonkPoolCreateEvent(pool_info) => {
                add_bonk_dev_address(&signature, pool_info.creator);
                DexEvent::BonkPoolCreateEvent(pool_info)
            }
            DexEvent::BonkTradeEvent(mut trade_info) => {
                trade_info.is_dev_create_token_trade =
                    is_bonk_dev_address_in_signature(&signature, &trade_info.payer);
                trade_info.is_bot = Some(trade_info.payer) == bot_wallet;
                DexEvent::BonkTradeEvent(trade_info)
            }
            DexEvent::WhirlpoolSwapEvent(mut trade_info) => {
                if let Some(swap_data) = trade_info.metadata.swap_data.as_mut() {
                    swap_data.from_amount = trade_info.input_amount;
                    swap_data.to_amount = trade_info.output_amount;
                }
                DexEvent::WhirlpoolSwapEvent(trade_info)
            }
            DexEvent::WhirlpoolSwapV2Event(mut trade_info) => {
                if let Some(swap_data) = trade_info.metadata.swap_data.as_mut() {
                    swap_data.from_amount = trade_info.input_amount;
                    swap_data.to_amount = trade_info.output_amount;
                }
                DexEvent::WhirlpoolSwapV2Event(trade_info)
            }
            _ => event,
        }
    }
}

/// 根据协议类型，从 program data 日志中提取额外字段并填充到事件中
fn enrich_event_from_program_data(
    event: &mut DexEvent,
    protocol: &Protocol,
    program_data_index: Option<&ProgramDataIndex>,
    outer_index: i64,
    inner_index: Option<i64>,
) {
    let Some(index) = program_data_index else {
        return;
    };

    let items = if let Some(inner_index) = inner_index {
        index.get_inner_all(outer_index, inner_index)
    } else {
        index.get_outer_all(outer_index)
    };
    if items.is_empty() {
        return;
    }

    for item in items {
        match protocol {
            Protocol::PancakeSwap => {
                use crate::streaming::event_parser::protocols::pancakeswap::parser::parse_swap_event_from_program_data;
                match event {
                    DexEvent::PancakeSwapSwapEvent(swap_event) => {
                        if let Some(log_data) =
                            parse_swap_event_from_program_data(item, &swap_event.pool_state)
                        {
                            swap_event.log_pool_state = log_data.pool_state;
                            swap_event.log_sender = log_data.sender;
                            swap_event.log_input_token_account = log_data.input_token_account;
                            swap_event.log_output_token_account = log_data.output_token_account;
                            swap_event.amount_0 = log_data.amount_0;
                            swap_event.transfer_fee_0 = log_data.transfer_fee_0;
                            swap_event.amount_1 = log_data.amount_1;
                            swap_event.transfer_fee_1 = log_data.transfer_fee_1;
                            swap_event.zero_for_one = log_data.zero_for_one;
                            swap_event.sqrt_price_x64 = log_data.sqrt_price_x64;
                            swap_event.liquidity = log_data.liquidity;
                            swap_event.tick = log_data.tick;
                        }
                    }
                    DexEvent::PancakeSwapSwapV2Event(swap_event) => {
                        if let Some(log_data) =
                            parse_swap_event_from_program_data(item, &swap_event.pool_state)
                        {
                            swap_event.log_pool_state = log_data.pool_state;
                            swap_event.log_sender = log_data.sender;
                            swap_event.log_input_token_account = log_data.input_token_account;
                            swap_event.log_output_token_account = log_data.output_token_account;
                            swap_event.amount_0 = log_data.amount_0;
                            swap_event.transfer_fee_0 = log_data.transfer_fee_0;
                            swap_event.amount_1 = log_data.amount_1;
                            swap_event.transfer_fee_1 = log_data.transfer_fee_1;
                            swap_event.zero_for_one = log_data.zero_for_one;
                            swap_event.sqrt_price_x64 = log_data.sqrt_price_x64;
                            swap_event.liquidity = log_data.liquidity;
                            swap_event.tick = log_data.tick;
                        }
                    }
                    _ => {}
                }
            }
            Protocol::RaydiumCpmm => {
                use crate::streaming::event_parser::protocols::raydium_cpmm::parser::parse_liquidity_state_from_program_data;
                use crate::streaming::event_parser::protocols::raydium_cpmm::parser::parse_swap_event_from_program_data;
                match event {
                    DexEvent::RaydiumCpmmDepositEvent(e) => {
                        if let Some(state) =
                            parse_liquidity_state_from_program_data(item, e.pool_state, 0)
                        {
                            e.liquidity_state = Some(state);
                        }
                    }
                    DexEvent::RaydiumCpmmWithdrawEvent(e) => {
                        if let Some(state) =
                            parse_liquidity_state_from_program_data(item, e.pool_state, 1)
                        {
                            e.liquidity_state = Some(state);
                        }
                    }
                    _ => {}
                }
                if let DexEvent::RaydiumCpmmSwapEvent(swap_event) = event {
                    if let Some(log_data) =
                        parse_swap_event_from_program_data(item, &swap_event.pool_state)
                    {
                        swap_event.input_vault_before = log_data.input_vault_before;
                        swap_event.output_vault_before = log_data.output_vault_before;
                        swap_event.input_amount = log_data.input_amount;
                        swap_event.output_amount = log_data.output_amount;
                        swap_event.input_transfer_fee = log_data.input_transfer_fee;
                        swap_event.output_transfer_fee = log_data.output_transfer_fee;
                        swap_event.base_input = log_data.base_input;
                        swap_event.trade_fee = log_data.trade_fee;
                        swap_event.creator_fee = log_data.creator_fee;
                        swap_event.creator_fee_on_input = log_data.creator_fee_on_input;
                    }
                }
            }
            Protocol::RaydiumClmm => {
                use crate::streaming::event_parser::protocols::raydium_clmm::parser::{
                    parse_execution_event_from_program_data, parse_swap_event_from_program_data,
                };
                match event {
                    DexEvent::RaydiumClmmInstructionEvent(instruction) => {
                        if let Some(execution) = parse_execution_event_from_program_data(item) {
                            instruction.execution_events.push(execution);
                        }
                    }
                    DexEvent::RaydiumClmmSwapEvent(swap_event) => {
                        if let Some(log_data) =
                            parse_swap_event_from_program_data(item, &swap_event.pool_state)
                        {
                            swap_event.sender = log_data.sender;
                            swap_event.token_account_0 = log_data.token_account_0;
                            swap_event.token_account_1 = log_data.token_account_1;
                            swap_event.amount_0 = log_data.amount_0;
                            swap_event.transfer_fee_0 = log_data.transfer_fee_0;
                            swap_event.amount_1 = log_data.amount_1;
                            swap_event.transfer_fee_1 = log_data.transfer_fee_1;
                            swap_event.zero_for_one = log_data.zero_for_one;
                            swap_event.sqrt_price_x64 = log_data.sqrt_price_x64;
                            swap_event.liquidity = log_data.liquidity;
                            swap_event.tick = log_data.tick;
                        }
                    }
                    DexEvent::RaydiumClmmSwapV2Event(swap_event) => {
                        if let Some(log_data) =
                            parse_swap_event_from_program_data(item, &swap_event.pool_state)
                        {
                            swap_event.sender = log_data.sender;
                            swap_event.token_account_0 = log_data.token_account_0;
                            swap_event.token_account_1 = log_data.token_account_1;
                            swap_event.amount_0 = log_data.amount_0;
                            swap_event.transfer_fee_0 = log_data.transfer_fee_0;
                            swap_event.amount_1 = log_data.amount_1;
                            swap_event.transfer_fee_1 = log_data.transfer_fee_1;
                            swap_event.zero_for_one = log_data.zero_for_one;
                            swap_event.sqrt_price_x64 = log_data.sqrt_price_x64;
                            swap_event.liquidity = log_data.liquidity;
                            swap_event.tick = log_data.tick;
                        }
                    }
                    _ => {}
                }
            }
            Protocol::Whirlpool => {
                use crate::streaming::event_parser::protocols::whirlpool::parser::{
                    parse_execution_event_from_program_data, parse_traded_event_from_program_data,
                };
                match event {
                    DexEvent::WhirlpoolInstructionEvent(instruction) => {
                        if let Some(execution) = parse_execution_event_from_program_data(item) {
                            instruction.execution_events.push(execution);
                        }
                    }
                    DexEvent::WhirlpoolSwapEvent(swap_event) => {
                        if let Some(log_data) =
                            parse_traded_event_from_program_data(item, &swap_event.whirlpool)
                        {
                            swap_event.a_to_b = log_data.a_to_b;
                            swap_event.pre_sqrt_price = log_data.pre_sqrt_price;
                            swap_event.post_sqrt_price = log_data.post_sqrt_price;
                            swap_event.input_amount = log_data.input_amount;
                            swap_event.output_amount = log_data.output_amount;
                            swap_event.input_transfer_fee = log_data.input_transfer_fee;
                            swap_event.output_transfer_fee = log_data.output_transfer_fee;
                            swap_event.lp_fee = log_data.lp_fee;
                            swap_event.protocol_fee = log_data.protocol_fee;
                        }
                    }
                    DexEvent::WhirlpoolSwapV2Event(swap_event) => {
                        if let Some(log_data) =
                            parse_traded_event_from_program_data(item, &swap_event.whirlpool)
                        {
                            swap_event.a_to_b = log_data.a_to_b;
                            swap_event.pre_sqrt_price = log_data.pre_sqrt_price;
                            swap_event.post_sqrt_price = log_data.post_sqrt_price;
                            swap_event.input_amount = log_data.input_amount;
                            swap_event.output_amount = log_data.output_amount;
                            swap_event.input_transfer_fee = log_data.input_transfer_fee;
                            swap_event.output_transfer_fee = log_data.output_transfer_fee;
                            swap_event.lp_fee = log_data.lp_fee;
                            swap_event.protocol_fee = log_data.protocol_fee;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
