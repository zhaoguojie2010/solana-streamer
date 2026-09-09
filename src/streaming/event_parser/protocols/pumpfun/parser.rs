use crate::streaming::event_parser::InstructionAccounts;
use crate::streaming::event_parser::{
    common::{EventMetadata, EventType},
    protocols::pumpfun::{
        discriminators, pumpfun_create_v2_token_event_log_decode, pumpfun_migrate_event_log_decode,
        pumpfun_trade_event_log_decode, PumpFunCreateTokenEvent, PumpFunCreateV2TokenEvent,
        PumpFunMigrateEvent, PumpFunTradeEvent,
    },
    TxEvent,
};
use solana_sdk::pubkey::Pubkey;

/// PumpFun程序ID
pub const PUMPFUN_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");

/// 解析 PumpFun instruction data
///
/// 根据判别器路由到具体的 instruction 解析函数
pub fn parse_pumpfun_instruction_data(
    discriminator: &[u8],
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    metadata: EventMetadata,
) -> Option<TxEvent> {
    match discriminator {
        discriminators::CREATE_TOKEN_IX => parse_create_token_instruction(data, accounts, metadata),
        discriminators::CREATE_V2_TOKEN_IX => {
            parse_create_v2_token_instruction(data, accounts, metadata)
        }
        discriminators::BUY_IX => parse_buy_instruction(data, accounts, metadata),
        discriminators::SELL_IX => parse_sell_instruction(data, accounts, metadata),
        discriminators::MIGRATE_IX => parse_migrate_instruction(data, accounts, metadata),
        _ => None,
    }
}

/// 解析 PumpFun inner instruction data
///
/// 根据判别器路由到具体的 inner instruction 解析函数
pub fn parse_pumpfun_inner_instruction_data(
    discriminator: &[u8],
    data: &[u8],
    metadata: EventMetadata,
) -> Option<TxEvent> {
    match discriminator {
        discriminators::CREATE_TOKEN_EVENT => parse_create_token_inner_instruction(data, metadata),
        discriminators::TRADE_EVENT => parse_trade_inner_instruction(data, metadata),
        discriminators::COMPLETE_PUMP_AMM_MIGRATION_EVENT => {
            parse_migrate_inner_instruction(data, metadata)
        }
        _ => None,
    }
}

/// 解析 PumpFun 账户数据
///
/// 根据判别器路由到具体的账户解析函数
pub fn parse_pumpfun_account_data(
    discriminator: &[u8],
    account: crate::streaming::grpc::AccountFrame,
    metadata: crate::streaming::event_parser::common::EventMetadata,
) -> Option<crate::streaming::event_parser::AccountEvent> {
    match discriminator {
        discriminators::BONDING_CURVE_ACCOUNT => {
            crate::streaming::event_parser::protocols::pumpfun::types::bonding_curve_parser(
                account, metadata,
            )
        }
        discriminators::GLOBAL_ACCOUNT => {
            crate::streaming::event_parser::protocols::pumpfun::types::global_parser(
                account, metadata,
            )
        }
        _ => None,
    }
}

/// 解析迁移事件
fn parse_migrate_inner_instruction(data: &[u8], mut metadata: EventMetadata) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpFunMigrate;
    if let Some(event) = pumpfun_migrate_event_log_decode(data) {
        Some(TxEvent::PumpFunMigrateEvent(PumpFunMigrateEvent { metadata, ..event }))
    } else {
        None
    }
}

/// 解析创建代币日志事件
fn parse_create_token_inner_instruction(
    data: &[u8],
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpFunCreateToken;
    if let Some(event) = pumpfun_create_v2_token_event_log_decode(data) {
        Some(TxEvent::PumpFunCreateV2TokenEvent(PumpFunCreateV2TokenEvent { metadata, ..event }))
    } else {
        None
    }
}

/// 解析交易事件 (inner instruction 不设置 event_type，因为不知道是 Buy 还是 Sell)
fn parse_trade_inner_instruction(data: &[u8], metadata: EventMetadata) -> Option<TxEvent> {
    // 注意：inner instruction 的 trade event 不设置 event_type
    // 因为它会被合并到 instruction event 中，而 instruction event 已经设置了正确的 event_type
    if let Some(event) = pumpfun_trade_event_log_decode(data) {
        Some(TxEvent::PumpFunTradeEvent(PumpFunTradeEvent { metadata, ..event }))
    } else {
        None
    }
}

