use crate::streaming::event_parser::{
    common::{
        build_program_data_index, filter::EventTypeFilter,
        high_performance_clock::elapsed_micros_since, EventMetadata, ProgramDataIndex,
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
    DexEvent, Protocol,
};
use prost_types::Timestamp;
use solana_sdk::{
    message::compiled_instruction::CompiledInstruction, pubkey::Pubkey, signature::Signature,
    transaction::VersionedTransaction,
};
use solana_transaction_status::InnerInstructions;
use std::sync::Arc;
use yellowstone_grpc_proto::geyser::SubscribeUpdateTransactionInfo;

pub struct EventParser {}

#[derive(Clone, Copy)]
struct MintLeg {
    event_index: usize,
    from_mint: Pubkey,
    to_mint: Pubkey,
}

impl EventParser {
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
        callback: Arc<dyn Fn(DexEvent) + Send + Sync>,
    ) -> anyhow::Result<()> {
        // 创建适配器回调，将所有权回调转换为引用回调
        let adapter_callback = Arc::new(move |event: &DexEvent| {
            callback(event.clone());
        });
        if let Some(transition) = grpc_tx.transaction {
            if let Some(message) = &transition.message {
                let mut address_table_lookups: Vec<Vec<u8>> = vec![];
                let mut inner_instructions: Vec<
                    yellowstone_grpc_proto::solana::storage::confirmed_block::InnerInstructions,
                > = vec![];
                let mut log_messages: Vec<String> = vec![];

                if let Some(meta) = grpc_tx.meta {
                    inner_instructions = meta.inner_instructions;
                    log_messages = meta.log_messages;
                    address_table_lookups.reserve(
                        meta.loaded_writable_addresses.len() + meta.loaded_readonly_addresses.len(),
                    );
                    let loaded_writable_addresses = meta.loaded_writable_addresses;
                    let loaded_readonly_addresses = meta.loaded_readonly_addresses;
                    address_table_lookups.extend(
                        loaded_writable_addresses.into_iter().chain(loaded_readonly_addresses),
                    );
                }

                let mut accounts_bytes: Vec<Vec<u8>> =
                    Vec::with_capacity(message.account_keys.len() + address_table_lookups.len());
                accounts_bytes.extend_from_slice(&message.account_keys);
                accounts_bytes.extend(address_table_lookups);
                // 转换为 Pubkey
                let accounts: Vec<Pubkey> = accounts_bytes
                    .iter()
                    .filter_map(|account| {
                        if account.len() == 32 {
                            Some(Pubkey::try_from(account.as_slice()).unwrap_or_default())
                        } else {
                            None
                        }
                    })
                    .collect();
                // 解析指令事件
                let instructions = &message.instructions;
                Self::parse_instruction_events_from_grpc_transaction(
                    protocols,
                    event_type_filter,
                    &instructions,
                    signature,
                    slot,
                    block_time,
                    recv_us,
                    &accounts,
                    &inner_instructions,
                    &log_messages,
                    bot_wallet,
                    transaction_index,
                    adapter_callback,
                )
                .await?;
            }
        }

        Ok(())
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
        callback: Arc<dyn Fn(DexEvent) + Send + Sync>,
    ) -> anyhow::Result<()> {
        // 创建适配器回调，将所有权回调转换为引用回调
        let adapter_callback = Arc::new(move |event: &DexEvent| {
            callback(event.clone());
        });
        // 获取交易的指令和账户
        let compiled_instructions = transaction.message.instructions();
        let mut accounts: Vec<Pubkey> = accounts.to_vec();
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
                            accounts.resize(*max_idx as usize + 1, Pubkey::default());
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
                            adapter_callback.clone(),
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
                                adapter_callback.clone(),
                            )?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // ================================================================================================
    // gRPC Transaction Processing
    // ================================================================================================

    /// Parse instruction events from gRPC transaction format
    ///
    /// Iterates through all instructions in a gRPC transaction, checks if they should be handled,
    /// and delegates to instruction-level parsing for both outer and inner instructions.
    #[allow(clippy::too_many_arguments)]
    async fn parse_instruction_events_from_grpc_transaction(
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
        callback: Arc<dyn for<'a> Fn(&'a DexEvent) + Send + Sync>,
    ) -> anyhow::Result<()> {
        // 获取交易的指令和账户
        let mut accounts = accounts.to_vec();
        // 检查交易中是否包含程序
        let has_program = accounts
            .iter()
            .any(|account| Self::should_handle(protocols, event_type_filter, account));
        if has_program {
            // 解析每个指令
            let mut program_data_index: Option<ProgramDataIndex> = None;
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
                        accounts.resize(*max_idx as usize + 1, Pubkey::default());
                    }
                    if Self::should_handle(protocols, event_type_filter, &program_id) {
                        Self::parse_events_from_grpc_instruction(
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
                            program_data_index.as_ref(),
                            callback.clone(),
                        )?;
                    }
                    // Immediately process inner instructions for correct ordering
                    if let Some(inner_instructions) = inner_instructions {
                        let mut inner_events: Vec<DexEvent> =
                            Vec::with_capacity(inner_instructions.instructions.len());
                        for (inner_index, inner_instruction) in
                            inner_instructions.instructions.iter().enumerate()
                        {
                            let inner_accounts = &inner_instruction.accounts;
                            let data = &inner_instruction.data;
                            let instruction =
                                yellowstone_grpc_proto::prelude::CompiledInstruction {
                                    program_id_index: inner_instruction.program_id_index,
                                    accounts: inner_accounts.to_vec(),
                                    data: data.to_vec(),
                                };
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
                                &instruction,
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
                            )? {
                                inner_events.push(inner_event);
                            }
                        }

                        Self::mark_arb_inner_swap_events(&mut inner_events);
                        for inner_event in inner_events.iter() {
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
        instruction: &yellowstone_grpc_proto::prelude::CompiledInstruction,
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
        callback: Arc<dyn for<'a> Fn(&'a DexEvent) + Send + Sync>,
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
        )? {
            callback(&event);
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_event_from_grpc_instruction(
        protocols: &[Protocol],
        event_type_filter: Option<&EventTypeFilter>,
        instruction: &yellowstone_grpc_proto::prelude::CompiledInstruction,
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
        callback: Arc<dyn for<'a> Fn(&'a DexEvent) + Send + Sync>,
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
                callback(&event);
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
        callback(&event);

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
            _ => return None,
        };
        if from_mint == Pubkey::default() || to_mint == Pubkey::default() {
            None
        } else {
            Some((from_mint, to_mint))
        }
    }

    fn mark_arb_segment(events: &mut [DexEvent], legs: &[MintLeg]) {
        if legs.len() < 2 {
            return;
        }
        let Some(first_leg) = legs.first() else {
            return;
        };
        let Some(last_leg) = legs.last() else {
            return;
        };
        if first_leg.from_mint != last_leg.to_mint {
            return;
        }

        for leg in legs.iter() {
            events[leg.event_index].metadata_mut().is_arb_leg = true;
        }
    }

    #[inline]
    fn mark_arb_inner_swap_events(events: &mut [DexEvent]) {
        let mut segment_legs: Vec<MintLeg> = Vec::new();
        for event_index in 0..events.len() {
            let (is_inner, swap_mints) = {
                let event = &events[event_index];
                let metadata = event.metadata();
                (metadata.inner_index.is_some(), Self::extract_swap_mints(event))
            };

            if !is_inner {
                continue;
            }

            let Some((from_mint, to_mint)) = swap_mints else {
                Self::mark_arb_segment(events, &segment_legs);
                segment_legs.clear();
                continue;
            };

            let next_leg = MintLeg { event_index, from_mint, to_mint };

            if let Some(last_leg) = segment_legs.last() {
                if last_leg.to_mint != next_leg.from_mint {
                    Self::mark_arb_segment(events, &segment_legs);
                    segment_legs.clear();
                }
            }
            segment_legs.push(next_leg);
        }

        Self::mark_arb_segment(events, &segment_legs);
    }

    fn instruction_needs_program_data(protocol: &Protocol, data: &[u8]) -> bool {
        match protocol {
            Protocol::RaydiumCpmm => {
                if data.len() < 8 {
                    return false;
                }
                crate::streaming::event_parser::protocols::raydium_cpmm::parser::is_raydium_cpmm_swap_instruction(
                    &data[..8],
                )
            }
            Protocol::RaydiumClmm => {
                if data.len() < 8 {
                    return false;
                }
                crate::streaming::event_parser::protocols::raydium_clmm::parser::is_raydium_clmm_swap_instruction(
                    &data[..8],
                )
            }
            Protocol::Whirlpool => {
                if data.len() < 8 {
                    return false;
                }
                crate::streaming::event_parser::protocols::whirlpool::parser::is_whirlpool_swap_instruction(
                    &data[..8],
                )
            }
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

    let item = if let Some(inner_index) = inner_index {
        index.get_inner(outer_index, inner_index)
    } else {
        index.get_outer(outer_index)
    };

    let Some(item) = item else {
        return;
    };

    match protocol {
        Protocol::RaydiumCpmm => {
            use crate::streaming::event_parser::protocols::raydium_cpmm::parser::parse_swap_event_from_program_data;
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
            use crate::streaming::event_parser::protocols::raydium_clmm::parser::parse_swap_event_from_program_data;
            match event {
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
            use crate::streaming::event_parser::protocols::whirlpool::parser::parse_traded_event_from_program_data;
            match event {
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
