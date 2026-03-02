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
/// PancakeSwap CLMM 的账户布局与 Raydium CLMM 兼容：
/// - PoolState
/// - TickArrayState
/// - TickArrayBitmapExtension
///
/// 解码层复用 Raydium CLMM 的结构体，但产出 PancakeSwap 自有账户事件类型。
pub fn parse_pancakeswap_account_data(
    discriminator: &[u8],
    account: crate::streaming::grpc::AccountPretty,
    metadata: crate::streaming::event_parser::common::EventMetadata,
) -> Option<crate::streaming::event_parser::DexEvent> {
    use crate::streaming::event_parser::protocols::{
        pancakeswap::types, raydium_clmm::events::discriminators as clmm_discriminators,
    };

    match discriminator {
        d if d == clmm_discriminators::POOL_STATE => types::pool_state_parser(account, metadata),
        d if d == clmm_discriminators::TICK_ARRAY_STATE => {
            types::tick_array_state_parser(account, metadata)
        }
        d if d == clmm_discriminators::TICK_ARRAY_BITMAP_EXTENSION => {
            types::tick_array_bitmap_extension_parser(account, metadata)
        }
        _ => None,
    }
}

fn parse_swap_instruction(
    data: &[u8],
    accounts: &[Pubkey],
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::PancakeSwapSwap;

    // IDL 标准布局: amount(u64) + other_amount_threshold(u64) + sqrt_price_limit(u128) + is_base_input(bool)
    // 兼容历史 34 字节布局（尾部多 1 字节附加方向位）。
    if data.len() < 33 || accounts.len() < 10 {
        return None;
    }

    let is_base_input = parse_is_base_input(data)?;
    // Swap 账户顺序（IDL）:
    // [0]=payer, [1]=amm_config, [2]=pool_state, [3]=input_token_account, [4]=output_token_account,
    // [5]=input_vault, [6]=output_vault, [7]=observation_state, [8]=token_program, [9]=tick_array
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
        payer: accounts[0],
        amm_config: accounts[1],
        pool_state: accounts[2],
        input_token_account,
        output_token_account,
        input_vault,
        output_vault,
        observation_state: accounts[7],
        token_program: accounts[8],
        tick_array: accounts[9],
        remaining_accounts: accounts[10..].to_vec(),
        ..Default::default()
    }))
}

fn parse_swap_v2_instruction(
    data: &[u8],
    accounts: &[Pubkey],
    mut metadata: EventMetadata,
) -> Option<DexEvent> {
    metadata.event_type = EventType::PancakeSwapSwapV2;

    // IDL 标准布局同 Swap（33 字节），兼容历史 34 字节布局。
    if data.len() < 33 || accounts.len() < 13 {
        return None;
    }

    let is_base_input = parse_is_base_input(data)?;
    // SwapV2 账户顺序（IDL）:
    // [0]=payer, [1]=amm_config, [2]=pool_state, [3]=input_token_account, [4]=output_token_account,
    // [5]=input_vault, [6]=output_vault, [7]=observation_state, [8]=token_program, [9]=token_program_2022,
    // [10]=memo_program, [11]=input_vault_mint, [12]=output_vault_mint
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
        payer: accounts[0],
        amm_config: accounts[1],
        pool_state: accounts[2],
        input_token_account,
        output_token_account,
        input_vault,
        output_vault,
        observation_state: accounts[7],
        token_program: accounts[8],
        token_program_2022: accounts[9],
        memo_program: accounts[10],
        input_mint,
        output_mint,
        remaining_accounts: accounts[13..].to_vec(),
        ..Default::default()
    }))
}

#[inline]
fn parse_is_base_input(data: &[u8]) -> Option<bool> {
    if data.len() < 33 {
        return None;
    }
    Some(read_u8_le(data, 32)? != 0)
}

