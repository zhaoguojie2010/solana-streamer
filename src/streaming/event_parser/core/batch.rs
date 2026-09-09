use super::{
    frame::{InstructionFrame, InstructionView, TxMetadata},
    traits::{TxEvent, TxExecutionMetaAudit, TxSwapKind},
};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxSummary {
    pub compute_unit_price: Option<u64>,
    pub compute_unit_limit: Option<u32>,
    pub is_jito: Option<bool>,
    pub swap_kind: Option<TxSwapKind>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TxBatch {
    pub meta: TxMetadata,
    pub keys: Vec<Pubkey>,
    pub events: Vec<TxEvent>,
    pub summary: TxSummary,
    pub audit: Option<TxExecutionMetaAudit>,
    pub(crate) source: Option<Vec<InstructionFrame>>,
    pub logs: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug)]
pub struct TxView<'a> {
    pub meta: &'a TxMetadata,
    pub keys: &'a [Pubkey],
    pub events: &'a [TxEvent],
    pub summary: &'a TxSummary,
    pub audit: Option<&'a TxExecutionMetaAudit>,
    pub(crate) source: Option<&'a [InstructionFrame]>,
    pub logs: Option<&'a [String]>,
}

impl TxBatch {
    pub fn view(&self) -> TxView<'_> {
        TxView {
            meta: &self.meta,
            keys: &self.keys,
            events: &self.events,
            summary: &self.summary,
            audit: self.audit.as_ref(),
            source: self.source.as_deref(),
            logs: self.logs.as_deref(),
        }
    }
    pub fn instruction(&self, index: u32) -> Option<InstructionView<'_>> {
        self.view().instruction(index)
    }
}
impl<'a> TxView<'a> {
    pub fn instruction(self, index: u32) -> Option<InstructionView<'a>> {
        self.source?.get(index as usize).map(|ix| ix.view(self.keys))
    }
    pub fn instructions(self) -> impl Iterator<Item = InstructionView<'a>> {
        self.source.unwrap_or_default().iter().map(move |ix| ix.view(self.keys))
    }
    /// Explicitly copy a borrowed batch when a consumer needs to retain it.
    /// Prefer `parse_owned` / queued delivery when every batch must be retained.
    pub fn to_owned(self) -> TxBatch {
        TxBatch {
            meta: self.meta.clone(),
            keys: self.keys.to_vec(),
            events: self.events.to_vec(),
            summary: self.summary.clone(),
            audit: self.audit.cloned(),
            source: self.source.map(<[_]>::to_vec),
            logs: self.logs.map(<[_]>::to_vec),
        }
    }
}

impl TxEvent {
    pub fn requires_instruction_source(&self) -> bool {
        matches!(
            self,
            Self::RaydiumClmmInstructionEvent(_)
                | Self::MeteoraDlmmInstructionEvent(_)
                | Self::MeteoraDammV2InstructionEvent(_)
                | Self::WhirlpoolInstructionEvent(_)
        )
    }
}

