//! Synchronous, worker-local transaction parser. No locks, tasks or global transaction state.
use crate::streaming::event_parser::{
    common::{EventMetadata, InvocationIndex, ProgramDataItem},
    core::{dispatcher::EventDispatcher, merger_event::merge},
    ParsePlan, Protocol, TxBatch, TxEvent, TxExecutionMetaAudit, TxFrame, TxSummary, TxSwapKind,
    TxTokenBalanceChange, TxView,
};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine};
use solana_sdk::pubkey::Pubkey;
use std::collections::{HashMap, HashSet};

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

/// Maximum retained worker capacity after a transaction. Larger inputs are still parsed.
#[derive(Clone, Copy, Debug)]
pub struct ScratchLimits {
    pub events: usize,
    pub instructions: usize,
    pub log_items: usize,
    pub decoded_log_bytes: usize,
    pub developers: usize,
}
impl Default for ScratchLimits {
    fn default() -> Self {
        Self {
            events: 64,
            instructions: 512,
            log_items: 1024,
            decoded_log_bytes: 64 * 1024,
            developers: 32,
        }
    }
}
#[derive(Default)]
pub struct TxParser {
    events: Vec<TxEvent>,
    next_output_capacity: usize,
    developers: Vec<Pubkey>,
    bonk_developers: Vec<Pubkey>,
    index: InvocationIndex,
    decoded: Vec<u8>,
    summary: TxSummary,
    audit: Option<TxExecutionMetaAudit>,
    limits: ScratchLimits,
}
struct MintLeg {
    from_mint: Pubkey,
    to_mint: Pubkey,
}
struct AccountLeg {
    from_account: Pubkey,
    to_account: Pubkey,
}

