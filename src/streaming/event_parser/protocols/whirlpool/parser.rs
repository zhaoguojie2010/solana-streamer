use crate::streaming::event_parser::InstructionAccounts;
use crate::streaming::event_parser::{
    common::{read_u128_le, read_u64_le, read_u8_le, EventMetadata, EventType, ProgramDataItem},
    protocols::whirlpool::{
        discriminators, WhirlpoolExecutionEvent, WhirlpoolInstructionEvent,
        WhirlpoolInstructionKind, WhirlpoolSwapEvent, WhirlpoolSwapV2Event,
    },
    TxEvent,
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
    accounts: InstructionAccounts<'_>,
    metadata: EventMetadata,
) -> Option<TxEvent> {
    match discriminator {
        discriminators::SWAP => parse_swap_instruction(data, accounts, metadata),
        discriminators::SWAP_V2 => parse_swap_v2_instruction(data, accounts, metadata),
        _ => parse_modeled_instruction(discriminator, data, accounts, metadata),
    }
}

fn parse_modeled_instruction(
    discriminator: &[u8],
    _data: &[u8],
    _accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    let disc: [u8; 8] = discriminator.try_into().ok()?;
    use WhirlpoolInstructionKind as K;
    let kind = match disc {
        [95, 180, 10, 172, 84, 174, 232, 40] => K::InitializePool,
        [207, 45, 87, 242, 27, 63, 204, 67] => K::InitializePoolV2,
        [143, 94, 96, 76, 172, 124, 119, 199] => K::InitializePoolWithAdaptiveFee,
        [11, 188, 193, 214, 141, 91, 149, 184] => K::InitializeTickArray,
        [41, 33, 165, 200, 120, 231, 142, 50] => K::InitializeDynamicTickArray,
        [135, 128, 47, 77, 15, 152, 240, 49] => K::OpenPosition,
        [242, 29, 134, 48, 58, 110, 14, 60] => K::OpenPositionWithMetadata,
        [212, 47, 95, 92, 114, 102, 131, 250] => K::OpenPositionWithTokenExtensions,
        [169, 113, 126, 171, 213, 172, 212, 49] => K::OpenBundledPosition,
        [46, 156, 243, 118, 13, 205, 251, 178] => K::IncreaseLiquidity,
        [133, 29, 89, 223, 69, 238, 176, 10] => K::IncreaseLiquidityV2,
        [239, 251, 9, 124, 210, 198, 53, 43] => K::IncreaseLiquidityByTokenAmountsV2,
        [160, 38, 208, 111, 104, 91, 44, 1] => K::DecreaseLiquidity,
        [58, 127, 188, 62, 79, 82, 196, 96] => K::DecreaseLiquidityV2,
        [191, 169, 224, 11, 131, 19, 158, 253] => K::RepositionLiquidityV2,
        [195, 96, 237, 108, 68, 162, 219, 230] => K::TwoHopSwap,
        [186, 143, 209, 29, 254, 2, 194, 117] => K::TwoHopSwapV2,
        [53, 243, 137, 65, 8, 140, 158, 6] => K::SetFeeRate,
        [95, 7, 4, 50, 154, 79, 156, 131] => K::SetProtocolFeeRate,
        [154, 230, 250, 13, 236, 209, 75, 223] => K::UpdateFeesAndRewards,
        [164, 152, 207, 99, 30, 186, 19, 182] => K::CollectFees,
        [207, 117, 95, 191, 229, 180, 226, 15] => K::CollectFeesV2,
        [70, 5, 132, 87, 86, 235, 177, 34] => K::CollectReward,
        [177, 107, 37, 180, 160, 19, 49, 209] => K::CollectRewardV2,
        [123, 134, 81, 0, 49, 68, 98, 98] => K::ClosePosition,
        [1, 182, 135, 59, 155, 25, 99, 223] => K::ClosePositionWithTokenExtensions,
        [41, 36, 216, 245, 27, 85, 103, 67] => K::CloseBundledPosition,
        other => K::Other(other),
    };
    metadata.event_type = EventType::WhirlpoolInstruction;
    Some(TxEvent::WhirlpoolInstructionEvent(WhirlpoolInstructionEvent {
        metadata,
        kind,
        execution_events: Vec::new(),
    }))
}

