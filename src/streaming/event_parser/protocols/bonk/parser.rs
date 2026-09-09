use crate::streaming::event_parser::InstructionAccounts;
use solana_sdk::pubkey::Pubkey;

use crate::streaming::event_parser::{
    common::{utils::*, EventMetadata, EventType},
    protocols::bonk::{
        bonk_pool_create_event_log_decode, bonk_trade_event_log_decode, discriminators, AmmFeeOn,
        BonkMigrateToAmmEvent, BonkMigrateToCpswapEvent, BonkPoolCreateEvent, BonkTradeEvent,
        ConstantCurve, CurveParams, FixedCurve, LinearCurve, MintParams, TradeDirection,
        VestingParams,
    },
    TxEvent,
};

/// Bonk Program ID
pub const BONK_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("LanMV9sAd7wArD4vJFi2qDdfnVhFxYSUg6eADduJ3uj");

/// 解析 Bonk instruction data
///
/// 根据判别器路由到具体的 instruction 解析函数
pub fn parse_bonk_instruction_data(
    discriminator: &[u8],
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    metadata: EventMetadata,
) -> Option<TxEvent> {
    match discriminator {
        discriminators::BUY_EXACT_IN => parse_buy_exact_in_instruction(data, accounts, metadata),
        discriminators::BUY_EXACT_OUT => parse_buy_exact_out_instruction(data, accounts, metadata),
        discriminators::SELL_EXACT_IN => parse_sell_exact_in_instruction(data, accounts, metadata),
        discriminators::SELL_EXACT_OUT => {
            parse_sell_exact_out_instruction(data, accounts, metadata)
        }
        discriminators::INITIALIZE => parse_initialize_instruction(data, accounts, metadata),
        discriminators::INITIALIZE_V2 => parse_initialize_v2_instruction(data, accounts, metadata),
        discriminators::INITIALIZE_WITH_TOKEN_2022 => {
            parse_initialize_with_token_2022_instruction(data, accounts, metadata)
        }
        discriminators::MIGRATE_TO_AMM => {
            parse_migrate_to_amm_instruction(data, accounts, metadata)
        }
        discriminators::MIGRATE_TO_CP_SWAP => {
            parse_migrate_to_cpswap_instruction(data, accounts, metadata)
        }
        _ => None,
    }
}

/// 解析 Bonk inner instruction data
///
/// 根据判别器路由到具体的 inner instruction 解析函数
pub fn parse_bonk_inner_instruction_data(
    discriminator: &[u8],
    data: &[u8],
    metadata: EventMetadata,
) -> Option<TxEvent> {
    match discriminator {
        discriminators::TRADE_EVENT => parse_trade_inner_instruction(data, metadata),
        discriminators::POOL_CREATE_EVENT => parse_pool_create_inner_instruction(data, metadata),
        _ => None,
    }
}

/// 解析 Bonk 账户数据
///
/// 根据判别器路由到具体的账户解析函数
pub fn parse_bonk_account_data(
    discriminator: &[u8],
    account: crate::streaming::grpc::AccountFrame,
    metadata: crate::streaming::event_parser::common::EventMetadata,
) -> Option<crate::streaming::event_parser::AccountEvent> {
    match discriminator {
        discriminators::POOL_STATE_ACCOUNT => {
            crate::streaming::event_parser::protocols::bonk::types::pool_state_parser(
                account, metadata,
            )
        }
        discriminators::GLOBAL_CONFIG_ACCOUNT => {
            crate::streaming::event_parser::protocols::bonk::types::global_config_parser(
                account, metadata,
            )
        }
        discriminators::PLATFORM_CONFIG_ACCOUNT => {
            crate::streaming::event_parser::protocols::bonk::types::platform_config_parser(
                account, metadata,
            )
        }
        _ => None,
    }
}

/// Parse pool creation event
fn parse_pool_create_inner_instruction(data: &[u8], metadata: EventMetadata) -> Option<TxEvent> {
    // Note: event_type will be set by the instruction parser, not here
    // Because different initialize instructions have different event types
    if let Some(event) = bonk_pool_create_event_log_decode(data) {
        Some(TxEvent::BonkPoolCreateEvent(BonkPoolCreateEvent { metadata, ..event }))
    } else {
        None
    }
}

