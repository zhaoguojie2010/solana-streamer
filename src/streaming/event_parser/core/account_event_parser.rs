//! Account classification precedes optional snapshot decoding.
use super::dispatcher::EventDispatcher;
use crate::streaming::event_parser::{
    common::{EventMetadata, EventType, ProtocolType},
    AccountEvent, ParsePlan,
};
use crate::streaming::grpc::AccountFrame;
use serde::{Deserialize, Serialize};
use solana_account_decoder::parse_nonce::parse_nonce;
use solana_sdk::pubkey::Pubkey;
use spl_token::solana_program::program_pack::Pack;
use spl_token::state::{Account, Mint};
use spl_token_2022::{
    extension::StateWithExtensions,
    state::{Account as Account2022, Mint as Mint2022},
};

/// 通用账户事件
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenAccountEvent {
    pub metadata: EventMetadata,
    pub pubkey: Pubkey,
    pub executable: bool,
    pub lamports: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    pub amount: Option<u64>,
    pub token_owner: Pubkey,
}

/// Nonce account event
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonceAccountEvent {
    pub metadata: EventMetadata,
    pub pubkey: Pubkey,
    pub executable: bool,
    pub lamports: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    pub nonce: String,
    pub authority: String,
}

/// Nonce account event
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenInfoEvent {
    pub metadata: EventMetadata,
    pub pubkey: Pubkey,
    pub executable: bool,
    pub lamports: u64,
    pub owner: Pubkey,
    pub rent_epoch: u64,
    pub supply: u64,
    pub decimals: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct AccountView<'a> {
    pub frame: &'a AccountFrame,
    pub event_type: EventType,
}
impl AccountFrame {
    pub fn view<'a>(&'a self, plan: &ParsePlan) -> Option<AccountView<'a>> {
        let event_type = account_event_type(self, plan)?;
        plan.includes(event_type).then_some(AccountView { frame: self, event_type })
    }
}
impl AccountView<'_> {
    pub fn bytes(&self) -> &[u8] {
        &self.frame.data
    }
    pub fn u64_at(&self, offset: usize) -> Option<u64> {
        Some(u64::from_le_bytes(self.bytes().get(offset..offset.checked_add(8)?)?.try_into().ok()?))
    }
    pub fn u128_at(&self, offset: usize) -> Option<u128> {
        Some(u128::from_le_bytes(
            self.bytes().get(offset..offset.checked_add(16)?)?.try_into().ok()?,
        ))
    }
    pub fn pubkey_at(&self, offset: usize) -> Option<Pubkey> {
        Some(Pubkey::new_from_array(
            self.bytes().get(offset..offset.checked_add(32)?)?.try_into().ok()?,
        ))
    }
    /// Decode only when requested. Cache the returned snapshot in the consumer if needed.
    /// Cloning the frame here shares `Bytes`; it does not copy the account payload.
    pub fn decode(&self) -> Option<AccountEvent> {
        let metadata = EventMetadata {
            event_type: self.event_type,
            protocol: ProtocolType::Common,
            program_id: self.frame.owner,
            ..EventMetadata::default()
        };
        if let Some(protocol) = EventDispatcher::match_protocol_by_program_id(&self.frame.owner) {
            return EventDispatcher::dispatch_account(
                protocol,
                self.bytes().get(..8)?,
                self.frame.clone(),
                metadata,
            );
        }
        match self.event_type {
            EventType::TokenAccount => parse_token(self.frame, metadata),
            EventType::NonceAccount => {
                let solana_account_decoder::parse_nonce::UiNonceState::Initialized(details) =
                    parse_nonce(self.bytes()).ok()?
                else {
                    return None;
                };
                Some(AccountEvent::NonceAccountEvent(NonceAccountEvent {
                    metadata,
                    pubkey: self.frame.pubkey,
                    executable: self.frame.executable,
                    lamports: self.frame.lamports,
                    owner: self.frame.owner,
                    rent_epoch: self.frame.rent_epoch,
                    nonce: details.blockhash,
                    authority: details.authority,
                }))
            }
            _ => None,
        }
    }
}
fn parse_token(account: &AccountFrame, metadata: EventMetadata) -> Option<AccountEvent> {
    let token2022 = account.owner.to_bytes() == spl_token_2022::ID.to_bytes();
    let mint = if token2022 {
        StateWithExtensions::<Mint2022>::unpack(&account.data)
            .ok()
            .map(|m| (m.base.supply, m.base.decimals))
    } else {
        Mint::unpack(&account.data).ok().map(|m| (m.supply, m.decimals))
    };
    if let Some((supply, decimals)) = mint {
        return Some(AccountEvent::TokenInfoEvent(TokenInfoEvent {
            metadata,
            pubkey: account.pubkey,
            executable: account.executable,
            lamports: account.lamports,
            owner: account.owner,
            rent_epoch: account.rent_epoch,
            supply,
            decimals,
        }));
    }
    let (amount, token_owner) = if token2022 {
        let state = StateWithExtensions::<Account2022>::unpack(&account.data).ok()?;
        (state.base.amount, state.base.owner)
    } else {
        let state = Account::unpack(&account.data).ok()?;
        (state.amount, state.owner)
    };
    Some(AccountEvent::TokenAccountEvent(TokenAccountEvent {
        metadata,
        pubkey: account.pubkey,
        executable: account.executable,
        lamports: account.lamports,
        owner: account.owner,
        rent_epoch: account.rent_epoch,
        amount: Some(amount),
        token_owner: Pubkey::new_from_array(token_owner.to_bytes()),
    }))
}
fn account_event_type(account: &AccountFrame, plan: &ParsePlan) -> Option<EventType> {
    use crate::streaming::event_parser::protocols::*;
    if let Some(protocol) = EventDispatcher::match_protocol_by_program_id(&account.owner) {
        if !plan.includes_protocol(protocol) {
            return None;
        }
        let disc = account.data.get(..8)?;
        return match protocol {
            Protocol::PancakeSwap => pancakeswap::parser::account_event_type(disc),
            Protocol::PumpSwap => pumpswap::parser::account_event_type(disc),
            Protocol::PumpFun => pumpfun::parser::account_event_type(disc),
            Protocol::Bonk => bonk::parser::account_event_type(disc),
            Protocol::RaydiumCpmm => raydium_cpmm::parser::account_event_type(disc),
            Protocol::RaydiumClmm => raydium_clmm::parser::account_event_type(disc),
            Protocol::RaydiumAmmV4 => raydium_amm_v4::parser::account_event_type(disc),
            Protocol::MeteoraDammV2 => meteora_damm_v2::parser::account_event_type(disc),
            Protocol::MeteoraDlmm => meteora_dlmm::parser::account_event_type(disc),
            Protocol::Whirlpool => whirlpool::parser::account_event_type(disc),
        };
    }
    if (account.owner.to_bytes() == spl_token::ID.to_bytes()
        || account.owner.to_bytes() == spl_token_2022::ID.to_bytes())
        && account.data.len() >= Mint::LEN
    {
        return Some(EventType::TokenAccount);
    }
    if account.owner == Pubkey::default() && account.data.len() == 80 {
        return Some(EventType::NonceAccount);
    }
    None
}
