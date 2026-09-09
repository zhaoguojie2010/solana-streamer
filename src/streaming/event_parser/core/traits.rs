use crate::streaming::event_parser::common::EventMetadata;
use crate::streaming::event_parser::core::account_event_parser::{
    NonceAccountEvent, TokenAccountEvent, TokenInfoEvent,
};
use crate::streaming::event_parser::core::common_event_parser::{
    SetComputeUnitLimitEvent, SetComputeUnitPriceEvent,
};
use crate::streaming::event_parser::protocols::bonk::events::*;
use crate::streaming::event_parser::protocols::meteora_damm_v2::events::*;
use crate::streaming::event_parser::protocols::meteora_dlmm::events::*;
use crate::streaming::event_parser::protocols::pancakeswap::events::*;
use crate::streaming::event_parser::protocols::pumpfun::events::*;
use crate::streaming::event_parser::protocols::pumpswap::events::*;
use crate::streaming::event_parser::protocols::raydium_amm_v4::events::*;
use crate::streaming::event_parser::protocols::raydium_clmm::events::*;
use crate::streaming::event_parser::protocols::raydium_cpmm::events::*;
use crate::streaming::event_parser::protocols::whirlpool::events::*;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::fmt::Debug;

/// Execution evidence carried by the source that produced a transaction batch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TxExecutionStatus {
    #[default]
    Unknown,
    Success,
    Failed,
}

