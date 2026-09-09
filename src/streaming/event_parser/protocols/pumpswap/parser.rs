use crate::streaming::event_parser::InstructionAccounts;
use crate::streaming::event_parser::{
    common::{read_u64_le, EventMetadata, EventType},
    protocols::pumpswap::{
        discriminators, pump_swap_buy_event_log_decode, pump_swap_create_pool_event_log_decode,
        pump_swap_deposit_event_log_decode, pump_swap_sell_event_log_decode,
        pump_swap_withdraw_event_log_decode, PumpSwapBuyEvent, PumpSwapBuyExactQuoteInEvent,
        PumpSwapCreatePoolEvent, PumpSwapDepositEvent, PumpSwapInitBoostEvent, PumpSwapSellEvent,
        PumpSwapWithdrawEvent,
    },
    TxEvent,
};
use solana_sdk::pubkey::Pubkey;

/// PumpSwap程序ID
pub const PUMPSWAP_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");

/// 解析 PumpSwap instruction data
///
/// 根据判别器路由到具体的 instruction 解析函数
pub fn parse_pumpswap_instruction_data(
    discriminator: &[u8],
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    metadata: EventMetadata,
) -> Option<TxEvent> {
    match discriminator {
        discriminators::BUY_IX => parse_buy_instruction(data, accounts, metadata),
        discriminators::BUY_EXACT_QUOTE_IN_IX => {
            parse_buy_exact_quote_in_instruction(data, accounts, metadata)
        }
        discriminators::SELL_IX => parse_sell_instruction(data, accounts, metadata),
        discriminators::CREATE_POOL_IX => parse_create_pool_instruction(data, accounts, metadata),
        discriminators::INIT_BOOST_IX => parse_init_boost_instruction(accounts, metadata),
        discriminators::DEPOSIT_IX => parse_deposit_instruction(data, accounts, metadata),
        discriminators::WITHDRAW_IX => parse_withdraw_instruction(data, accounts, metadata),
        _ => None,
    }
}

/// 解析 PumpSwap inner instruction data
///
/// 根据判别器路由到具体的 inner instruction 解析函数
pub fn parse_pumpswap_inner_instruction_data(
    discriminator: &[u8],
    data: &[u8],
    metadata: EventMetadata,
) -> Option<TxEvent> {
    match discriminator {
        discriminators::BUY_EVENT => parse_buy_inner_instruction(data, metadata),
        discriminators::SELL_EVENT => parse_sell_inner_instruction(data, metadata),
        discriminators::CREATE_POOL_EVENT => parse_create_pool_inner_instruction(data, metadata),
        discriminators::INIT_BOOST_EVENT => parse_init_boost_inner_instruction(data, metadata),
        discriminators::DEPOSIT_EVENT => parse_deposit_inner_instruction(data, metadata),
        discriminators::WITHDRAW_EVENT => parse_withdraw_inner_instruction(data, metadata),
        _ => None,
    }
}

/// 解析 PumpSwap 账户数据
///
/// 根据判别器路由到具体的账户解析函数
pub fn parse_pumpswap_account_data(
    discriminator: &[u8],
    account: crate::streaming::grpc::AccountFrame,
    metadata: crate::streaming::event_parser::common::EventMetadata,
) -> Option<crate::streaming::event_parser::AccountEvent> {
    match discriminator {
        discriminators::GLOBAL_CONFIG_ACCOUNT => {
            crate::streaming::event_parser::protocols::pumpswap::types::global_config_parser(
                account, metadata,
            )
        }
        discriminators::POOL_ACCOUNT => {
            crate::streaming::event_parser::protocols::pumpswap::types::pool_parser(
                account, metadata,
            )
        }
        _ => None,
    }
}

/// 解析买入日志事件
fn parse_buy_inner_instruction(data: &[u8], metadata: EventMetadata) -> Option<TxEvent> {
    // Note: event_type will be set by instruction parser
    if let Some(event) = pump_swap_buy_event_log_decode(data) {
        Some(TxEvent::PumpSwapBuyEvent(PumpSwapBuyEvent { metadata, ..event }))
    } else {
        None
    }
}