/// PancakeSwap Program data 中 SwapEvent 解码结果
#[derive(Debug, Clone, Default)]
pub struct SwapEventLogData {
    pub pool_state: Pubkey,
    pub sender: Pubkey,
    pub input_token_account: Pubkey,
    pub output_token_account: Pubkey,
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
    let pool_state = Pubkey::new_from_array(decoded.get(offset..offset + 32)?.try_into().ok()?);
    offset += 32;
    let sender = Pubkey::new_from_array(decoded.get(offset..offset + 32)?.try_into().ok()?);
    offset += 32;
    let input_token_account =
        Pubkey::new_from_array(decoded.get(offset..offset + 32)?.try_into().ok()?);
    offset += 32;
    let output_token_account =
        Pubkey::new_from_array(decoded.get(offset..offset + 32)?.try_into().ok()?);
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
        pool_state,
        sender,
        input_token_account,
        output_token_account,
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
pub fn parse_swap_event_from_program_data(
    item: &ProgramDataItem,
    expected_pool_state: &Pubkey,
) -> Option<SwapEventLogData> {
    if item.program_id != PANCAKESWAP_PROGRAM_ID {
        return None;
    }
    let event_data = parse_swap_event_from_log(&item.base64)?;
    if &event_data.pool_state != expected_pool_state {
        return None;
    }
    Some(event_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::event_parser::common::EventMetadata;

    fn sample_swap_data(is_base_input: u8) -> Vec<u8> {
        let mut data = vec![0u8; 33];
        data[0..8].copy_from_slice(&42u64.to_le_bytes());
        data[8..16].copy_from_slice(&7u64.to_le_bytes());
        data[16..32].copy_from_slice(&123u128.to_le_bytes());
        data[32] = is_base_input;
        data
    }

    fn unique_accounts(n: usize) -> Vec<Pubkey> {
        (0..n).map(|_| Pubkey::new_unique()).collect()
    }

    #[test]
    fn parses_swap_with_idl_min_accounts_and_correct_pool_index() {
        let accounts = unique_accounts(10);
        let data = sample_swap_data(0);
        let event = parse_pancakeswap_instruction_data(
            discriminators::SWAP,
            &data,
            &accounts,
            EventMetadata::default(),
        )
        .expect("swap should parse");

        match event {
            DexEvent::PancakeSwapSwapEvent(e) => {
                assert_eq!(e.pool_state, accounts[2]);
                assert_eq!(e.payer, accounts[0]);
                assert_eq!(e.amm_config, accounts[1]);
                assert_eq!(e.observation_state, accounts[7]);
                assert_eq!(e.tick_array, accounts[9]);
                assert_eq!(e.input_token_account, accounts[3]);
                assert_eq!(e.output_token_account, accounts[4]);
                assert!(!e.is_base_input);
            }
            _ => panic!("unexpected event type"),
        }
    }

    #[test]
    fn parses_swap_v2_with_idl_min_accounts_and_correct_pool_index() {
        let accounts = unique_accounts(13);
        let data = sample_swap_data(1);
        let event = parse_pancakeswap_instruction_data(
            discriminators::SWAP_V2,
            &data,
            &accounts,
            EventMetadata::default(),
        )
        .expect("swap_v2 should parse");

        match event {
            DexEvent::PancakeSwapSwapV2Event(e) => {
                assert_eq!(e.pool_state, accounts[2]);
                assert_eq!(e.payer, accounts[0]);
                assert_eq!(e.amm_config, accounts[1]);
                assert_eq!(e.observation_state, accounts[7]);
                assert_eq!(e.input_mint, accounts[11]);
                assert_eq!(e.output_mint, accounts[12]);
                assert!(e.is_base_input);
            }
            _ => panic!("unexpected event type"),
        }
    }

    #[test]
    fn parse_is_base_input_uses_byte_32_for_33_and_34_layout() {
        let mut data_33 = sample_swap_data(1);
        assert_eq!(parse_is_base_input(&data_33), Some(true));

        data_33.push(0);
        assert_eq!(parse_is_base_input(&data_33), Some(true));
    }
}