/// 交易级 swap 形态分类，用于下游快速判断一笔 tx 的池子结构。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TxSwapKind {
    /// 仅涉及单池单次 swap（未构成闭环，也没有同 mint 跨池）。
    #[default]
    SimpleSwap,
    /// 闭环套利：同一外层指令内，DEX swap 的 mint/account legs 形成环。
    Arb,
    /// 跨池路由/拆单：整个 tx 内同一非稳定币 mint 出现在 ≥2 个不同池子。
    Route,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxTokenBalanceChange {
    pub account_index: u32,
    pub account: Option<Pubkey>,
    pub mint: String,
    pub owner: String,
    pub program_id: String,
    pub decimals: u32,
    pub pre_amount: Option<u64>,
    pub post_amount: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxExecutionMetaAudit {
    pub token_balance_changes: Vec<TxTokenBalanceChange>,
    pub parse_errors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TxEvent {
    PancakeSwapSwapEvent(PancakeSwapSwapEvent),
    PancakeSwapSwapV2Event(PancakeSwapSwapV2Event),
    BonkTradeEvent(BonkTradeEvent),
    BonkPoolCreateEvent(BonkPoolCreateEvent),
    BonkMigrateToAmmEvent(BonkMigrateToAmmEvent),
    BonkMigrateToCpswapEvent(BonkMigrateToCpswapEvent),
    PumpFunCreateTokenEvent(PumpFunCreateTokenEvent),
    PumpFunCreateV2TokenEvent(PumpFunCreateV2TokenEvent),
    PumpFunTradeEvent(PumpFunTradeEvent),
    PumpFunMigrateEvent(PumpFunMigrateEvent),
    PumpSwapBuyEvent(PumpSwapBuyEvent),
    PumpSwapBuyExactQuoteInEvent(PumpSwapBuyExactQuoteInEvent),
    PumpSwapSellEvent(PumpSwapSellEvent),
    PumpSwapCreatePoolEvent(PumpSwapCreatePoolEvent),
    PumpSwapInitBoostEvent(PumpSwapInitBoostEvent),
    PumpSwapDepositEvent(PumpSwapDepositEvent),
    PumpSwapWithdrawEvent(PumpSwapWithdrawEvent),
    RaydiumAmmV4SwapEvent(RaydiumAmmV4SwapEvent),
    RaydiumAmmV4DepositEvent(RaydiumAmmV4DepositEvent),
    RaydiumAmmV4WithdrawEvent(RaydiumAmmV4WithdrawEvent),
    RaydiumAmmV4WithdrawPnlEvent(RaydiumAmmV4WithdrawPnlEvent),
    RaydiumAmmV4Initialize2Event(RaydiumAmmV4Initialize2Event),
    RaydiumClmmSwapEvent(RaydiumClmmSwapEvent),
    RaydiumClmmSwapV2Event(RaydiumClmmSwapV2Event),
    RaydiumClmmInstructionEvent(RaydiumClmmInstructionEvent),
    RaydiumClmmClosePositionEvent(RaydiumClmmClosePositionEvent),
    RaydiumClmmIncreaseLiquidityV2Event(RaydiumClmmIncreaseLiquidityV2Event),
    RaydiumClmmDecreaseLiquidityV2Event(RaydiumClmmDecreaseLiquidityV2Event),
    RaydiumClmmCreatePoolEvent(RaydiumClmmCreatePoolEvent),
    RaydiumClmmOpenPositionWithToken22NftEvent(RaydiumClmmOpenPositionWithToken22NftEvent),
    RaydiumClmmOpenPositionV2Event(RaydiumClmmOpenPositionV2Event),
    RaydiumCpmmSwapEvent(RaydiumCpmmSwapEvent),
    RaydiumCpmmDepositEvent(RaydiumCpmmDepositEvent),
    RaydiumCpmmWithdrawEvent(RaydiumCpmmWithdrawEvent),
    RaydiumCpmmInitializeEvent(RaydiumCpmmInitializeEvent),
    MeteoraDammV2SwapEvent(MeteoraDammV2SwapEvent),
    MeteoraDammV2Swap2Event(MeteoraDammV2Swap2Event),
    MeteoraDammV2InitializePoolEvent(MeteoraDammV2InitializePoolEvent),
    MeteoraDammV2InitializeCustomizablePoolEvent(MeteoraDammV2InitializeCustomizablePoolEvent),
    MeteoraDammV2InitializePoolWithDynamicConfigEvent(
        MeteoraDammV2InitializePoolWithDynamicConfigEvent,
    ),
    MeteoraDammV2LiquidityChangeEvent(MeteoraDammV2LiquidityChangeEvent),
    MeteoraDammV2InstructionEvent(MeteoraDammV2InstructionEvent),
    MeteoraDlmmSwapEvent(MeteoraDlmmSwapEvent),
    MeteoraDlmmSwap2Event(MeteoraDlmmSwap2Event),
    MeteoraDlmmInstructionEvent(MeteoraDlmmInstructionEvent),
    WhirlpoolSwapEvent(WhirlpoolSwapEvent),
    WhirlpoolSwapV2Event(WhirlpoolSwapV2Event),
    WhirlpoolInstructionEvent(WhirlpoolInstructionEvent),
    SetComputeUnitLimitEvent(SetComputeUnitLimitEvent),
    SetComputeUnitPriceEvent(SetComputeUnitPriceEvent),
}
impl TxEvent {
    pub fn metadata(&self) -> &EventMetadata {
        match self {
            Self::PancakeSwapSwapEvent(e) => &e.metadata,
            Self::PancakeSwapSwapV2Event(e) => &e.metadata,
            Self::BonkTradeEvent(e) => &e.metadata,
            Self::BonkPoolCreateEvent(e) => &e.metadata,
            Self::BonkMigrateToAmmEvent(e) => &e.metadata,
            Self::BonkMigrateToCpswapEvent(e) => &e.metadata,
            Self::PumpFunCreateTokenEvent(e) => &e.metadata,
            Self::PumpFunCreateV2TokenEvent(e) => &e.metadata,
            Self::PumpFunTradeEvent(e) => &e.metadata,
            Self::PumpFunMigrateEvent(e) => &e.metadata,
            Self::PumpSwapBuyEvent(e) => &e.metadata,
            Self::PumpSwapBuyExactQuoteInEvent(e) => &e.metadata,
            Self::PumpSwapSellEvent(e) => &e.metadata,
            Self::PumpSwapCreatePoolEvent(e) => &e.metadata,
            Self::PumpSwapInitBoostEvent(e) => &e.metadata,
            Self::PumpSwapDepositEvent(e) => &e.metadata,
            Self::PumpSwapWithdrawEvent(e) => &e.metadata,
            Self::RaydiumAmmV4SwapEvent(e) => &e.metadata,
            Self::RaydiumAmmV4DepositEvent(e) => &e.metadata,
            Self::RaydiumAmmV4WithdrawEvent(e) => &e.metadata,
            Self::RaydiumAmmV4WithdrawPnlEvent(e) => &e.metadata,
            Self::RaydiumAmmV4Initialize2Event(e) => &e.metadata,
            Self::RaydiumClmmSwapEvent(e) => &e.metadata,
            Self::RaydiumClmmSwapV2Event(e) => &e.metadata,
            Self::RaydiumClmmInstructionEvent(e) => &e.metadata,
            Self::RaydiumClmmClosePositionEvent(e) => &e.metadata,
            Self::RaydiumClmmIncreaseLiquidityV2Event(e) => &e.metadata,
            Self::RaydiumClmmDecreaseLiquidityV2Event(e) => &e.metadata,
            Self::RaydiumClmmCreatePoolEvent(e) => &e.metadata,
            Self::RaydiumClmmOpenPositionWithToken22NftEvent(e) => &e.metadata,
            Self::RaydiumClmmOpenPositionV2Event(e) => &e.metadata,
            Self::RaydiumCpmmSwapEvent(e) => &e.metadata,
            Self::RaydiumCpmmDepositEvent(e) => &e.metadata,
            Self::RaydiumCpmmWithdrawEvent(e) => &e.metadata,
            Self::RaydiumCpmmInitializeEvent(e) => &e.metadata,
            Self::MeteoraDammV2SwapEvent(e) => &e.metadata,
            Self::MeteoraDammV2Swap2Event(e) => &e.metadata,
            Self::MeteoraDammV2InitializePoolEvent(e) => &e.metadata,
            Self::MeteoraDammV2InitializeCustomizablePoolEvent(e) => &e.metadata,
            Self::MeteoraDammV2InitializePoolWithDynamicConfigEvent(e) => &e.metadata,
            Self::MeteoraDammV2LiquidityChangeEvent(e) => &e.metadata,
            Self::MeteoraDammV2InstructionEvent(e) => &e.metadata,
            Self::MeteoraDlmmSwapEvent(e) => &e.metadata,
            Self::MeteoraDlmmSwap2Event(e) => &e.metadata,
            Self::MeteoraDlmmInstructionEvent(e) => &e.metadata,
            Self::WhirlpoolSwapEvent(e) => &e.metadata,
            Self::WhirlpoolSwapV2Event(e) => &e.metadata,
            Self::WhirlpoolInstructionEvent(e) => &e.metadata,
            Self::SetComputeUnitLimitEvent(e) => &e.metadata,
            Self::SetComputeUnitPriceEvent(e) => &e.metadata,
        }
    }
    pub fn metadata_mut(&mut self) -> &mut EventMetadata {
        match self {
            Self::PancakeSwapSwapEvent(e) => &mut e.metadata,
            Self::PancakeSwapSwapV2Event(e) => &mut e.metadata,
            Self::BonkTradeEvent(e) => &mut e.metadata,
            Self::BonkPoolCreateEvent(e) => &mut e.metadata,
            Self::BonkMigrateToAmmEvent(e) => &mut e.metadata,
            Self::BonkMigrateToCpswapEvent(e) => &mut e.metadata,
            Self::PumpFunCreateTokenEvent(e) => &mut e.metadata,
            Self::PumpFunCreateV2TokenEvent(e) => &mut e.metadata,
            Self::PumpFunTradeEvent(e) => &mut e.metadata,
            Self::PumpFunMigrateEvent(e) => &mut e.metadata,
            Self::PumpSwapBuyEvent(e) => &mut e.metadata,
            Self::PumpSwapBuyExactQuoteInEvent(e) => &mut e.metadata,
            Self::PumpSwapSellEvent(e) => &mut e.metadata,
            Self::PumpSwapCreatePoolEvent(e) => &mut e.metadata,
            Self::PumpSwapInitBoostEvent(e) => &mut e.metadata,
            Self::PumpSwapDepositEvent(e) => &mut e.metadata,
            Self::PumpSwapWithdrawEvent(e) => &mut e.metadata,
            Self::RaydiumAmmV4SwapEvent(e) => &mut e.metadata,
            Self::RaydiumAmmV4DepositEvent(e) => &mut e.metadata,
            Self::RaydiumAmmV4WithdrawEvent(e) => &mut e.metadata,
            Self::RaydiumAmmV4WithdrawPnlEvent(e) => &mut e.metadata,
            Self::RaydiumAmmV4Initialize2Event(e) => &mut e.metadata,
            Self::RaydiumClmmSwapEvent(e) => &mut e.metadata,
            Self::RaydiumClmmSwapV2Event(e) => &mut e.metadata,
            Self::RaydiumClmmInstructionEvent(e) => &mut e.metadata,
            Self::RaydiumClmmClosePositionEvent(e) => &mut e.metadata,
            Self::RaydiumClmmIncreaseLiquidityV2Event(e) => &mut e.metadata,
            Self::RaydiumClmmDecreaseLiquidityV2Event(e) => &mut e.metadata,
            Self::RaydiumClmmCreatePoolEvent(e) => &mut e.metadata,
            Self::RaydiumClmmOpenPositionWithToken22NftEvent(e) => &mut e.metadata,
            Self::RaydiumClmmOpenPositionV2Event(e) => &mut e.metadata,
            Self::RaydiumCpmmSwapEvent(e) => &mut e.metadata,
            Self::RaydiumCpmmDepositEvent(e) => &mut e.metadata,
            Self::RaydiumCpmmWithdrawEvent(e) => &mut e.metadata,
            Self::RaydiumCpmmInitializeEvent(e) => &mut e.metadata,
            Self::MeteoraDammV2SwapEvent(e) => &mut e.metadata,
            Self::MeteoraDammV2Swap2Event(e) => &mut e.metadata,
            Self::MeteoraDammV2InitializePoolEvent(e) => &mut e.metadata,
            Self::MeteoraDammV2InitializeCustomizablePoolEvent(e) => &mut e.metadata,
            Self::MeteoraDammV2InitializePoolWithDynamicConfigEvent(e) => &mut e.metadata,
            Self::MeteoraDammV2LiquidityChangeEvent(e) => &mut e.metadata,
            Self::MeteoraDammV2InstructionEvent(e) => &mut e.metadata,
            Self::MeteoraDlmmSwapEvent(e) => &mut e.metadata,
            Self::MeteoraDlmmSwap2Event(e) => &mut e.metadata,
            Self::MeteoraDlmmInstructionEvent(e) => &mut e.metadata,
            Self::WhirlpoolSwapEvent(e) => &mut e.metadata,
            Self::WhirlpoolSwapV2Event(e) => &mut e.metadata,
            Self::WhirlpoolInstructionEvent(e) => &mut e.metadata,
            Self::SetComputeUnitLimitEvent(e) => &mut e.metadata,
            Self::SetComputeUnitPriceEvent(e) => &mut e.metadata,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AccountEvent {
    PancakeSwapPoolStateAccountEvent(PancakeSwapPoolStateAccountEvent),
    PancakeSwapTickArrayStateAccountEvent(PancakeSwapTickArrayStateAccountEvent),
    PancakeSwapTickArrayBitmapExtensionAccountEvent(
        PancakeSwapTickArrayBitmapExtensionAccountEvent,
    ),
    BonkPoolStateAccountEvent(BonkPoolStateAccountEvent),
    BonkGlobalConfigAccountEvent(BonkGlobalConfigAccountEvent),
    BonkPlatformConfigAccountEvent(BonkPlatformConfigAccountEvent),
    PumpFunBondingCurveAccountEvent(PumpFunBondingCurveAccountEvent),
    PumpFunGlobalAccountEvent(PumpFunGlobalAccountEvent),
    PumpSwapGlobalConfigAccountEvent(PumpSwapGlobalConfigAccountEvent),
    PumpSwapPoolAccountEvent(PumpSwapPoolAccountEvent),
    RaydiumAmmV4AmmInfoAccountEvent(RaydiumAmmV4AmmInfoAccountEvent),
    RaydiumClmmAmmConfigAccountEvent(RaydiumClmmAmmConfigAccountEvent),
    RaydiumClmmPoolStateAccountEvent(RaydiumClmmPoolStateAccountEvent),
    RaydiumClmmTickArrayStateAccountEvent(RaydiumClmmTickArrayStateAccountEvent),
    RaydiumClmmTickArrayBitmapExtensionAccountEvent(
        RaydiumClmmTickArrayBitmapExtensionAccountEvent,
    ),
    RaydiumCpmmAmmConfigAccountEvent(RaydiumCpmmAmmConfigAccountEvent),
    RaydiumCpmmPoolStateAccountEvent(RaydiumCpmmPoolStateAccountEvent),
    MeteoraDammV2PoolStateAccountEvent(MeteoraDammV2PoolStateAccountEvent),
    MeteoraDlmmLbPairAccountEvent(MeteoraDlmmLbPairAccountEvent),
    MeteoraDlmmBinArrayAccountEvent(MeteoraDlmmBinArrayAccountEvent),
    MeteoraDlmmBinArrayBitmapExtensionAccountEvent(MeteoraDlmmBinArrayBitmapExtensionAccountEvent),
    WhirlpoolAccountEvent(WhirlpoolAccountEvent),
    WhirlpoolTickArrayAccountEvent(WhirlpoolTickArrayAccountEvent),
    TokenAccountEvent(TokenAccountEvent),
    NonceAccountEvent(NonceAccountEvent),
    TokenInfoEvent(TokenInfoEvent),
}
impl AccountEvent {
    pub fn metadata(&self) -> &EventMetadata {
        match self {
            Self::PancakeSwapPoolStateAccountEvent(e) => &e.metadata,
            Self::PancakeSwapTickArrayStateAccountEvent(e) => &e.metadata,
            Self::PancakeSwapTickArrayBitmapExtensionAccountEvent(e) => &e.metadata,
            Self::BonkPoolStateAccountEvent(e) => &e.metadata,
            Self::BonkGlobalConfigAccountEvent(e) => &e.metadata,
            Self::BonkPlatformConfigAccountEvent(e) => &e.metadata,
            Self::PumpFunBondingCurveAccountEvent(e) => &e.metadata,
            Self::PumpFunGlobalAccountEvent(e) => &e.metadata,
            Self::PumpSwapGlobalConfigAccountEvent(e) => &e.metadata,
            Self::PumpSwapPoolAccountEvent(e) => &e.metadata,
            Self::RaydiumAmmV4AmmInfoAccountEvent(e) => &e.metadata,
            Self::RaydiumClmmAmmConfigAccountEvent(e) => &e.metadata,
            Self::RaydiumClmmPoolStateAccountEvent(e) => &e.metadata,
            Self::RaydiumClmmTickArrayStateAccountEvent(e) => &e.metadata,
            Self::RaydiumClmmTickArrayBitmapExtensionAccountEvent(e) => &e.metadata,
            Self::RaydiumCpmmAmmConfigAccountEvent(e) => &e.metadata,
            Self::RaydiumCpmmPoolStateAccountEvent(e) => &e.metadata,
            Self::MeteoraDammV2PoolStateAccountEvent(e) => &e.metadata,
            Self::MeteoraDlmmLbPairAccountEvent(e) => &e.metadata,
            Self::MeteoraDlmmBinArrayAccountEvent(e) => &e.metadata,
            Self::MeteoraDlmmBinArrayBitmapExtensionAccountEvent(e) => &e.metadata,
            Self::WhirlpoolAccountEvent(e) => &e.metadata,
            Self::WhirlpoolTickArrayAccountEvent(e) => &e.metadata,
            Self::TokenAccountEvent(e) => &e.metadata,
            Self::NonceAccountEvent(e) => &e.metadata,
            Self::TokenInfoEvent(e) => &e.metadata,
        }
    }
    pub fn metadata_mut(&mut self) -> &mut EventMetadata {
        match self {
            Self::PancakeSwapPoolStateAccountEvent(e) => &mut e.metadata,
            Self::PancakeSwapTickArrayStateAccountEvent(e) => &mut e.metadata,
            Self::PancakeSwapTickArrayBitmapExtensionAccountEvent(e) => &mut e.metadata,
            Self::BonkPoolStateAccountEvent(e) => &mut e.metadata,
            Self::BonkGlobalConfigAccountEvent(e) => &mut e.metadata,
            Self::BonkPlatformConfigAccountEvent(e) => &mut e.metadata,
            Self::PumpFunBondingCurveAccountEvent(e) => &mut e.metadata,
            Self::PumpFunGlobalAccountEvent(e) => &mut e.metadata,
            Self::PumpSwapGlobalConfigAccountEvent(e) => &mut e.metadata,
            Self::PumpSwapPoolAccountEvent(e) => &mut e.metadata,
            Self::RaydiumAmmV4AmmInfoAccountEvent(e) => &mut e.metadata,
            Self::RaydiumClmmAmmConfigAccountEvent(e) => &mut e.metadata,
            Self::RaydiumClmmPoolStateAccountEvent(e) => &mut e.metadata,
            Self::RaydiumClmmTickArrayStateAccountEvent(e) => &mut e.metadata,
            Self::RaydiumClmmTickArrayBitmapExtensionAccountEvent(e) => &mut e.metadata,
            Self::RaydiumCpmmAmmConfigAccountEvent(e) => &mut e.metadata,
            Self::RaydiumCpmmPoolStateAccountEvent(e) => &mut e.metadata,
            Self::MeteoraDammV2PoolStateAccountEvent(e) => &mut e.metadata,
            Self::MeteoraDlmmLbPairAccountEvent(e) => &mut e.metadata,
            Self::MeteoraDlmmBinArrayAccountEvent(e) => &mut e.metadata,
            Self::MeteoraDlmmBinArrayBitmapExtensionAccountEvent(e) => &mut e.metadata,
            Self::WhirlpoolAccountEvent(e) => &mut e.metadata,
            Self::WhirlpoolTickArrayAccountEvent(e) => &mut e.metadata,
            Self::TokenAccountEvent(e) => &mut e.metadata,
            Self::NonceAccountEvent(e) => &mut e.metadata,
            Self::TokenInfoEvent(e) => &mut e.metadata,
        }
    }
}