/// Parse trade event
fn parse_trade_inner_instruction(data: &[u8], metadata: EventMetadata) -> Option<TxEvent> {
    if let Some(event) = bonk_trade_event_log_decode(data) {
        if metadata.event_type == EventType::BonkBuyExactIn
            || metadata.event_type == EventType::BonkBuyExactOut
        {
            if event.trade_direction != TradeDirection::Buy {
                return None;
            }
        } else if (metadata.event_type == EventType::BonkSellExactIn
            || metadata.event_type == EventType::BonkSellExactOut)
            && event.trade_direction != TradeDirection::Sell
        {
            return None;
        }
        Some(TxEvent::BonkTradeEvent(BonkTradeEvent { metadata, ..event }))
    } else {
        None
    }
}

/// Parse buy instruction event
fn parse_buy_exact_in_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::BonkBuyExactIn;

    if data.len() < 16 || accounts.len() < 18 {
        return None;
    }

    let amount_in = read_u64_le(data, 0)?;
    let minimum_amount_out = read_u64_le(data, 8)?;
    let share_fee_rate = read_u64_le(data, 16)?;

    Some(TxEvent::BonkTradeEvent(BonkTradeEvent {
        metadata,
        amount_in,
        minimum_amount_out,
        share_fee_rate,
        payer: *accounts.get(0)?,
        global_config: *accounts.get(2)?,
        platform_config: *accounts.get(3)?,
        pool_state: *accounts.get(4)?,
        user_base_token: *accounts.get(5)?,
        user_quote_token: *accounts.get(6)?,
        base_vault: *accounts.get(7)?,
        quote_vault: *accounts.get(8)?,
        base_token_mint: *accounts.get(9)?,
        quote_token_mint: *accounts.get(10)?,
        base_token_program: *accounts.get(11)?,
        quote_token_program: *accounts.get(12)?,
        system_program: *accounts.get(15)?,
        platform_associated_account: *accounts.get(16)?,
        creator_associated_account: *accounts.get(17)?,
        trade_direction: TradeDirection::Buy,
        ..Default::default()
    }))
}

fn parse_buy_exact_out_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::BonkBuyExactOut;

    if data.len() < 16 || accounts.len() < 18 {
        return None;
    }

    let amount_out = read_u64_le(data, 0)?;
    let maximum_amount_in = read_u64_le(data, 8)?;
    let share_fee_rate = read_u64_le(data, 16)?;

    Some(TxEvent::BonkTradeEvent(BonkTradeEvent {
        metadata,
        amount_out,
        maximum_amount_in,
        share_fee_rate,
        payer: *accounts.get(0)?,
        global_config: *accounts.get(2)?,
        platform_config: *accounts.get(3)?,
        pool_state: *accounts.get(4)?,
        user_base_token: *accounts.get(5)?,
        user_quote_token: *accounts.get(6)?,
        base_vault: *accounts.get(7)?,
        quote_vault: *accounts.get(8)?,
        base_token_mint: *accounts.get(9)?,
        quote_token_mint: *accounts.get(10)?,
        base_token_program: *accounts.get(11)?,
        quote_token_program: *accounts.get(12)?,
        system_program: *accounts.get(15)?,
        platform_associated_account: *accounts.get(16)?,
        creator_associated_account: *accounts.get(17)?,
        trade_direction: TradeDirection::Buy,
        ..Default::default()
    }))
}

fn parse_sell_exact_in_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::BonkSellExactIn;

    if data.len() < 16 || accounts.len() < 18 {
        return None;
    }

    let amount_in = read_u64_le(data, 0)?;
    let minimum_amount_out = read_u64_le(data, 8)?;
    let share_fee_rate = read_u64_le(data, 16)?;

    Some(TxEvent::BonkTradeEvent(BonkTradeEvent {
        metadata,
        amount_in,
        minimum_amount_out,
        share_fee_rate,
        payer: *accounts.get(0)?,
        global_config: *accounts.get(2)?,
        platform_config: *accounts.get(3)?,
        pool_state: *accounts.get(4)?,
        user_base_token: *accounts.get(5)?,
        user_quote_token: *accounts.get(6)?,
        base_vault: *accounts.get(7)?,
        quote_vault: *accounts.get(8)?,
        base_token_mint: *accounts.get(9)?,
        quote_token_mint: *accounts.get(10)?,
        base_token_program: *accounts.get(11)?,
        quote_token_program: *accounts.get(12)?,
        system_program: *accounts.get(15)?,
        platform_associated_account: *accounts.get(16)?,
        creator_associated_account: *accounts.get(17)?,
        trade_direction: TradeDirection::Sell,
        ..Default::default()
    }))
}