/// 解析创建代币指令事件
fn parse_create_token_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpFunCreateToken;

    if data.len() < 16 || accounts.len() < 11 {
        return None;
    }
    let mut offset = 0;
    if offset + 4 > data.len() {
        return None;
    }
    let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    if offset + name_len > data.len() {
        return None;
    }
    let name = String::from_utf8_lossy(&data[offset..offset + name_len]);
    offset += name_len;
    if offset + 4 > data.len() {
        return None;
    }
    let symbol_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    if offset + symbol_len > data.len() {
        return None;
    }
    let symbol = String::from_utf8_lossy(&data[offset..offset + symbol_len]);
    offset += symbol_len;
    if offset + 4 > data.len() {
        return None;
    }
    let uri_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    if offset + uri_len > data.len() {
        return None;
    }
    let uri = String::from_utf8_lossy(&data[offset..offset + uri_len]);
    offset += uri_len;
    let creator = if offset + 32 <= data.len() {
        Pubkey::new_from_array(data[offset..offset + 32].try_into().ok()?)
    } else {
        Pubkey::default()
    };

    Some(TxEvent::PumpFunCreateTokenEvent(PumpFunCreateTokenEvent {
        metadata,
        name: name.to_string(),
        symbol: symbol.to_string(),
        uri: uri.to_string(),
        creator,
        mint: *accounts.get(0)?,
        mint_authority: *accounts.get(1)?,
        bonding_curve: *accounts.get(2)?,
        associated_bonding_curve: *accounts.get(3)?,
        user: *accounts.get(7)?,
        ..Default::default()
    }))
}

/// 解析创建 V2 代币指令事件 (SPL-22 Token, Mayhem Mode)
fn parse_create_v2_token_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpFunCreateV2Token;

    if data.len() < 16 || accounts.len() < 11 {
        return None;
    }
    let mut offset = 0;
    if offset + 4 > data.len() {
        return None;
    }
    let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    if offset + name_len > data.len() {
        return None;
    }
    let name = String::from_utf8_lossy(&data[offset..offset + name_len]);
    offset += name_len;
    if offset + 4 > data.len() {
        return None;
    }
    let symbol_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    if offset + symbol_len > data.len() {
        return None;
    }
    let symbol = String::from_utf8_lossy(&data[offset..offset + symbol_len]);
    offset += symbol_len;
    if offset + 4 > data.len() {
        return None;
    }
    let uri_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    if offset + uri_len > data.len() {
        return None;
    }
    let uri = String::from_utf8_lossy(&data[offset..offset + uri_len]);
    offset += uri_len;
    let creator = if offset + 32 <= data.len() {
        Pubkey::new_from_array(data[offset..offset + 32].try_into().ok()?)
    } else {
        Pubkey::default()
    };

    Some(TxEvent::PumpFunCreateV2TokenEvent(PumpFunCreateV2TokenEvent {
        metadata,
        name: name.to_string(),
        symbol: symbol.to_string(),
        uri: uri.to_string(),
        creator,
        mint: *accounts.get(0)?,
        mint_authority: *accounts.get(1)?,
        bonding_curve: *accounts.get(2)?,
        associated_bonding_curve: *accounts.get(3)?,
        user: *accounts.get(7)?,
        ..Default::default()
    }))
}

// 解析买入指令事件
fn parse_buy_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpFunBuy;

    if data.len() < 16 || accounts.len() < 16 {
        return None;
    }
    let amount = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let max_sol_cost = u64::from_le_bytes(data[8..16].try_into().unwrap());
    Some(TxEvent::PumpFunTradeEvent(PumpFunTradeEvent {
        metadata,
        global: *accounts.get(0)?,
        fee_recipient: *accounts.get(1)?,
        mint: *accounts.get(2)?,
        bonding_curve: *accounts.get(3)?,
        associated_bonding_curve: *accounts.get(4)?,
        associated_user: *accounts.get(5)?,
        user: *accounts.get(6)?,
        system_program: *accounts.get(7)?,
        token_program: *accounts.get(8)?,
        creator_vault: *accounts.get(9)?,
        event_authority: *accounts.get(10)?,
        program: *accounts.get(11)?,
        global_volume_accumulator: *accounts.get(12)?,
        user_volume_accumulator: *accounts.get(13)?,
        fee_config: *accounts.get(14)?,
        fee_program: *accounts.get(15)?,
        max_sol_cost,
        amount,
        is_buy: true,
        ..Default::default()
    }))
}

