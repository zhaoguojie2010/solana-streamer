use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use crate::streaming::event_parser::Protocol;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwapCuInstructionMatcher {
    Discriminator8(Vec<&'static [u8]>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapCuTarget {
    pub protocol: Protocol,
    pub program_id: Pubkey,
    pub matcher: SwapCuInstructionMatcher,
}

#[derive(Debug, Clone, Default)]
pub struct SwapCuParseConfig {
    pub enabled: bool,
    pub targets: Vec<SwapCuTarget>,
}

impl SwapCuParseConfig {
    pub fn default_enabled() -> Self {
        Self {
            enabled: true,
            targets: default_swap_cu_targets(),
        }
    }

    #[inline]
    pub fn is_target_swap(
        &self,
        protocol: &Protocol,
        program_id: &Pubkey,
        instruction_data: &[u8],
    ) -> bool {
        if !self.enabled || self.targets.is_empty() {
            return false;
        }
        self.targets.iter().any(|target| {
            target.protocol == *protocol
                && target.program_id == *program_id
                && target.matcher.matches(instruction_data)
        })
    }
}

impl SwapCuInstructionMatcher {
    #[inline]
    fn matches(&self, instruction_data: &[u8]) -> bool {
        match self {
            Self::Discriminator8(discriminators) => {
                let Some(head) = instruction_data.get(..8) else {
                    return false;
                };
                discriminators.iter().any(|discriminator| head == *discriminator)
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SwapCuIndex {
    outer: Vec<Option<u32>>,
    inner: Vec<Vec<Option<u32>>>,
}

impl SwapCuIndex {
    pub fn get(&self, outer_index: i64, inner_index: Option<i64>) -> Option<u32> {
        if outer_index < 0 {
            return None;
        }
        if let Some(inner_index) = inner_index {
            if inner_index < 0 {
                return None;
            }
            return self
                .inner
                .get(outer_index as usize)?
                .get(inner_index as usize)?
                .as_ref()
                .copied();
        }
        self.outer.get(outer_index as usize)?.as_ref().copied()
    }
}

#[derive(Clone, Debug)]
struct InvocationSpan {
    program_id: Pubkey,
    depth: usize,
    consumed_cu: Option<u32>,
    start: usize,
    end: usize,
}

fn parse_invoke_line(log: &str) -> Option<(Pubkey, usize)> {
    const PREFIX: &str = "Program ";
    const INVOKE_MARK: &str = " invoke [";
    let rest = log.strip_prefix(PREFIX)?;
    let (program_id_str, depth_part) = rest.split_once(INVOKE_MARK)?;
    let depth_str = depth_part.strip_suffix(']')?.trim();
    let program_id = Pubkey::from_str(program_id_str.trim()).ok()?;
    let depth = depth_str.parse::<usize>().ok()?;
    Some((program_id, depth))
}

fn parse_consumed_line(log: &str) -> Option<(Pubkey, u32)> {
    const PREFIX: &str = "Program ";
    const CONSUMED_MARK: &str = " consumed ";
    const OF_MARK: &str = " of ";
    let rest = log.strip_prefix(PREFIX)?;
    let (program_id_str, consumed_part) = rest.split_once(CONSUMED_MARK)?;
    let (cu_str, _) = consumed_part.split_once(OF_MARK)?;
    let program_id = Pubkey::from_str(program_id_str.trim()).ok()?;
    let consumed_cu = cu_str.trim().parse::<u32>().ok()?;
    Some((program_id, consumed_cu))
}

fn is_exit_line(log: &str) -> bool {
    log.starts_with("Program ") && (log.contains(" success") || log.contains(" failed:"))
}

fn parse_invocation_spans(logs: &[String]) -> Vec<InvocationSpan> {
    let mut spans = Vec::new();
    let mut stack: Vec<usize> = Vec::new();

    for (idx, log) in logs.iter().enumerate() {
        if let Some((program_id, depth)) = parse_invoke_line(log) {
            spans.push(InvocationSpan {
                program_id,
                depth,
                consumed_cu: None,
                start: idx,
                end: idx,
            });
            stack.push(spans.len() - 1);
            continue;
        }

        if let Some((program_id, consumed_cu)) = parse_consumed_line(log) {
            if let Some(span_idx) = stack
                .iter()
                .rev()
                .find(|span_idx| spans[**span_idx].program_id == program_id)
                .copied()
            {
                spans[span_idx].consumed_cu = Some(consumed_cu);
            }
            continue;
        }

        if is_exit_line(log) {
            if let Some(span_idx) = stack.pop() {
                spans[span_idx].end = idx;
            }
        }
    }

    let last_idx = logs.len().saturating_sub(1);
    for span_idx in stack {
        spans[span_idx].end = last_idx;
    }

    spans
}

pub fn build_swap_cu_index(
    config: &SwapCuParseConfig,
    logs: &[String],
    outer_instructions: &[yellowstone_grpc_proto::prelude::CompiledInstruction],
    accounts: &[Pubkey],
    inner_instructions: &[yellowstone_grpc_proto::prelude::InnerInstructions],
) -> SwapCuIndex {
    let outer_len = outer_instructions.len();
    let mut index = SwapCuIndex {
        outer: vec![None; outer_len],
        inner: vec![Vec::new(); outer_len],
    };

    for inner in inner_instructions.iter() {
        let outer_idx = inner.index as usize;
        if outer_idx < outer_len {
            index.inner[outer_idx] = vec![None; inner.instructions.len()];
        }
    }

    if !config.enabled || config.targets.is_empty() || logs.is_empty() || outer_len == 0 {
        return index;
    }

    let spans = parse_invocation_spans(logs);
    let outer_spans: Vec<&InvocationSpan> = spans.iter().filter(|span| span.depth == 1).collect();

    for (outer_idx, instruction) in outer_instructions.iter().enumerate() {
        if let Some(program_id) = accounts.get(instruction.program_id_index as usize) {
            if let Some(protocol) =
                crate::streaming::event_parser::core::dispatcher::EventDispatcher::match_protocol_by_program_id(program_id)
            {
                if config.is_target_swap(&protocol, program_id, &instruction.data) {
                    index.outer[outer_idx] = outer_spans
                        .get(outer_idx)
                        .filter(|span| span.program_id == *program_id)
                        .and_then(|span| span.consumed_cu);
                }
            }
        }

        if index.inner[outer_idx].is_empty() {
            continue;
        }

        let Some(outer_span) = outer_spans.get(outer_idx).copied() else {
            continue;
        };
        let mut inner_spans: Vec<&InvocationSpan> = spans
            .iter()
            .filter(|span| {
                span.depth >= 2 && span.start >= outer_span.start && span.end <= outer_span.end
            })
            .collect();
        inner_spans.sort_by_key(|span| span.start);

        let Some(inner_group) = inner_instructions
            .iter()
            .find(|inner| inner.index as usize == outer_idx)
        else {
            continue;
        };

        for (inner_idx, inner_instruction) in inner_group.instructions.iter().enumerate() {
            let Some(program_id) = accounts.get(inner_instruction.program_id_index as usize) else {
                continue;
            };
            let Some(protocol) =
                crate::streaming::event_parser::core::dispatcher::EventDispatcher::match_protocol_by_program_id(program_id)
            else {
                continue;
            };
            if !config.is_target_swap(&protocol, program_id, &inner_instruction.data) {
                continue;
            }
            index.inner[outer_idx][inner_idx] = inner_spans
                .get(inner_idx)
                .filter(|span| span.program_id == *program_id)
                .and_then(|span| span.consumed_cu);
        }
    }

    index
}

pub fn default_swap_cu_targets() -> Vec<SwapCuTarget> {
    use crate::streaming::event_parser::protocols::{
        meteora_damm_v2, meteora_dlmm, pancakeswap, pumpswap, raydium_clmm, raydium_cpmm,
        whirlpool,
    };

    vec![
        SwapCuTarget {
            protocol: Protocol::PancakeSwap,
            program_id: pancakeswap::parser::PANCAKESWAP_PROGRAM_ID,
            matcher: SwapCuInstructionMatcher::Discriminator8(vec![
                &pancakeswap::discriminators::SWAP,
                &pancakeswap::discriminators::SWAP_V2,
            ]),
        },
        SwapCuTarget {
            protocol: Protocol::RaydiumCpmm,
            program_id: raydium_cpmm::parser::RAYDIUM_CPMM_PROGRAM_ID,
            matcher: SwapCuInstructionMatcher::Discriminator8(vec![
                &raydium_cpmm::discriminators::SWAP_BASE_IN,
                &raydium_cpmm::discriminators::SWAP_BASE_OUT,
            ]),
        },
        SwapCuTarget {
            protocol: Protocol::RaydiumClmm,
            program_id: raydium_clmm::parser::RAYDIUM_CLMM_PROGRAM_ID,
            matcher: SwapCuInstructionMatcher::Discriminator8(vec![
                &raydium_clmm::discriminators::SWAP,
                &raydium_clmm::discriminators::SWAP_V2,
            ]),
        },
        SwapCuTarget {
            protocol: Protocol::Whirlpool,
            program_id: whirlpool::parser::WHIRLPOOL_PROGRAM_ID,
            matcher: SwapCuInstructionMatcher::Discriminator8(vec![
                &whirlpool::discriminators::SWAP,
                &whirlpool::discriminators::SWAP_V2,
            ]),
        },
        SwapCuTarget {
            protocol: Protocol::MeteoraDlmm,
            program_id: meteora_dlmm::parser::METEORA_DLMM_PROGRAM_ID,
            matcher: SwapCuInstructionMatcher::Discriminator8(vec![
                &meteora_dlmm::discriminators::SWAP_IX,
                &meteora_dlmm::discriminators::SWAP2_IX,
                &meteora_dlmm::discriminators::SWAP_EXACT_OUT2_IX,
            ]),
        },
        SwapCuTarget {
            protocol: Protocol::MeteoraDammV2,
            program_id: meteora_damm_v2::parser::METEORA_DAMM_V2_PROGRAM_ID,
            matcher: SwapCuInstructionMatcher::Discriminator8(vec![
                &meteora_damm_v2::discriminators::SWAP_IX,
                &meteora_damm_v2::discriminators::SWAP2_IX,
            ]),
        },
        SwapCuTarget {
            protocol: Protocol::PumpSwap,
            program_id: pumpswap::parser::PUMPSWAP_PROGRAM_ID,
            matcher: SwapCuInstructionMatcher::Discriminator8(vec![
                &pumpswap::discriminators::BUY_IX,
                &pumpswap::discriminators::BUY_EXACT_QUOTE_IN_IX,
                &pumpswap::discriminators::SELL_IX,
            ]),
        },
    ]
}