pub fn parse_execution_event_from_program_data(
    item: &ProgramDataItem,
) -> Option<WhirlpoolExecutionEvent> {
    if item.program_id != WHIRLPOOL_PROGRAM_ID {
        return None;
    }
    let bytes = item.data;
    let disc = bytes.get(..8)?;
    let data = bytes.get(8..)?;
    let key = |o| Pubkey::try_from(data.get(o..o + 32)?).ok();
    let i32_at =
        |o| -> Option<i32> { Some(i32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?)) };
    let u128_at =
        |o| -> Option<u128> { Some(u128::from_le_bytes(data.get(o..o + 16)?.try_into().ok()?)) };
    match disc {
        [237, 175, 243, 230, 147, 117, 101, 121] => Some(WhirlpoolExecutionEvent::PositionOpened {
            whirlpool: key(0)?,
            position: key(32)?,
            tick_lower: i32_at(64)?,
            tick_upper: i32_at(68)?,
        }),
        [100, 118, 173, 87, 12, 198, 254, 229] => Some(WhirlpoolExecutionEvent::PoolInitialized {
            whirlpool: key(0)?,
            config: key(32)?,
            mint_a: key(64)?,
            mint_b: key(96)?,
            tick_spacing: u16::from_le_bytes(data.get(128..130)?.try_into().ok()?),
            token_program_a: key(130)?,
            token_program_b: key(162)?,
            decimals_a: *data.get(194)?,
            decimals_b: *data.get(195)?,
            initial_sqrt_price: u128_at(196)?,
        }),
        [30, 7, 144, 181, 102, 254, 155, 161] | [166, 1, 36, 71, 112, 202, 181, 171] => {
            let increasing = disc[0] == 30;
            Some(WhirlpoolExecutionEvent::LiquidityChanged {
                whirlpool: key(0)?,
                position: key(32)?,
                tick_lower: i32_at(64)?,
                tick_upper: i32_at(68)?,
                liquidity: u128_at(72)?,
                increasing,
            })
        }
        [95, 130, 181, 132, 251, 50, 195, 38] => {
            Some(WhirlpoolExecutionEvent::LiquidityRepositioned {
                whirlpool: key(0)?,
                position: key(32)?,
                old_lower: i32_at(64)?,
                old_upper: i32_at(68)?,
                new_lower: i32_at(72)?,
                new_upper: i32_at(76)?,
                old_liquidity: u128_at(80)?,
                new_liquidity: u128_at(96)?,
            })
        }
        discriminators::TRADED_EVENT => {
            let traded = parse_traded_event_from_bytes(item.data)?;
            Some(WhirlpoolExecutionEvent::Traded {
                whirlpool: traded.whirlpool,
                a_to_b: traded.a_to_b,
                pre_sqrt_price: traded.pre_sqrt_price,
                post_sqrt_price: traded.post_sqrt_price,
                input_amount: traded.input_amount,
                output_amount: traded.output_amount,
                lp_fee: traded.lp_fee,
                protocol_fee: traded.protocol_fee,
            })
        }
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
) -> Option<TxEvent> {
    None
}