// 解析卖出指令事件
fn parse_sell_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpFunSell;

    if data.len() < 16 || accounts.len() < 14 {
        return None;
    }
    let amount = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let min_sol_output = u64::from_le_bytes(data[8..16].try_into().unwrap());
    Some(TxEvent::PumpFunTradeEvent(PumpFunTradeEvent {
        metadata,
        global: *accounts.get(0)?,
        fee_recipient: *accounts.get(1)?,
        mint: *accounts.get(2)?,
        bonding_curve: *accounts.get(3)?,
        associated_bonding_curve: *accounts.get(4)?,
        associated_user: *accounts.get(5)?,
        user: *accounts.get(6)?,
        system_program: *accounts.get(7)?,
        creator_vault: *accounts.get(8)?,
        token_program: *accounts.get(9)?,
        event_authority: *accounts.get(10)?,
        program: *accounts.get(11)?,
        global_volume_accumulator: Pubkey::default(),
        user_volume_accumulator: Pubkey::default(),
        fee_config: *accounts.get(12)?,
        fee_program: *accounts.get(13)?,
        min_sol_output,
        amount,
        is_buy: false,
        ..Default::default()
    }))
}

/// 解析迁移指令事件
fn parse_migrate_instruction(
    _data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::PumpFunMigrate;

    if accounts.len() < 24 {
        return None;
    }
    Some(TxEvent::PumpFunMigrateEvent(PumpFunMigrateEvent {
        metadata,
        global: *accounts.get(0)?,
        withdraw_authority: *accounts.get(1)?,
        mint: *accounts.get(2)?,
        bonding_curve: *accounts.get(3)?,
        associated_bonding_curve: *accounts.get(4)?,
        user: *accounts.get(5)?,
        system_program: *accounts.get(6)?,
        token_program: *accounts.get(7)?,
        pump_amm: *accounts.get(8)?,
        pool: *accounts.get(9)?,
        pool_authority: *accounts.get(10)?,
        pool_authority_mint_account: *accounts.get(11)?,
        pool_authority_wsol_account: *accounts.get(12)?,
        amm_global_config: *accounts.get(13)?,
        wsol_mint: *accounts.get(14)?,
        lp_mint: *accounts.get(15)?,
        user_pool_token_account: *accounts.get(16)?,
        pool_base_token_account: *accounts.get(17)?,
        pool_quote_token_account: *accounts.get(18)?,
        token_2022_program: *accounts.get(19)?,
        associated_token_program: *accounts.get(20)?,
        pump_amm_event_authority: *accounts.get(21)?,
        event_authority: *accounts.get(22)?,
        program: *accounts.get(23)?,
        ..Default::default()
    }))
}

/// Classify before allocating or decoding a protocol instruction.
pub(crate) fn instruction_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::CREATE_TOKEN_IX => Some(EventType::PumpFunCreateToken),
        discriminators::CREATE_V2_TOKEN_IX => Some(EventType::PumpFunCreateV2Token),
        discriminators::BUY_IX => Some(EventType::PumpFunBuy),
        discriminators::SELL_IX => Some(EventType::PumpFunSell),
        discriminators::MIGRATE_IX => Some(EventType::PumpFunMigrate),
        _ => None,
    }
}

/// Classify before allocating or decoding a protocol account.
pub(crate) fn account_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::BONDING_CURVE_ACCOUNT => Some(EventType::AccountPumpFunBondingCurve),
        discriminators::GLOBAL_ACCOUNT => Some(EventType::AccountPumpFunGlobal),
        _ => None,
    }
}
