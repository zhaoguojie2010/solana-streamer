use crate::streaming::event_parser::{
    common::{
        read_i32_le, read_u128_le, read_u64_le, read_u8_le, EventMetadata, EventType,
        ProgramDataItem,
    },
    protocols::pancakeswap::{discriminators, PancakeSwapSwapEvent, PancakeSwapSwapV2Event},
    DexEvent,
};
use solana_sdk::pubkey::Pubkey;

/// PancakeSwap V3 程序ID（Solana）
pub const PANCAKESWAP_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("HpNfyc2Saw7RKkQd8nEL4khUcuPhQ7WwY1B2qjx8jxFq");

/// 解析 PancakeSwap instruction data
pub fn parse_pancakeswap_instruction_data(
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

pub fn is_pancakeswap_swap_instruction(discriminator: &[u8]) -> bool {
    matches!(discriminator, discriminators::SWAP | discriminators::SWAP_V2)
}

/// 解析 PancakeSwap inner instruction data
///
/// 当前 PancakeSwap 仅使用 instruction 数据，未使用同程序 inner event 日志。
pub fn parse_pancakeswap_inner_instruction_data(
    _discriminator: &[u8],
    _data: &[u8],
    _metadata: EventMetadata,
) -> Option<DexEvent> {
    None
}

/// 解析 PancakeSwap 账户数据
///
/// 当前暂无账户事件解析。
pub fn parse_pancakeswap_account_data(
    _discriminator: &[u8],
    _account: crate::streaming::grpc::AccountPretty,
    _metadata: crate::streaming::event_parser::common::EventMetadata,
) -> Option<crate::streaming::event_parser::DexEvent> {
    None
}

fn parse_swap_instruction(
    data: &[u8],
    accounts: &[Pubkey],
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::PancakeSwapSwap;

    // 链上观测：目前已出现 33 字节布局（无 is_base_input 字段）
    // amount(u64) + other_amount_threshold(u64) + sqrt_price_limit(u128) + a_to_b(bool)
    // 兼容 34 字节布局（含 is_base_input + a_to_b）
    if data.len() < 33 || accounts.len() < 12 {
        return None;
    }

    let (is_base_input, _) = parse_amount_flags(data)?;
    // 33 字节布局缺少 is_base_input 字段时，按链上行为回退为 true（exact-in）
    let is_base_input = is_base_input.or(Some(true));
    // Swap 实际账户顺序（链上实测）:
    // [2]=token_authority_or_signer, [3]=input_token_account, [4]=output_token_account,
    // [5]=input_vault, [6]=output_vault
    let input_token_account = accounts[3];
    let output_token_account = accounts[4];
    let input_vault = accounts[5];
    let output_vault = accounts[6];

    Some(DexEvent::PancakeSwapSwapEvent(PancakeSwapSwapEvent {
        metadata,
        amount: read_u64_le(data, 0)?,
        other_amount_threshold: read_u64_le(data, 8)?,
        sqrt_price_limit: read_u128_le(data, 16)?,
        is_base_input,
        token_authority: accounts[0],
        pool: accounts[1],
        input_token_account,
        output_token_account,
        input_vault,
        output_vault,
        account_6: accounts[6],
        account_7: accounts[7],
        token_program: accounts[8],
        account_9: accounts[9],
        account_10: accounts[10],
        account_11: accounts[11],
        remaining_accounts: accounts[12..].to_vec(),
        ..Default::default()
    }))
}

fn parse_swap_v2_instruction(
    data: &[u8],
    accounts: &[Pubkey],
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::PancakeSwapSwapV2;

    // 链上观测：当前 SwapV2 参数区也是 33 字节（同 Swap）。
    if data.len() < 33 || accounts.len() < 20 {
        return None;
    }

    let (is_base_input, _) = parse_amount_flags(data)?;
    // SwapV2 实际账户顺序（链上实测）:
    // [2]=payer_or_owner, [3]=input_token_account, [4]=output_token_account,
    // [5]=input_vault, [6]=output_vault, [11]=input_mint, [12]=output_mint
    let input_token_account = accounts[3];
    let output_token_account = accounts[4];
    let input_vault = accounts[5];
    let output_vault = accounts[6];
    let input_mint = accounts[11];
    let output_mint = accounts[12];

    Some(DexEvent::PancakeSwapSwapV2Event(PancakeSwapSwapV2Event {
        metadata,
        amount: read_u64_le(data, 0)?,
        other_amount_threshold: read_u64_le(data, 8)?,
        sqrt_price_limit: read_u128_le(data, 16)?,
        is_base_input,
        token_authority: accounts[0],
        pool: accounts[1],
        input_token_account,
        output_token_account,
        input_vault,
        output_vault,
        input_mint,
        output_mint,
        account_6: accounts[6],
        account_7: accounts[7],
        token_program_a: accounts[8],
        token_program_b: accounts[9],
        memo_program: accounts[10],
        account_13: accounts[13],
        account_14: accounts[14],
        account_15: accounts[15],
        account_16: accounts[16],
        account_17: accounts[17],
        account_18: accounts[18],
        account_19: accounts[19],
        remaining_accounts: accounts[20..].to_vec(),
        ..Default::default()
    }))
}

#[inline]
fn parse_amount_flags(data: &[u8]) -> Option<(Option<bool>, bool)> {
    let is_base_input =
        if data.len() >= 34 { Some(read_u8_le(data, 32)? != 0) } else { None };
    let a_to_b =
        if data.len() >= 34 { read_u8_le(data, 33)? != 0 } else { read_u8_le(data, 32)? != 0 };
    Some((is_base_input, a_to_b))
}

/// PancakeSwap Program data 中 SwapEvent 解码结果
#[derive(Debug, Clone, Default)]
pub struct SwapEventLogData {
    pub log_account_0: Pubkey,
    pub log_account_1: Pubkey,
    pub log_account_2: Pubkey,
    pub log_account_3: Pubkey,
    pub amount_0: u64,
    pub transfer_fee_0: u64,
    pub amount_1: u64,
    pub transfer_fee_1: u64,
    pub zero_for_one: bool,
    pub sqrt_price_x64: u128,
    pub liquidity: u128,
    pub tick: i32,
}

/// 从 Anchor Program data 日志解析 SwapEvent
///
/// 日志格式: "Program data: <base64>"
/// 编码格式: [8字节事件鉴别器][borsh(SwapEvent)]
pub fn parse_swap_event_from_log(log_data_base64: &str) -> Option<SwapEventLogData> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let decoded = STANDARD.decode(log_data_base64).ok()?;
    if decoded.len() < 8 {
        return None;
    }
    if &decoded[0..8] != discriminators::SWAP_EVENT {
        return None;
    }

    let mut offset = 8;
    let log_account_0 = Pubkey::new_from_array(decoded.get(offset..offset + 32)?.try_into().ok()?);
    offset += 32;
    let log_account_1 = Pubkey::new_from_array(decoded.get(offset..offset + 32)?.try_into().ok()?);
    offset += 32;
    let log_account_2 = Pubkey::new_from_array(decoded.get(offset..offset + 32)?.try_into().ok()?);
    offset += 32;
    let log_account_3 = Pubkey::new_from_array(decoded.get(offset..offset + 32)?.try_into().ok()?);
    offset += 32;

    let amount_0 = read_u64_le(&decoded, offset)?;
    offset += 8;
    let transfer_fee_0 = read_u64_le(&decoded, offset)?;
    offset += 8;
    let amount_1 = read_u64_le(&decoded, offset)?;
    offset += 8;
    let transfer_fee_1 = read_u64_le(&decoded, offset)?;
    offset += 8;
    let zero_for_one = read_u8_le(&decoded, offset)? != 0;
    offset += 1;
    let sqrt_price_x64 = read_u128_le(&decoded, offset)?;
    offset += 16;
    let liquidity = read_u128_le(&decoded, offset)?;
    offset += 16;
    let tick = read_i32_le(&decoded, offset)?;

    Some(SwapEventLogData {
        log_account_0,
        log_account_1,
        log_account_2,
        log_account_3,
        amount_0,
        transfer_fee_0,
        amount_1,
        transfer_fee_1,
        zero_for_one,
        sqrt_price_x64,
        liquidity,
        tick,
    })
}

/// 从 ProgramDataItem 解析 SwapEvent
pub fn parse_swap_event_from_program_data(item: &ProgramDataItem) -> Option<SwapEventLogData> {
    if item.program_id != PANCAKESWAP_PROGRAM_ID {
        return None;
    }
    parse_swap_event_from_log(&item.base64)
}