fn vec_bytes<T>(v: &Vec<T>) -> usize {
    v.capacity() * std::mem::size_of::<T>()
}
impl TxEvent {
    fn heap_bytes(&self) -> usize {
        let fields = match self {
            Self::PancakeSwapSwapEvent(e) => vec_bytes(&e.remaining_account_indices),
            Self::PancakeSwapSwapV2Event(e) => vec_bytes(&e.remaining_account_indices),
            Self::BonkTradeEvent(_) => 0,
            Self::BonkPoolCreateEvent(e) => {
                e.base_mint_param.name.capacity()
                    + e.base_mint_param.symbol.capacity()
                    + e.base_mint_param.uri.capacity()
            }
            Self::BonkMigrateToAmmEvent(_) => 0,
            Self::BonkMigrateToCpswapEvent(e) => vec_bytes(&e.remaining_account_indices),
            Self::PumpFunCreateTokenEvent(e) => {
                e.name.capacity() + e.symbol.capacity() + e.uri.capacity()
            }
            Self::PumpFunCreateV2TokenEvent(e) => {
                e.name.capacity() + e.symbol.capacity() + e.uri.capacity()
            }
            Self::PumpFunTradeEvent(_) => 0,
            Self::PumpFunMigrateEvent(_) => 0,
            Self::PumpSwapBuyEvent(e) => e.ix_name.capacity(),
            Self::PumpSwapBuyExactQuoteInEvent(_) => 0,
            Self::PumpSwapSellEvent(_) => 0,
            Self::PumpSwapCreatePoolEvent(_) => 0,
            Self::PumpSwapInitBoostEvent(_) => 0,
            Self::PumpSwapDepositEvent(_) => 0,
            Self::PumpSwapWithdrawEvent(_) => 0,
            Self::RaydiumAmmV4SwapEvent(_) => 0,
            Self::RaydiumAmmV4DepositEvent(_) => 0,
            Self::RaydiumAmmV4WithdrawEvent(_) => 0,
            Self::RaydiumAmmV4WithdrawPnlEvent(_) => 0,
            Self::RaydiumAmmV4Initialize2Event(_) => 0,
            Self::RaydiumClmmSwapEvent(e) => vec_bytes(&e.remaining_account_indices),
            Self::RaydiumClmmSwapV2Event(e) => vec_bytes(&e.remaining_account_indices),
            Self::RaydiumClmmInstructionEvent(e) => vec_bytes(&e.execution_events),
            Self::RaydiumClmmClosePositionEvent(_) => 0,
            Self::RaydiumClmmIncreaseLiquidityV2Event(_) => 0,
            Self::RaydiumClmmDecreaseLiquidityV2Event(e) => vec_bytes(&e.remaining_account_indices),
            Self::RaydiumClmmCreatePoolEvent(_) => 0,
            Self::RaydiumClmmOpenPositionWithToken22NftEvent(_) => 0,
            Self::RaydiumClmmOpenPositionV2Event(e) => vec_bytes(&e.remaining_account_indices),
            Self::RaydiumCpmmSwapEvent(_) => 0,
            Self::RaydiumCpmmDepositEvent(_) => 0,
            Self::RaydiumCpmmWithdrawEvent(_) => 0,
            Self::RaydiumCpmmInitializeEvent(_) => 0,
            Self::MeteoraDammV2SwapEvent(_) => 0,
            Self::MeteoraDammV2Swap2Event(_) => 0,
            Self::MeteoraDammV2InitializePoolEvent(e) => vec_bytes(&e.remaining_account_indices),
            Self::MeteoraDammV2InitializeCustomizablePoolEvent(e) => {
                vec_bytes(&e.remaining_account_indices)
            }
            Self::MeteoraDammV2InitializePoolWithDynamicConfigEvent(_) => 0,
            Self::MeteoraDammV2LiquidityChangeEvent(_) => 0,
            Self::MeteoraDammV2InstructionEvent(_) => 0,
            Self::MeteoraDlmmSwapEvent(e) => vec_bytes(&e.remaining_account_indices),
            Self::MeteoraDlmmSwap2Event(e) => vec_bytes(&e.remaining_account_indices),
            Self::MeteoraDlmmInstructionEvent(_) => 0,
            Self::WhirlpoolSwapEvent(e) => vec_bytes(&e.remaining_account_indices),
            Self::WhirlpoolSwapV2Event(e) => vec_bytes(&e.remaining_account_indices),
            Self::WhirlpoolInstructionEvent(e) => vec_bytes(&e.execution_events),
            Self::SetComputeUnitLimitEvent(_) => 0,
            Self::SetComputeUnitPriceEvent(_) => 0,
        };
        fields
            + self.metadata().swap_data.as_ref().map_or(0, |data| {
                std::mem::size_of_val(data.as_ref())
                    + match &data.description {
                        Some(std::borrow::Cow::Owned(s)) => s.capacity(),
                        _ => 0,
                    }
            })
    }
}
impl TxBatch {
    /// Payload capacity charged by the owned queue. Excludes allocator/channel overhead.
    pub fn retained_bytes(&self) -> usize {
        let source = self.source.as_ref().map_or(0, |ixs| {
            vec_bytes(ixs)
                + ixs.iter().map(|ix| ix.accounts.capacity() + ix.data.capacity()).sum::<usize>()
        });
        let logs = self
            .logs
            .as_ref()
            .map_or(0, |logs| vec_bytes(logs) + logs.iter().map(String::capacity).sum::<usize>());
        let audit = self.audit.as_ref().map_or(0, |a| {
            vec_bytes(&a.token_balance_changes)
                + a.token_balance_changes
                    .iter()
                    .map(|b| b.mint.capacity() + b.owner.capacity() + b.program_id.capacity())
                    .sum::<usize>()
                + vec_bytes(&a.parse_errors)
                + a.parse_errors.iter().map(String::capacity).sum::<usize>()
        });
        std::mem::size_of_val(self)
            + vec_bytes(&self.keys)
            + vec_bytes(&self.events)
            + self.events.iter().map(TxEvent::heap_bytes).sum::<usize>()
            + source
            + logs
            + audit
    }
}