fn parse_sell_exact_out_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::BonkSellExactOut;

    if data.len() < 16 || accounts.len() < 18 {
        return None;
    }

    let amount_out = read_u64_le(data, 0)?;
    let maximum_amount_in = read_u64_le(data, 8)?;
    let share_fee_rate = read_u64_le(data, 16)?;

    Some(TxEvent::BonkTradeEvent(BonkTradeEvent {
        metadata,
        amount_out,
        maximum_amount_in,
        share_fee_rate,
        payer: *accounts.get(0)?,
        global_config: *accounts.get(2)?,
        platform_config: *accounts.get(3)?,
        pool_state: *accounts.get(4)?,
        user_base_token: *accounts.get(5)?,
        user_quote_token: *accounts.get(6)?,
        base_vault: *accounts.get(7)?,
        quote_vault: *accounts.get(8)?,
        base_token_mint: *accounts.get(9)?,
        quote_token_mint: *accounts.get(10)?,
        base_token_program: *accounts.get(11)?,
        quote_token_program: *accounts.get(12)?,
        system_program: *accounts.get(15)?,
        platform_associated_account: *accounts.get(16)?,
        creator_associated_account: *accounts.get(17)?,
        trade_direction: TradeDirection::Sell,
        ..Default::default()
    }))
}

/// Parse initialize event
fn parse_initialize_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::BonkInitialize;

    if data.len() < 24 {
        return None;
    }

    let mut offset = 0;
    let base_mint_param = parse_mint_params(data, &mut offset)?;
    let curve_param = parse_curve_params(data, &mut offset)?;
    let vesting_param = parse_vesting_params(data, &mut offset)?;

    Some(TxEvent::BonkPoolCreateEvent(BonkPoolCreateEvent {
        metadata,
        payer: *accounts.get(0)?,
        creator: *accounts.get(1)?,
        global_config: *accounts.get(2)?,
        platform_config: *accounts.get(3)?,
        pool_state: *accounts.get(5)?,
        base_mint: *accounts.get(6)?,
        quote_mint: *accounts.get(7)?,
        base_vault: *accounts.get(8)?,
        quote_vault: *accounts.get(9)?,
        base_mint_param,
        curve_param,
        vesting_param,
        ..Default::default()
    }))
}

/// Parse initialize event
fn parse_initialize_v2_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::BonkInitializeV2;

    if data.len() < 24 {
        return None;
    }

    let mut offset = 0;
    let base_mint_param = parse_mint_params(data, &mut offset)?;
    let curve_param = parse_curve_params(data, &mut offset)?;
    let vesting_param = parse_vesting_params(data, &mut offset)?;
    let amm_fee_on = data[offset];

    Some(TxEvent::BonkPoolCreateEvent(BonkPoolCreateEvent {
        metadata,
        payer: *accounts.get(0)?,
        creator: *accounts.get(1)?,
        global_config: *accounts.get(2)?,
        platform_config: *accounts.get(3)?,
        pool_state: *accounts.get(5)?,
        base_mint: *accounts.get(6)?,
        quote_mint: *accounts.get(7)?,
        base_vault: *accounts.get(8)?,
        quote_vault: *accounts.get(9)?,
        base_mint_param,
        curve_param,
        vesting_param,
        amm_fee_on: if amm_fee_on == 0 {
            Some(AmmFeeOn::QuoteToken)
        } else {
            Some(AmmFeeOn::BothToken)
        },
        ..Default::default()
    }))
}

/// Parse initialize event
fn parse_initialize_with_token_2022_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::BonkInitializeWithToken2022;

    if data.len() < 24 {
        return None;
    }

    let mut offset = 0;
    let base_mint_param = parse_mint_params(data, &mut offset)?;
    let curve_param = parse_curve_params(data, &mut offset)?;
    let vesting_param = parse_vesting_params(data, &mut offset)?;
    let amm_fee_on = data[offset];

    Some(TxEvent::BonkPoolCreateEvent(BonkPoolCreateEvent {
        metadata,
        payer: *accounts.get(0)?,
        creator: *accounts.get(1)?,
        global_config: *accounts.get(2)?,
        platform_config: *accounts.get(3)?,
        pool_state: *accounts.get(5)?,
        base_mint: *accounts.get(6)?,
        quote_mint: *accounts.get(7)?,
        base_vault: *accounts.get(8)?,
        quote_vault: *accounts.get(9)?,
        base_mint_param,
        curve_param,
        vesting_param,
        amm_fee_on: if amm_fee_on == 0 {
            Some(AmmFeeOn::QuoteToken)
        } else {
            Some(AmmFeeOn::BothToken)
        },
        ..Default::default()
    }))
}