/// 解析卖出日志事件
fn parse_sell_inner_instruction(data: &[u8], metadata: EventMetadata) -> Option<TxEvent> {
    // Note: event_type will be set by instruction parser
    if let Some(event) = pump_swap_sell_event_log_decode(data) {
        Some(TxEvent::PumpSwapSellEvent(PumpSwapSellEvent { metadata, ..event }))
    } else {
        None
    }
}

/// 解析创建池子日志事件
fn parse_create_pool_inner_instruction(data: &[u8], metadata: EventMetadata) -> Option<TxEvent> {
    // Note: event_type will be set by instruction parser
    if let Some(event) = pump_swap_create_pool_event_log_decode(data) {
        Some(TxEvent::PumpSwapCreatePoolEvent(PumpSwapCreatePoolEvent { metadata, ..event }))
    } else {
        None
    }
}

fn parse_init_boost_inner_instruction(data: &[u8], metadata: EventMetadata) -> Option<TxEvent> {
    let event = super::pump_swap_init_boost_event_log_decode(data)?;
    Some(TxEvent::PumpSwapInitBoostEvent(PumpSwapInitBoostEvent { metadata, ..event }))
}

/// 解析存款日志事件
fn parse_deposit_inner_instruction(data: &[u8], metadata: EventMetadata) -> Option<TxEvent> {
    // Note: event_type will be set by instruction parser
    if let Some(event) = pump_swap_deposit_event_log_decode(data) {
        Some(TxEvent::PumpSwapDepositEvent(PumpSwapDepositEvent { metadata, ..event }))
    } else {
        None
    }
}

/// 解析提款日志事件
fn parse_withdraw_inner_instruction(data: &[u8], metadata: EventMetadata) -> Option<TxEvent> {
    // Note: event_type will be set by instruction parser
    if let Some(event) = pump_swap_withdraw_event_log_decode(data) {
        Some(TxEvent::PumpSwapWithdrawEvent(PumpSwapWithdrawEvent { metadata, ..event }))
    } else {
        None
    }
}

/// 解析买入指令事件
fn parse_buy_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpSwapBuy;

    if data.len() < 16 || accounts.len() < 13 {
        return None;
    }

    let base_amount_out = read_u64_le(data, 0)?;
    let max_quote_amount_in = read_u64_le(data, 8)?;
    let track_volume = data.get(16).copied().unwrap_or(0) != 0;

    Some(TxEvent::PumpSwapBuyEvent(PumpSwapBuyEvent {
        metadata,
        base_amount_out,
        max_quote_amount_in,
        pool: *accounts.get(0)?,
        user: *accounts.get(1)?,
        base_mint: *accounts.get(3)?,
        quote_mint: *accounts.get(4)?,
        user_base_token_account: *accounts.get(5)?,
        user_quote_token_account: *accounts.get(6)?,
        pool_base_token_account: *accounts.get(7)?,
        pool_quote_token_account: *accounts.get(8)?,
        protocol_fee_recipient: *accounts.get(9)?,
        protocol_fee_recipient_token_account: *accounts.get(10)?,
        base_token_program: *accounts.get(11)?,
        quote_token_program: *accounts.get(12)?,
        coin_creator_vault_ata: accounts.get(17).copied().unwrap_or_default(),
        coin_creator_vault_authority: accounts.get(18).copied().unwrap_or_default(),
        track_volume,
        ..Default::default()
    }))
}

