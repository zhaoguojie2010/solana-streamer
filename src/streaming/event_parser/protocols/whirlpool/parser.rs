use crate::streaming::event_parser::{
    common::{read_u128_le, read_u64_le, read_u8_le, EventMetadata, EventType, ProgramDataItem},
    protocols::whirlpool::{discriminators, WhirlpoolSwapEvent, WhirlpoolSwapV2Event},
    DexEvent,
};
use solana_sdk::pubkey::Pubkey;

/// Whirlpool 程序ID
pub const WHIRLPOOL_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");

/// Traded 事件日志解析后的数据
#[derive(Clone, Debug, Default)]
pub struct TradedEventLogData {
    pub whirlpool: Pubkey,
    pub a_to_b: bool,
    pub pre_sqrt_price: u128,
    pub post_sqrt_price: u128,
    pub input_amount: u64,
    pub output_amount: u64,
    pub input_transfer_fee: u64,
    pub output_transfer_fee: u64,
    pub lp_fee: u64,
    pub protocol_fee: u64,
}

/// 解析 Whirlpool instruction data
pub fn parse_whirlpool_instruction_data(
    discriminator: &[u8],
    data: &[u8],
    accounts: &[Pubkey],
    metadata: EventMetadata,
) -> Option<DexEvent> {
    match discriminator {
        discriminators::SWAP => parse_swap_instruction(data, accounts, metadata),
        discriminators::SWAP_V2 => parse_swap_v2_instruction(data, accounts, metadata),
        _ => None,
    }
}

pub fn is_whirlpool_swap_instruction(discriminator: &[u8]) -> bool {
    matches!(discriminator, discriminators::SWAP | discriminators::SWAP_V2)
}

/// 解析 Whirlpool inner instruction data
///
/// Whirlpool 当前不通过 inner instruction 承载 Swap 事件
pub fn parse_whirlpool_inner_instruction_data(
    _discriminator: &[u8],
    _data: &[u8],
    _metadata: EventMetadata,
) -> Option<DexEvent> {
    None
}

/// 解析 Whirlpool 账户数据
///
/// 根据判别器路由到具体的账户解析函数
pub fn parse_whirlpool_account_data(
    discriminator: &[u8],
    account: crate::streaming::grpc::AccountPretty,
    metadata: crate::streaming::event_parser::common::EventMetadata,
) -> Option<crate::streaming::event_parser::DexEvent> {
    match discriminator {
        discriminators::WHIRLPOOL => {
            crate::streaming::event_parser::protocols::whirlpool::types::whirlpool_parser(
                account, metadata,
            )
        }
        discriminators::TICK_ARRAY => {
            crate::streaming::event_parser::protocols::whirlpool::types::whirlpool_tick_array_parser(
                account, metadata,
            )
        }
        _ => None,
    }
}

fn parse_swap_instruction(
    data: &[u8],
    accounts: &[Pubkey],
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::WhirlpoolSwap;

    if data.len() < 34 || accounts.len() < 11 {
        return None;
    }

    Some(DexEvent::WhirlpoolSwapEvent(WhirlpoolSwapEvent {
        metadata,
        amount: read_u64_le(data, 0)?,
        other_amount_threshold: read_u64_le(data, 8)?,
        sqrt_price_limit: read_u128_le(data, 16)?,
        amount_specified_is_input: read_u8_le(data, 32)? != 0,
        a_to_b: read_u8_le(data, 33)? != 0,
        token_program: accounts[0],
        token_authority: accounts[1],
        whirlpool: accounts[2],
        token_owner_account_a: accounts[3],
        token_vault_a: accounts[4],
        token_owner_account_b: accounts[5],
        token_vault_b: accounts[6],
        tick_array_0: accounts[7],
        tick_array_1: accounts[8],
        tick_array_2: accounts[9],
        oracle: accounts[10],
        remaining_accounts: accounts[11..].to_vec(),
        ..Default::default()
    }))
}

fn parse_swap_v2_instruction(
    data: &[u8],
    accounts: &[Pubkey],
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::WhirlpoolSwapV2;

    if data.len() < 34 || accounts.len() < 15 {
        return None;
    }

    Some(DexEvent::WhirlpoolSwapV2Event(WhirlpoolSwapV2Event {
        metadata,
        amount: read_u64_le(data, 0)?,
        other_amount_threshold: read_u64_le(data, 8)?,
        sqrt_price_limit: read_u128_le(data, 16)?,
        amount_specified_is_input: read_u8_le(data, 32)? != 0,
        a_to_b: read_u8_le(data, 33)? != 0,
        token_program_a: accounts[0],
        token_program_b: accounts[1],
        memo_program: accounts[2],
        token_authority: accounts[3],
        whirlpool: accounts[4],
        token_mint_a: accounts[5],
        token_mint_b: accounts[6],
        token_owner_account_a: accounts[7],
        token_vault_a: accounts[8],
        token_owner_account_b: accounts[9],
        token_vault_b: accounts[10],
        tick_array_0: accounts[11],
        tick_array_1: accounts[12],
        tick_array_2: accounts[13],
        oracle: accounts[14],
        remaining_accounts: accounts[15..].to_vec(),
        ..Default::default()
    }))
}

/// 从 Anchor Program data 日志解析 Traded 事件
///
/// 日志格式: "Program data: <base64>"
/// 编码格式: [8字节事件鉴别器][borsh(Traded)]
pub fn parse_traded_event_from_log(log_data_base64: &str) -> Option<TradedEventLogData> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let decoded = STANDARD.decode(log_data_base64).ok()?;
    if decoded.len() < 8 {
        return None;
    }
    if &decoded[0..8] != discriminators::TRADED_EVENT {
        return None;
    }

    let mut offset = 8;
    let whirlpool = Pubkey::new_from_array(decoded.get(offset..offset + 32)?.try_into().ok()?);
    offset += 32;
    let a_to_b = read_u8_le(&decoded, offset)? != 0;
    offset += 1;
    let pre_sqrt_price = read_u128_le(&decoded, offset)?;
    offset += 16;
    let post_sqrt_price = read_u128_le(&decoded, offset)?;
    offset += 16;
    let input_amount = read_u64_le(&decoded, offset)?;
    offset += 8;
    let output_amount = read_u64_le(&decoded, offset)?;
    offset += 8;
    let input_transfer_fee = read_u64_le(&decoded, offset)?;
    offset += 8;
    let output_transfer_fee = read_u64_le(&decoded, offset)?;
    offset += 8;
    let lp_fee = read_u64_le(&decoded, offset)?;
    offset += 8;
    let protocol_fee = read_u64_le(&decoded, offset)?;

    Some(TradedEventLogData {
        whirlpool,
        a_to_b,
        pre_sqrt_price,
        post_sqrt_price,
        input_amount,
        output_amount,
        input_transfer_fee,
        output_transfer_fee,
        lp_fee,
        protocol_fee,
    })
}

/// 从 ProgramDataItem 解析 Traded 事件
pub fn parse_traded_event_from_program_data(
    item: &ProgramDataItem,
    expected_whirlpool: &Pubkey,
) -> Option<TradedEventLogData> {
    if item.program_id != WHIRLPOOL_PROGRAM_ID {
        return None;
    }
    let event_data = parse_traded_event_from_log(&item.base64)?;
    if &event_data.whirlpool != expected_whirlpool {
        return None;
    }
    Some(event_data)
}