/// Parse MintParams structure
fn parse_mint_params(data: &[u8], offset: &mut usize) -> Option<MintParams> {
    // Read decimals (1 byte)
    let decimals = read_u8(data, *offset)?;
    *offset += 1;

    // Read name string length and content
    let name_len = read_u32_le(data, *offset)? as usize;
    *offset += 4;
    if data.len() < *offset + name_len {
        return None;
    }
    let name = String::from_utf8(data[*offset..*offset + name_len].to_vec()).ok()?;
    *offset += name_len;

    // Read symbol string length and content
    let symbol_len = read_u32_le(data, *offset)? as usize;
    *offset += 4;
    if data.len() < *offset + symbol_len {
        return None;
    }
    let symbol = String::from_utf8(data[*offset..*offset + symbol_len].to_vec()).ok()?;
    *offset += symbol_len;

    // Read uri string length and content
    let uri_len = read_u32_le(data, *offset)? as usize;
    *offset += 4;
    if data.len() < *offset + uri_len {
        return None;
    }
    let uri = String::from_utf8(data[*offset..*offset + uri_len].to_vec()).ok()?;
    *offset += uri_len;

    Some(MintParams { decimals, name, symbol, uri })
}

/// Parse CurveParams structure
fn parse_curve_params(data: &[u8], offset: &mut usize) -> Option<CurveParams> {
    // Read curve type identifier (1 byte)
    let curve_type = read_u8(data, *offset)?;
    *offset += 1;

    match curve_type {
        0 => {
            // Constant curve
            let supply = read_u64_le(data, *offset)?;
            *offset += 8;
            let total_base_sell = read_u64_le(data, *offset)?;
            *offset += 8;
            let total_quote_fund_raising = read_u64_le(data, *offset)?;
            *offset += 8;
            let migrate_type = read_u8(data, *offset)?;
            *offset += 1;

            Some(CurveParams::Constant {
                data: ConstantCurve {
                    supply,
                    total_base_sell,
                    total_quote_fund_raising,
                    migrate_type,
                },
            })
        }
        1 => {
            // Fixed curve
            let supply = read_u64_le(data, *offset)?;
            *offset += 8;
            let total_quote_fund_raising = read_u64_le(data, *offset)?;
            *offset += 8;
            let migrate_type = read_u8(data, *offset)?;
            *offset += 1;

            Some(CurveParams::Fixed {
                data: FixedCurve { supply, total_quote_fund_raising, migrate_type },
            })
        }
        2 => {
            // Linear curve
            let supply = read_u64_le(data, *offset)?;
            *offset += 8;
            let total_quote_fund_raising = read_u64_le(data, *offset)?;
            *offset += 8;
            let migrate_type = read_u8(data, *offset)?;
            *offset += 1;

            Some(CurveParams::Linear {
                data: LinearCurve { supply, total_quote_fund_raising, migrate_type },
            })
        }
        _ => None,
    }
}

/// Parse VestingParams structure
fn parse_vesting_params(data: &[u8], offset: &mut usize) -> Option<VestingParams> {
    let total_locked_amount = read_u64_le(data, *offset)?;
    *offset += 8;
    let cliff_period = read_u64_le(data, *offset)?;
    *offset += 8;
    let unlock_period = read_u64_le(data, *offset)?;
    *offset += 8;

    Some(VestingParams { total_locked_amount, cliff_period, unlock_period })
}