/// 解析 Whirlpool 账户数据
///
/// 根据判别器路由到具体的账户解析函数
pub fn parse_whirlpool_account_data(
    discriminator: &[u8],
    account: crate::streaming::grpc::AccountFrame,
    metadata: crate::streaming::event_parser::common::EventMetadata,
) -> Option<crate::streaming::event_parser::AccountEvent> {
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
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::WhirlpoolSwap;

    if data.len() < 34 || accounts.len() < 11 {
        return None;
    }

    Some(TxEvent::WhirlpoolSwapEvent(WhirlpoolSwapEvent {
        metadata,
        amount: read_u64_le(data, 0)?,
        other_amount_threshold: read_u64_le(data, 8)?,
        sqrt_price_limit: read_u128_le(data, 16)?,
        amount_specified_is_input: read_u8_le(data, 32)? != 0,
        a_to_b: read_u8_le(data, 33)? != 0,
        token_program: *accounts.get(0)?,
        token_authority: *accounts.get(1)?,
        whirlpool: *accounts.get(2)?,
        token_owner_account_a: *accounts.get(3)?,
        token_vault_a: *accounts.get(4)?,
        token_owner_account_b: *accounts.get(5)?,
        token_vault_b: *accounts.get(6)?,
        tick_array_0: *accounts.get(7)?,
        tick_array_1: *accounts.get(8)?,
        tick_array_2: *accounts.get(9)?,
        oracle: *accounts.get(10)?,
        remaining_account_indices: accounts.indices_from(11).collect(),
        ..Default::default()
    }))
}

fn parse_swap_v2_instruction(
    data: &[u8],
    accounts: InstructionAccounts<'_>,
    mut metadata: EventMetadata,
) -> Option<TxEvent> {
    metadata.event_type = EventType::WhirlpoolSwapV2;

    if data.len() < 34 || accounts.len() < 15 {
        return None;
    }

    Some(TxEvent::WhirlpoolSwapV2Event(WhirlpoolSwapV2Event {
        metadata,
        amount: read_u64_le(data, 0)?,
        other_amount_threshold: read_u64_le(data, 8)?,
        sqrt_price_limit: read_u128_le(data, 16)?,
        amount_specified_is_input: read_u8_le(data, 32)? != 0,
        a_to_b: read_u8_le(data, 33)? != 0,
        token_program_a: *accounts.get(0)?,
        token_program_b: *accounts.get(1)?,
        memo_program: *accounts.get(2)?,
        token_authority: *accounts.get(3)?,
        whirlpool: *accounts.get(4)?,
        token_mint_a: *accounts.get(5)?,
        token_mint_b: *accounts.get(6)?,
        token_owner_account_a: *accounts.get(7)?,
        token_vault_a: *accounts.get(8)?,
        token_owner_account_b: *accounts.get(9)?,
        token_vault_b: *accounts.get(10)?,
        tick_array_0: *accounts.get(11)?,
        tick_array_1: *accounts.get(12)?,
        tick_array_2: *accounts.get(13)?,
        oracle: *accounts.get(14)?,
        remaining_account_indices: accounts.indices_from(15).collect(),
        ..Default::default()
    }))
}

/// 从 Anchor Program data 日志解析 Traded 事件
///
/// 日志格式: "Program data: <base64>"
/// 编码格式: [8字节事件鉴别器][borsh(Traded)]
pub fn parse_traded_event_from_bytes(decoded: &[u8]) -> Option<TradedEventLogData> {
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
    let event_data = parse_traded_event_from_bytes(item.data)?;
    if &event_data.whirlpool != expected_whirlpool {
        return None;
    }
    Some(event_data)
}

/// Classify before allocating or decoding a protocol instruction.
pub(crate) fn instruction_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::SWAP => Some(EventType::WhirlpoolSwap),
        discriminators::SWAP_V2 => Some(EventType::WhirlpoolSwapV2),
        _ => Some(EventType::WhirlpoolInstruction),
    }
}

/// Classify before allocating or decoding a protocol account.
pub(crate) fn account_event_type(discriminator: &[u8]) -> Option<EventType> {
    match discriminator {
        discriminators::WHIRLPOOL => Some(EventType::AccountWhirlpool),
        discriminators::TICK_ARRAY => Some(EventType::AccountWhirlpoolTickArray),
        _ => None,
    }
}
