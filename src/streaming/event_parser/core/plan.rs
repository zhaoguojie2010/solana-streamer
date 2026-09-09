use super::dispatcher::EventDispatcher;
use crate::streaming::event_parser::{
    common::{EventType, SwapCuParseConfig},
    protocols::*,
    Protocol,
};
use solana_sdk::pubkey::Pubkey;

/// Optional work. Defaults perform instruction decoding and CPI event merging only.
#[derive(Clone, Debug, Default)]
pub struct ParseOptions {
    pub retain_instructions: bool,
    pub retain_logs: bool,
    pub enrich_logs: bool,
    pub swap_cu: SwapCuParseConfig,
    pub balance_audit: bool,
    pub classify_swaps: bool,
    pub detect_jito: bool,
    pub compute_budget: bool,
    pub bot_wallet: Option<Pubkey>,
}

/// Immutable subscription-level selection and dependency plan.
/// `None` selects all event types; `Some(&[])` selects none.
#[derive(Clone, Debug)]
pub struct ParsePlan {
    protocols: u16,
    events: [u64; 2],
    options: ParseOptions,
}

impl ParsePlan {
    pub fn new(
        protocols: &[Protocol],
        events: Option<&[EventType]>,
        options: ParseOptions,
    ) -> Self {
        let mut bits = [0; 2];
        if let Some(events) = events {
            for &event in events {
                bits[event as usize / 64] |= 1 << (event as usize % 64);
            }
        } else {
            bits = [u64::MAX; 2];
        }
        Self {
            protocols: protocols.iter().fold(0, |mask, &p| mask | (1 << p as u8)),
            events: bits,
            options,
        }
    }
    pub fn all(protocols: &[Protocol]) -> Self {
        Self::new(protocols, None, ParseOptions::default())
    }
    pub fn options(&self) -> &ParseOptions {
        &self.options
    }
    pub fn includes(&self, event: EventType) -> bool {
        self.events[event as usize / 64] & (1 << (event as usize % 64)) != 0
    }
    pub fn includes_protocol(&self, protocol: Protocol) -> bool {
        self.protocols & (1 << protocol as u8) != 0
    }
    pub fn protocols(&self) -> impl Iterator<Item = Protocol> + '_ {
        [
            Protocol::PancakeSwap,
            Protocol::PumpSwap,
            Protocol::PumpFun,
            Protocol::Bonk,
            Protocol::RaydiumCpmm,
            Protocol::RaydiumClmm,
            Protocol::RaydiumAmmV4,
            Protocol::MeteoraDammV2,
            Protocol::MeteoraDlmm,
            Protocol::Whirlpool,
        ]
        .into_iter()
        .filter(|&p| self.includes_protocol(p))
    }
    pub(crate) fn keep_empty_batch(&self) -> bool {
        let o = &self.options;
        o.retain_instructions
            || o.retain_logs
            || o.balance_audit
            || o.compute_budget
            || o.detect_jito
            || o.classify_swaps
    }
    pub(crate) fn needs(&self, event: EventType) -> bool {
        self.includes(event)
            || self.options.classify_swaps
            || match event {
                EventType::PumpFunCreateToken | EventType::PumpFunCreateV2Token => {
                    self.includes(EventType::PumpFunBuy) || self.includes(EventType::PumpFunSell)
                }
                EventType::BonkInitialize
                | EventType::BonkInitializeV2
                | EventType::BonkInitializeWithToken2022 => [
                    EventType::BonkBuyExactIn,
                    EventType::BonkBuyExactOut,
                    EventType::BonkSellExactIn,
                    EventType::BonkSellExactOut,
                ]
                .into_iter()
                .any(|t| self.includes(t)),
                EventType::SetComputeUnitLimit | EventType::SetComputeUnitPrice => {
                    self.options.compute_budget
                }
                _ => false,
            }
    }
    pub(crate) fn instruction_type(
        &self,
        program: &Pubkey,
        data: &[u8],
    ) -> Option<(Option<Protocol>, EventType)> {
        if EventDispatcher::is_compute_budget_program(program) {
            return Some((
                None,
                match data.first()? {
                    2 => EventType::SetComputeUnitLimit,
                    3 => EventType::SetComputeUnitPrice,
                    _ => return None,
                },
            ));
        }
        let protocol = EventDispatcher::match_protocol_by_program_id(program)?;
        if !self.includes_protocol(protocol) {
            return None;
        }
        let disc = data.get(..if protocol == Protocol::RaydiumAmmV4 { 1 } else { 8 })?;
        let event = match protocol {
            Protocol::PancakeSwap => pancakeswap::parser::instruction_event_type(disc),
            Protocol::PumpSwap => pumpswap::parser::instruction_event_type(disc),
            Protocol::PumpFun => pumpfun::parser::instruction_event_type(disc),
            Protocol::Bonk => bonk::parser::instruction_event_type(disc),
            Protocol::RaydiumCpmm => raydium_cpmm::parser::instruction_event_type(disc),
            Protocol::RaydiumClmm => raydium_clmm::parser::instruction_event_type(disc),
            Protocol::RaydiumAmmV4 => raydium_amm_v4::parser::instruction_event_type(disc),
            Protocol::MeteoraDammV2 => meteora_damm_v2::parser::instruction_event_type(disc),
            Protocol::MeteoraDlmm => meteora_dlmm::parser::instruction_event_type(disc),
            Protocol::Whirlpool => whirlpool::parser::instruction_event_type(disc),
        }?;
        Some((Some(protocol), event))
    }
}

// EventType is a fieldless enum; expanding it beyond the compiled mask is a build error.
const _: () = assert!((EventType::Unknown as usize) < 128);
