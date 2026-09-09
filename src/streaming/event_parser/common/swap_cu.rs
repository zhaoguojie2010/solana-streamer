use solana_sdk::pubkey::Pubkey;

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
        Self { enabled: true, targets: default_swap_cu_targets() }
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

pub fn default_swap_cu_targets() -> Vec<SwapCuTarget> {
    use crate::streaming::event_parser::protocols::{
        meteora_damm_v2, meteora_dlmm, pancakeswap, pumpswap, raydium_clmm, raydium_cpmm, whirlpool,
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