/// Parse migrate to AMM event
fn parse_migrate_to_amm_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::BonkMigrateToAmm;

    if data.len() < 17 {
        return None;
    }

    let base_lot_size = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let quote_lot_size = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let market_vault_signer_nonce = data[16];

    Some(TxEvent::BonkMigrateToAmmEvent(BonkMigrateToAmmEvent {
        metadata,
        base_lot_size,
        quote_lot_size,
        market_vault_signer_nonce,
        payer: *accounts.get(0)?,
        base_mint: *accounts.get(1)?,
        quote_mint: *accounts.get(2)?,
        openbook_program: *accounts.get(3)?,
        market: *accounts.get(4)?,
        request_queue: *accounts.get(5)?,
        event_queue: *accounts.get(6)?,
        bids: *accounts.get(7)?,
        asks: *accounts.get(8)?,
        market_vault_signer: *accounts.get(9)?,
        market_base_vault: *accounts.get(10)?,
        market_quote_vault: *accounts.get(11)?,
        amm_program: *accounts.get(12)?,
        amm_pool: *accounts.get(13)?,
        amm_authority: *accounts.get(14)?,
        amm_open_orders: *accounts.get(15)?,
        amm_lp_mint: *accounts.get(16)?,
        amm_base_vault: *accounts.get(17)?,
        amm_quote_vault: *accounts.get(18)?,
        amm_target_orders: *accounts.get(19)?,
        amm_config: *accounts.get(20)?,
        amm_create_fee_destination: *accounts.get(21)?,
        authority: *accounts.get(22)?,
        pool_state: *accounts.get(23)?,
        global_config: *accounts.get(24)?,
        base_vault: *accounts.get(25)?,
        quote_vault: *accounts.get(26)?,
        pool_lp_token: *accounts.get(27)?,
        spl_token_program: *accounts.get(28)?,
        associated_token_program: *accounts.get(29)?,
        system_program: *accounts.get(30)?,
        rent_program: *accounts.get(31)?,
        ..Default::default()
    }))
}

/// Parse migrate to CP Swap event
fn parse_migrate_to_cpswap_instruction(
    _data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::BonkMigrateToCpswap;

    Some(TxEvent::BonkMigrateToCpswapEvent(BonkMigrateToCpswapEvent {
        metadata,
        payer: *accounts.get(0)?,
        base_mint: *accounts.get(1)?,
        quote_mint: *accounts.get(2)?,
        platform_config: *accounts.get(3)?,
        cpswap_program: *accounts.get(4)?,
        cpswap_pool: *accounts.get(5)?,
        cpswap_authority: *accounts.get(6)?,
        cpswap_lp_mint: *accounts.get(7)?,
        cpswap_base_vault: *accounts.get(8)?,
        cpswap_quote_vault: *accounts.get(9)?,
        cpswap_config: *accounts.get(10)?,
        cpswap_create_pool_fee: *accounts.get(11)?,
        cpswap_observation: *accounts.get(12)?,
        lock_program: *accounts.get(13)?,
        lock_authority: *accounts.get(14)?,
        lock_lp_vault: *accounts.get(15)?,
        authority: *accounts.get(16)?,
        pool_state: *accounts.get(17)?,
        global_config: *accounts.get(18)?,
        base_vault: *accounts.get(19)?,
        quote_vault: *accounts.get(20)?,
        pool_lp_token: *accounts.get(21)?,
        base_token_program: *accounts.get(22)?,
        quote_token_program: *accounts.get(23)?,
        associated_token_program: *accounts.get(24)?,
        system_program: *accounts.get(25)?,
        rent_program: *accounts.get(26)?,
        metadata_program: *accounts.get(27)?,
        remaining_account_indices: accounts.indices_from(28).collect(),
        ..Default::default()
    }))
}

/// Classify before allocating or decoding a protocol instruction.
pub(crate) fn instruction_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::BUY_EXACT_IN => Some(EventType::BonkBuyExactIn),
        discriminators::BUY_EXACT_OUT => Some(EventType::BonkBuyExactOut),
        discriminators::SELL_EXACT_IN => Some(EventType::BonkSellExactIn),
        discriminators::SELL_EXACT_OUT => Some(EventType::BonkSellExactOut),
        discriminators::INITIALIZE => Some(EventType::BonkInitialize),
        discriminators::INITIALIZE_V2 => Some(EventType::BonkInitializeV2),
        discriminators::INITIALIZE_WITH_TOKEN_2022 => Some(EventType::BonkInitializeWithToken2022),
        discriminators::MIGRATE_TO_AMM => Some(EventType::BonkMigrateToAmm),
        discriminators::MIGRATE_TO_CP_SWAP => Some(EventType::BonkMigrateToCpswap),
        _ => None,
    }
}

/// Classify before allocating or decoding a protocol account.
pub(crate) fn account_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::POOL_STATE_ACCOUNT => Some(EventType::AccountBonkPoolState),
        discriminators::GLOBAL_CONFIG_ACCOUNT => Some(EventType::AccountBonkGlobalConfig),
        discriminators::PLATFORM_CONFIG_ACCOUNT => Some(EventType::AccountBonkPlatformConfig),
        _ => None,
    }
}