/// 解析买入指令事件（BuyExactQuoteIn）
///
/// 参数布局: quote_amount_in(u64), min_base_amount_out(u64), track_volume(OptionBool)
fn parse_buy_exact_quote_in_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpSwapBuyExactQuoteIn;

    if data.len() < 16 || accounts.len() < 13 {
        return None;
    }

    let quote_amount_in = read_u64_le(data, 0)?;
    let min_base_amount_out = read_u64_le(data, 8)?;
    let track_volume = data.get(16).copied().unwrap_or(0) != 0;

    Some(TxEvent::PumpSwapBuyExactQuoteInEvent(PumpSwapBuyExactQuoteInEvent {
        metadata,
        quote_amount_in,
        min_base_amount_out,
        user_quote_amount_in: quote_amount_in,
        pool: *accounts.get(0)?,
        user: *accounts.get(1)?,
        base_mint: *accounts.get(3)?,
        quote_mint: *accounts.get(4)?,
        user_base_token_account: *accounts.get(5)?,
        user_quote_token_account: *accounts.get(6)?,
        pool_base_token_account: *accounts.get(7)?,
        pool_quote_token_account: *accounts.get(8)?,
        protocol_fee_recipient: *accounts.get(9)?,
        protocol_fee_recipient_token_account: *accounts.get(10)?,
        base_token_program: *accounts.get(11)?,
        quote_token_program: *accounts.get(12)?,
        coin_creator_vault_ata: accounts.get(17).copied().unwrap_or_default(),
        coin_creator_vault_authority: accounts.get(18).copied().unwrap_or_default(),
        track_volume,
        ..Default::default()
    }))
}

/// 解析卖出指令事件
fn parse_sell_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpSwapSell;

    if data.len() < 16 || accounts.len() < 13 {
        return None;
    }

    let base_amount_in = read_u64_le(data, 0)?;
    let min_quote_amount_out = read_u64_le(data, 8)?;

    Some(TxEvent::PumpSwapSellEvent(PumpSwapSellEvent {
        metadata,
        base_amount_in,
        min_quote_amount_out,
        pool: *accounts.get(0)?,
        user: *accounts.get(1)?,
        base_mint: *accounts.get(3)?,
        quote_mint: *accounts.get(4)?,
        user_base_token_account: *accounts.get(5)?,
        user_quote_token_account: *accounts.get(6)?,
        pool_base_token_account: *accounts.get(7)?,
        pool_quote_token_account: *accounts.get(8)?,
        protocol_fee_recipient: *accounts.get(9)?,
        protocol_fee_recipient_token_account: *accounts.get(10)?,
        base_token_program: *accounts.get(11)?,
        quote_token_program: *accounts.get(12)?,
        coin_creator_vault_ata: accounts.get(17).copied().unwrap_or_default(),
        coin_creator_vault_authority: accounts.get(18).copied().unwrap_or_default(),
        ..Default::default()
    }))
}

/// 解析创建池子指令事件
fn parse_create_pool_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpSwapCreatePool;

    if data.len() < 18 || accounts.len() < 11 {
        return None;
    }

    let index = u16::from_le_bytes(data[0..2].try_into().ok()?);
    let base_amount_in = u64::from_le_bytes(data[2..10].try_into().ok()?);
    let quote_amount_in = u64::from_le_bytes(data[10..18].try_into().ok()?);
    let coin_creator = if data.len() >= 50 {
        Pubkey::new_from_array(data[18..50].try_into().ok()?)
    } else {
        Pubkey::default()
    };
    let is_mayhem_mode = data.get(50).copied().unwrap_or_default() != 0;
    let is_cashback_coin = data.get(51).copied().unwrap_or_default() != 0;

    Some(TxEvent::PumpSwapCreatePoolEvent(PumpSwapCreatePoolEvent {
        metadata,
        index,
        base_amount_in,
        quote_amount_in,
        pool: *accounts.get(0)?,
        creator: *accounts.get(2)?,
        base_mint: *accounts.get(3)?,
        quote_mint: *accounts.get(4)?,
        lp_mint: *accounts.get(5)?,
        user_base_token_account: *accounts.get(6)?,
        user_quote_token_account: *accounts.get(7)?,
        user_pool_token_account: *accounts.get(8)?,
        pool_base_token_account: *accounts.get(9)?,
        pool_quote_token_account: *accounts.get(10)?,
        coin_creator,
        is_mayhem_mode,
        is_cashback_coin,
        ..Default::default()
    }))
}