impl TxParser {
    pub fn new(limits: ScratchLimits) -> Self {
        Self { limits, ..Self::default() }
    }
    pub fn visit(
        &mut self,
        frame: &TxFrame,
        plan: &ParsePlan,
        mut sink: impl FnMut(TxView<'_>),
    ) -> Result<()> {
        self.parse(frame, plan);
        if !self.events.is_empty() || plan.keep_empty_batch() {
            let retain = plan.options().retain_instructions
                || self.events.iter().any(TxEvent::requires_instruction_source);
            sink(TxView {
                meta: &frame.meta,
                keys: &frame.keys,
                events: &self.events,
                summary: &self.summary,
                audit: self.audit.as_ref(),
                source: retain.then_some(&frame.instructions),
                logs: plan.options().retain_logs.then_some(&frame.logs),
            });
        }
        self.trim();
        Ok(())
    }
    pub fn parse_owned(&mut self, frame: TxFrame, plan: &ParsePlan) -> Result<Option<TxBatch>> {
        self.parse(&frame, plan);
        self.next_output_capacity = self.events.len().min(self.limits.events);
        if self.events.is_empty() && !plan.keep_empty_batch() {
            self.trim();
            return Ok(None);
        }
        let retain = plan.options().retain_instructions
            || self.events.iter().any(TxEvent::requires_instruction_source);
        let batch = TxBatch {
            meta: frame.meta,
            keys: frame.keys,
            events: std::mem::take(&mut self.events),
            summary: std::mem::take(&mut self.summary),
            audit: self.audit.take(),
            source: retain.then_some(frame.instructions),
            logs: plan.options().retain_logs.then_some(frame.logs),
        };
        self.trim();
        Ok(Some(batch))
    }
    fn trim(&mut self) {
        self.events.clear();
        self.developers.clear();
        self.bonk_developers.clear();
        self.decoded.clear();
        if self.events.capacity() > self.limits.events {
            self.events = Vec::new();
        }
        if self.developers.capacity() > self.limits.developers {
            self.developers = Vec::new();
        }
        if self.bonk_developers.capacity() > self.limits.developers {
            self.bonk_developers = Vec::new();
        }
        if self.decoded.capacity() > self.limits.decoded_log_bytes {
            self.decoded = Vec::new();
        }
        self.index.trim(self.limits.instructions, self.limits.log_items);
        self.audit = None;
    }
    fn parse(&mut self, frame: &TxFrame, plan: &ParsePlan) {
        self.events.clear();
        self.developers.clear();
        self.bonk_developers.clear();
        self.summary = TxSummary::default();
        self.audit = None;
        let options = plan.options();
        let logs = (options.enrich_logs || options.swap_cu.enabled) && !frame.logs.is_empty();
        let mut indexed = false;
        if options.detect_jito {
            self.summary.is_jito = Some(frame.instructions.iter().any(|ix| {
                frame.keys[ix.program_id_index as usize] == SYSTEM_PROGRAM_ID
                    && ix.data.get(..4) == Some(2u32.to_le_bytes().as_slice())
                    && ix
                        .accounts
                        .get(1)
                        .is_some_and(|&i| JITO_TIP_ACCOUNTS.contains(&frame.keys[usize::from(i)]))
            }));
        }
        for (instruction_index, ix) in frame.instructions.iter().enumerate() {
            let view = ix.view(&frame.keys);
            let Some((protocol, event_type)) = plan.instruction_type(view.program_id, view.data)
            else {
                continue;
            };
            if !plan.needs(event_type) {
                continue;
            }
            let metadata = EventMetadata {
                event_type,
                program_id: *view.program_id,
                instruction_index: instruction_index as u32,
                outer_index: i64::from(ix.outer_index),
                inner_index: ix.inner_index.map(i64::from),
                ..EventMetadata::default()
            };
            let mut event = if let Some(protocol) = protocol {
                let disc_len = if protocol == Protocol::RaydiumAmmV4 { 1 } else { 8 };
                let Some(event) = EventDispatcher::dispatch_instruction(
                    protocol,
                    &ix.data[..disc_len],
                    &ix.data[disc_len..],
                    view.accounts,
                    metadata,
                    &frame.meta,
                ) else {
                    continue;
                };
                event
            } else {
                let Some(event) =
                    EventDispatcher::dispatch_compute_budget_instruction(&ix.data, metadata)
                else {
                    continue;
                };
                event
            };
            if let Some(protocol) = protocol {
                let need_execution_logs = options.enrich_logs
                    && matches!(
                        protocol,
                        Protocol::PancakeSwap
                            | Protocol::RaydiumCpmm
                            | Protocol::RaydiumClmm
                            | Protocol::Whirlpool
                    );
                let need_cu = options.swap_cu.is_target_swap(&protocol, view.program_id, view.data);
                if logs && (need_execution_logs || need_cu) {
                    if !indexed {
                        self.index.build(frame, options.enrich_logs, options.swap_cu.enabled);
                        indexed = true;
                    }
                    let observation = &self.index.observations[instruction_index];
                    if need_cu {
                        event.metadata_mut().swap_compute_units = observation.consumed_cu;
                    }
                    let mut location =
                        if need_execution_logs { observation.first_data } else { None };
                    while let Some(i) = location {
                        let item = &self.index.data[i];
                        let encoded = frame.logs[item.log_index]
                            .strip_prefix("Program data: ")
                            .unwrap_or_default();
                        self.decoded.clear();
                        if STANDARD.decode_vec(encoded, &mut self.decoded).is_ok() {
                            enrich_event_from_program_data(
                                &mut event,
                                &protocol,
                                &ProgramDataItem {
                                    data: &self.decoded,
                                    program_id: *view.program_id,
                                    log_index: item.log_index,
                                },
                            );
                        }
                        location = item.next;
                    }
                }
                let mut merged = false;
                // Only descendants of this invocation can supply its CPI event.
                // Stack heights are available on current sources. Without them, stop at the next
                // normal instruction for this program to avoid merging a sibling swap's event.
                for child in &frame.instructions[instruction_index + 1..ix.group_end] {
                    if ix.inner_index.is_some()
                        && child.stack_height.zip(ix.stack_height).is_some_and(|(c, p)| c <= p)
                    {
                        break;
                    }
                    if frame.keys[child.program_id_index as usize] != *view.program_id {
                        continue;
                    }
                    if child.data.get(..8) != Some(&[228, 69, 165, 46, 81, 203, 154, 29]) {
                        if child.stack_height.is_none() {
                            break;
                        }
                        continue;
                    }
                    if child.data.len() < 16
                        || child
                            .stack_height
                            .zip(ix.stack_height)
                            .is_some_and(|(c, p)| c != p.saturating_add(1))
                    {
                        continue;
                    }
                    if let Some(inner) = EventDispatcher::dispatch_inner_instruction(
                        protocol,
                        &child.data[..16],
                        &child.data[16..],
                        event.metadata().clone(),
                    ) {
                        if compatible_cpi(&event, &inner) {
                            merge(&mut event, inner);
                            merged = true;
                            break;
                        }
                    }
                }
                if matches!(event, TxEvent::PumpFunMigrateEvent(_)) && !merged {
                    continue;
                }
            }
            if options.compute_budget {
                match &event {
                    TxEvent::SetComputeUnitLimitEvent(e) => {
                        self.summary.compute_unit_limit = Some(e.units)
                    }
                    TxEvent::SetComputeUnitPriceEvent(e) => {
                        self.summary.compute_unit_price = Some(e.micro_lamports)
                    }
                    _ => {}
                }
            }
            let event = self.process_event(event, options.bot_wallet);
            if plan.includes(event.metadata().event_type) || options.classify_swaps {
                // Owned delivery transfers the Vec. Learn a bounded size hint instead of
                // pretending that transferred storage can be reused. Allocate only on output.
                if self.events.capacity() == 0 && self.next_output_capacity > 1 {
                    self.events
                        .reserve_exact(self.next_output_capacity.min(frame.instructions.len()));
                }
                self.events.push(event);
            }
        }
        if options.classify_swaps {
            self.summary.swap_kind = Some(Self::swap_kind_from_is_arb(
                Self::is_arb_inner_swap_events(&self.events),
                &self.events,
            ));
            self.events.retain(|event| plan.includes(event.metadata().event_type));
        }
        if options.balance_audit {
            self.audit = Some(collect_audit(frame));
        }
    }
    #[inline]
    fn extract_swap_mints(event: &TxEvent) -> Option<(Pubkey, Pubkey)> {
        let (from_mint, to_mint) = match event {
            TxEvent::PumpSwapBuyEvent(e) => (e.quote_mint, e.base_mint),
            TxEvent::PumpSwapBuyExactQuoteInEvent(e) => (e.quote_mint, e.base_mint),
            TxEvent::PumpSwapSellEvent(e) => (e.base_mint, e.quote_mint),
            TxEvent::PancakeSwapSwapV2Event(e) => (e.input_mint, e.output_mint),
            TxEvent::BonkTradeEvent(e) => match e.trade_direction {
                crate::streaming::event_parser::protocols::bonk::types::TradeDirection::Buy => {
                    (e.quote_token_mint, e.base_token_mint)
                }
                crate::streaming::event_parser::protocols::bonk::types::TradeDirection::Sell => {
                    (e.base_token_mint, e.quote_token_mint)
                }
            },
            TxEvent::RaydiumCpmmSwapEvent(e) => (e.input_token_mint, e.output_token_mint),
            TxEvent::RaydiumClmmSwapV2Event(e) => (e.input_vault_mint, e.output_vault_mint),
            TxEvent::MeteoraDlmmSwapEvent(e) => {
                if e.swap_for_y {
                    (e.token_x_mint?, e.token_y_mint?)
                } else {
                    (e.token_y_mint?, e.token_x_mint?)
                }
            }
            TxEvent::MeteoraDlmmSwap2Event(e) => {
                if e.swap_for_y {
                    (e.token_x_mint?, e.token_y_mint?)
                } else {
                    (e.token_y_mint?, e.token_x_mint?)
                }
            }
            TxEvent::WhirlpoolSwapV2Event(e) => {
                if e.a_to_b {
                    (e.token_mint_a, e.token_mint_b)
                } else {
                    (e.token_mint_b, e.token_mint_a)
                }
            }
            TxEvent::MeteoraDammV2SwapEvent(e) => (e.token_a_mint, e.token_b_mint),
            TxEvent::MeteoraDammV2Swap2Event(e) => (e.token_a_mint, e.token_b_mint),
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
    fn extract_swap_pool_id(event: &TxEvent) -> Option<Pubkey> {
        match event {
            TxEvent::PumpSwapBuyEvent(e) => Some(e.pool),
            TxEvent::PumpSwapBuyExactQuoteInEvent(e) => Some(e.pool),
            TxEvent::PumpSwapSellEvent(e) => Some(e.pool),
            TxEvent::PancakeSwapSwapEvent(e) => Some(e.pool_state),
            TxEvent::PancakeSwapSwapV2Event(e) => Some(e.pool_state),
            TxEvent::RaydiumClmmSwapEvent(e) => Some(e.pool_state),
            TxEvent::RaydiumClmmSwapV2Event(e) => Some(e.pool_state),
            TxEvent::MeteoraDammV2SwapEvent(e) => Some(e.pool),
            TxEvent::MeteoraDammV2Swap2Event(e) => Some(e.pool),
            TxEvent::RaydiumCpmmSwapEvent(e) => Some(e.pool_state),
            TxEvent::MeteoraDlmmSwapEvent(e) => Some(e.lb_pair),
            TxEvent::MeteoraDlmmSwap2Event(e) => Some(e.lb_pair),
            TxEvent::WhirlpoolSwapEvent(e) => Some(e.whirlpool),
            TxEvent::WhirlpoolSwapV2Event(e) => Some(e.whirlpool),
            TxEvent::BonkTradeEvent(e) => Some(e.pool_state),
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
    fn is_multi_pool_route_events(events: &[TxEvent]) -> bool {
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
    fn swap_kind_from_is_arb(is_arb: bool, events: &[TxEvent]) -> TxSwapKind {
        if is_arb {
            TxSwapKind::Arb
        } else if Self::is_multi_pool_route_events(events) {
            TxSwapKind::Route
        } else {
            TxSwapKind::SimpleSwap
        }
    }

    #[inline]
    fn extract_swap_token_accounts(event: &TxEvent) -> Option<(Pubkey, Pubkey)> {
        let (from_account, to_account) = match event {
            TxEvent::BonkTradeEvent(e) => match e.trade_direction {
                crate::streaming::event_parser::protocols::bonk::types::TradeDirection::Buy => {
                    (e.user_quote_token, e.user_base_token)
                }
                crate::streaming::event_parser::protocols::bonk::types::TradeDirection::Sell => {
                    (e.user_base_token, e.user_quote_token)
                }
            },
            TxEvent::PumpSwapBuyEvent(e) => (e.user_quote_token_account, e.user_base_token_account),
            TxEvent::PumpSwapBuyExactQuoteInEvent(e) => {
                (e.user_quote_token_account, e.user_base_token_account)
            }
            TxEvent::PumpSwapSellEvent(e) => {
                (e.user_base_token_account, e.user_quote_token_account)
            }
            TxEvent::PancakeSwapSwapEvent(e) => (e.input_token_account, e.output_token_account),
            TxEvent::PancakeSwapSwapV2Event(e) => (e.input_token_account, e.output_token_account),
            TxEvent::RaydiumAmmV4SwapEvent(e) => {
                (e.user_source_token_account, e.user_destination_token_account)
            }
            TxEvent::RaydiumClmmSwapEvent(e) => (e.input_token_account, e.output_token_account),
            TxEvent::RaydiumClmmSwapV2Event(e) => (e.input_token_account, e.output_token_account),
            TxEvent::MeteoraDlmmSwapEvent(e) => (e.user_token_in?, e.user_token_out?),
            TxEvent::MeteoraDlmmSwap2Event(e) => (e.user_token_in?, e.user_token_out?),
            TxEvent::WhirlpoolSwapEvent(e) => {
                if e.a_to_b {
                    (e.token_owner_account_a, e.token_owner_account_b)
                } else {
                    (e.token_owner_account_b, e.token_owner_account_a)
                }
            }
            TxEvent::WhirlpoolSwapV2Event(e) => {
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
    fn is_arb_inner_swap_events(events: &[TxEvent]) -> bool {
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

    fn process_event(&mut self, event: TxEvent, bot_wallet: Option<Pubkey>) -> TxEvent {
        match event {
            TxEvent::PumpFunCreateTokenEvent(token_info) => {
                self.developers.push(token_info.user);
                if token_info.creator != Pubkey::default() && token_info.creator != token_info.user
                {
                    self.developers.push(token_info.creator);
                }
                TxEvent::PumpFunCreateTokenEvent(token_info)
            }
            TxEvent::PumpFunCreateV2TokenEvent(token_info) => {
                self.developers.push(token_info.user);
                if token_info.creator != Pubkey::default() && token_info.creator != token_info.user
                {
                    self.developers.push(token_info.creator);
                }
                TxEvent::PumpFunCreateV2TokenEvent(token_info)
            }
            TxEvent::PumpFunTradeEvent(mut trade_info) => {
                trade_info.is_dev_create_token_trade = self.developers.contains(&trade_info.user)
                    || self.developers.contains(&trade_info.creator);
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
                TxEvent::PumpFunTradeEvent(trade_info)
            }
            TxEvent::PumpSwapBuyEvent(mut trade_info) => {
                if let Some(swap_data) = trade_info.metadata.swap_data.as_mut() {
                    swap_data.from_amount = trade_info.user_quote_amount_in;
                    swap_data.to_amount = trade_info.base_amount_out;
                }
                TxEvent::PumpSwapBuyEvent(trade_info)
            }
            TxEvent::PumpSwapBuyExactQuoteInEvent(mut trade_info) => {
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
                TxEvent::PumpSwapBuyExactQuoteInEvent(trade_info)
            }
            TxEvent::PumpSwapSellEvent(mut trade_info) => {
                if let Some(swap_data) = trade_info.metadata.swap_data.as_mut() {
                    swap_data.from_amount = trade_info.base_amount_in;
                    swap_data.to_amount = trade_info.user_quote_amount_out;
                }
                TxEvent::PumpSwapSellEvent(trade_info)
            }
            TxEvent::PancakeSwapSwapEvent(mut trade_info) => {
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
                TxEvent::PancakeSwapSwapEvent(trade_info)
            }
            TxEvent::PancakeSwapSwapV2Event(mut trade_info) => {
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
                TxEvent::PancakeSwapSwapV2Event(trade_info)
            }
            TxEvent::BonkPoolCreateEvent(pool_info) => {
                self.bonk_developers.push(pool_info.creator);
                TxEvent::BonkPoolCreateEvent(pool_info)
            }
            TxEvent::BonkTradeEvent(mut trade_info) => {
                trade_info.is_dev_create_token_trade =
                    self.bonk_developers.contains(&trade_info.payer);
                trade_info.is_bot = Some(trade_info.payer) == bot_wallet;
                TxEvent::BonkTradeEvent(trade_info)
            }
            TxEvent::WhirlpoolSwapEvent(mut trade_info) => {
                if let Some(swap_data) = trade_info.metadata.swap_data.as_mut() {
                    swap_data.from_amount = trade_info.input_amount;
                    swap_data.to_amount = trade_info.output_amount;
                }
                TxEvent::WhirlpoolSwapEvent(trade_info)
            }
            TxEvent::WhirlpoolSwapV2Event(mut trade_info) => {
                if let Some(swap_data) = trade_info.metadata.swap_data.as_mut() {
                    swap_data.from_amount = trade_info.input_amount;
                    swap_data.to_amount = trade_info.output_amount;
                }
                TxEvent::WhirlpoolSwapV2Event(trade_info)
            }
            _ => event,
        }
    }
}

fn enrich_event_from_program_data(
    event: &mut TxEvent,
    protocol: &Protocol,
    item: &ProgramDataItem<'_>,
) {
    match protocol {
        Protocol::PancakeSwap => {
            use crate::streaming::event_parser::protocols::pancakeswap::parser::parse_swap_event_from_program_data;
            match event {
                TxEvent::PancakeSwapSwapEvent(swap_event) => {
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
                TxEvent::PancakeSwapSwapV2Event(swap_event) => {
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
                TxEvent::RaydiumCpmmDepositEvent(e) => {
                    if let Some(state) =
                        parse_liquidity_state_from_program_data(item, e.pool_state, 0)
                    {
                        e.liquidity_state = Some(state);
                    }
                }
                TxEvent::RaydiumCpmmWithdrawEvent(e) => {
                    if let Some(state) =
                        parse_liquidity_state_from_program_data(item, e.pool_state, 1)
                    {
                        e.liquidity_state = Some(state);
                    }
                }
                _ => {}
            }
            if let TxEvent::RaydiumCpmmSwapEvent(swap_event) = event {
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
                TxEvent::RaydiumClmmInstructionEvent(instruction) => {
                    if let Some(execution) = parse_execution_event_from_program_data(item) {
                        instruction.execution_events.push(execution);
                    }
                }
                TxEvent::RaydiumClmmSwapEvent(swap_event) => {
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
                TxEvent::RaydiumClmmSwapV2Event(swap_event) => {
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
                TxEvent::WhirlpoolInstructionEvent(instruction) => {
                    if let Some(execution) = parse_execution_event_from_program_data(item) {
                        instruction.execution_events.push(execution);
                    }
                }
                TxEvent::WhirlpoolSwapEvent(swap_event) => {
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
                TxEvent::WhirlpoolSwapV2Event(swap_event) => {
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

fn compatible_cpi(a: &TxEvent, b: &TxEvent) -> bool {
    matches!(
        (a, b),
        (TxEvent::PumpFunTradeEvent(_), TxEvent::PumpFunTradeEvent(_))
            | (TxEvent::PumpFunCreateTokenEvent(_), TxEvent::PumpFunCreateV2TokenEvent(_))
            | (TxEvent::PumpFunCreateV2TokenEvent(_), TxEvent::PumpFunCreateV2TokenEvent(_))
            | (TxEvent::PumpFunMigrateEvent(_), TxEvent::PumpFunMigrateEvent(_))
            | (TxEvent::BonkTradeEvent(_), TxEvent::BonkTradeEvent(_))
            | (TxEvent::BonkPoolCreateEvent(_), TxEvent::BonkPoolCreateEvent(_))
            | (TxEvent::BonkMigrateToAmmEvent(_), TxEvent::BonkMigrateToAmmEvent(_))
            | (TxEvent::PumpSwapBuyEvent(_), TxEvent::PumpSwapBuyEvent(_))
            | (TxEvent::PumpSwapBuyExactQuoteInEvent(_), TxEvent::PumpSwapBuyEvent(_))
            | (TxEvent::PumpSwapSellEvent(_), TxEvent::PumpSwapSellEvent(_))
            | (TxEvent::PumpSwapCreatePoolEvent(_), TxEvent::PumpSwapCreatePoolEvent(_))
            | (TxEvent::PumpSwapInitBoostEvent(_), TxEvent::PumpSwapInitBoostEvent(_))
            | (TxEvent::PumpSwapDepositEvent(_), TxEvent::PumpSwapDepositEvent(_))
            | (TxEvent::PumpSwapWithdrawEvent(_), TxEvent::PumpSwapWithdrawEvent(_))
            | (TxEvent::MeteoraDlmmSwapEvent(_), TxEvent::MeteoraDlmmSwapEvent(_))
            | (TxEvent::MeteoraDlmmSwapEvent(_), TxEvent::MeteoraDlmmSwap2Event(_))
            | (TxEvent::MeteoraDlmmSwap2Event(_), TxEvent::MeteoraDlmmSwap2Event(_))
            | (TxEvent::MeteoraDlmmSwap2Event(_), TxEvent::MeteoraDlmmSwapEvent(_))
            | (TxEvent::MeteoraDammV2SwapEvent(_), TxEvent::MeteoraDammV2SwapEvent(_))
            | (TxEvent::MeteoraDammV2Swap2Event(_), TxEvent::MeteoraDammV2SwapEvent(_))
            | (
                TxEvent::MeteoraDammV2InitializePoolEvent(_),
                TxEvent::MeteoraDammV2InitializePoolEvent(_)
            )
            | (
                TxEvent::MeteoraDammV2InitializeCustomizablePoolEvent(_),
                TxEvent::MeteoraDammV2InitializePoolEvent(_)
            )
            | (
                TxEvent::MeteoraDammV2InitializePoolWithDynamicConfigEvent(_),
                TxEvent::MeteoraDammV2InitializePoolEvent(_)
            )
            | (
                TxEvent::MeteoraDammV2LiquidityChangeEvent(_),
                TxEvent::MeteoraDammV2LiquidityChangeEvent(_)
            )
    )
}
fn collect_audit(frame: &TxFrame) -> TxExecutionMetaAudit {
    let mut audit = TxExecutionMetaAudit::default();
    let Some((pre, post)) = frame.balances.as_ref() else {
        audit.parse_errors.push("execution balances unavailable".into());
        return audit;
    };
    let pre: std::collections::BTreeMap<_, _> = pre.iter().map(|b| (b.account_index, b)).collect();
    let post: std::collections::BTreeMap<_, _> =
        post.iter().map(|b| (b.account_index, b)).collect();
    let indices: std::collections::BTreeSet<_> = pre.keys().chain(post.keys()).copied().collect();
    for account_index in indices {
        let pre = pre.get(&account_index).copied();
        let post = post.get(&account_index).copied();
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
            account: frame.keys.get(usize::try_from(account_index).unwrap_or(usize::MAX)).copied(),
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