fn parse_init_boost_instruction(
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpSwapInitBoost;
    Some(TxEvent::PumpSwapInitBoostEvent(PumpSwapInitBoostEvent {
        metadata,
        pool: *accounts.first()?,
        mint: accounts.get(3).copied().unwrap_or_default(),
        ..Default::default()
    }))
}

/// 解析存款指令事件
fn parse_deposit_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpSwapDeposit;

    if data.len() < 24 || accounts.len() < 11 {
        return None;
    }

    let lp_token_amount_out = u64::from_le_bytes(data[0..8].try_into().ok()?);
    let max_base_amount_in = u64::from_le_bytes(data[8..16].try_into().ok()?);
    let max_quote_amount_in = u64::from_le_bytes(data[16..24].try_into().ok()?);

    Some(TxEvent::PumpSwapDepositEvent(PumpSwapDepositEvent {
        metadata,
        lp_token_amount_out,
        max_base_amount_in,
        max_quote_amount_in,
        pool: *accounts.get(0)?,
        user: *accounts.get(2)?,
        base_mint: *accounts.get(3)?,
        quote_mint: *accounts.get(4)?,
        user_base_token_account: *accounts.get(6)?,
        user_quote_token_account: *accounts.get(7)?,
        user_pool_token_account: *accounts.get(8)?,
        pool_base_token_account: *accounts.get(9)?,
        pool_quote_token_account: *accounts.get(10)?,
        ..Default::default()
    }))
}

/// 解析提款指令事件
fn parse_withdraw_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpSwapWithdraw;

    if data.len() < 24 || accounts.len() < 11 {
        return None;
    }

    let lp_token_amount_in = u64::from_le_bytes(data[0..8].try_into().ok()?);
    let min_base_amount_out = u64::from_le_bytes(data[8..16].try_into().ok()?);
    let min_quote_amount_out = u64::from_le_bytes(data[16..24].try_into().ok()?);

    Some(TxEvent::PumpSwapWithdrawEvent(PumpSwapWithdrawEvent {
        metadata,
        lp_token_amount_in,
        min_base_amount_out,
        min_quote_amount_out,
        pool: *accounts.get(0)?,
        user: *accounts.get(2)?,
        base_mint: *accounts.get(3)?,
        quote_mint: *accounts.get(4)?,
        user_base_token_account: *accounts.get(6)?,
        user_quote_token_account: *accounts.get(7)?,
        user_pool_token_account: *accounts.get(8)?,
        pool_base_token_account: *accounts.get(9)?,
        pool_quote_token_account: *accounts.get(10)?,
        ..Default::default()
    }))
}

/// Classify before allocating or decoding a protocol instruction.
pub(crate) fn instruction_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::BUY_IX => Some(EventType::PumpSwapBuy),
        discriminators::BUY_EXACT_QUOTE_IN_IX => Some(EventType::PumpSwapBuyExactQuoteIn),
        discriminators::SELL_IX => Some(EventType::PumpSwapSell),
        discriminators::CREATE_POOL_IX => Some(EventType::PumpSwapCreatePool),
        discriminators::INIT_BOOST_IX => Some(EventType::PumpSwapInitBoost),
        discriminators::DEPOSIT_IX => Some(EventType::PumpSwapDeposit),
        discriminators::WITHDRAW_IX => Some(EventType::PumpSwapWithdraw),
        _ => None,
    }
}

/// Classify before allocating or decoding a protocol account.
pub(crate) fn account_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::GLOBAL_CONFIG_ACCOUNT => Some(EventType::AccountPumpSwapGlobalConfig),
        discriminators::POOL_ACCOUNT => Some(EventType::AccountPumpSwapPool),
        _ => None,
    }
}
