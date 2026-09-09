use crate::streaming::{
    common::SimdUtils,
    event_parser::{common::SwapData, TxEvent},
};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

lazy_static::lazy_static! {
    static ref SOL_MINT: Pubkey = Pubkey::from_str("So11111111111111111111111111111111111111111").unwrap();
    static ref SYSTEM_PROGRAMS: [Pubkey; 3] = [
        Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap(),
        Pubkey::from_str("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb").unwrap(),
        Pubkey::from_str("11111111111111111111111111111111").unwrap(),
    ];
}

/// Parse token transfer data from next instructions
pub fn parse_swap_data_from_next_instructions(
    event: &TxEvent,
    inner_instruction: &super::types::InnerInstructions,
    current_index: i8,
    accounts: &[Pubkey],
) -> Option<SwapData> {
    let mut swap_data = SwapData {
        from_mint: Pubkey::default(),
        to_mint: Pubkey::default(),
        from_amount: 0,
        to_amount: 0,
        description: None,
    };

    // 先根据 event 取出关键信息
    // let mut user: Option<Pubkey> = None;
    let mut from_mint: Option<Pubkey> = None;
    let mut to_mint: Option<Pubkey> = None;
    let mut user_from_token: Option<Pubkey> = None;
    let mut user_to_token: Option<Pubkey> = None;
    let mut from_vault: Option<Pubkey> = None;
    let mut to_vault: Option<Pubkey> = None;

    match event {
        TxEvent::BonkTradeEvent(e) => {
            // user = Some(e.payer);
            from_mint = Some(e.base_token_mint);
            to_mint = Some(e.quote_token_mint);
            user_from_token = Some(e.user_base_token);
            user_to_token = Some(e.user_quote_token);
            from_vault = Some(e.base_vault);
            to_vault = Some(e.quote_vault);
        }
        TxEvent::PumpFunTradeEvent(e) => {
            swap_data.from_mint = if e.is_buy { *SOL_MINT } else { e.mint };
            swap_data.to_mint = if e.is_buy { e.mint } else { *SOL_MINT };
        }
        TxEvent::PumpSwapBuyEvent(e) => {
            swap_data.from_mint = e.quote_mint;
            swap_data.to_mint = e.base_mint;
        }
        TxEvent::PumpSwapBuyExactQuoteInEvent(e) => {
            swap_data.from_mint = e.quote_mint;
            swap_data.to_mint = e.base_mint;
        }
        TxEvent::PumpSwapSellEvent(e) => {
            swap_data.from_mint = e.base_mint;
            swap_data.to_mint = e.quote_mint;
        }
        TxEvent::PancakeSwapSwapEvent(e) => {
            swap_data.description =
                Some("Unable to get from_mint and to_mint from PancakeSwapSwapEvent".into());
            user_from_token = Some(e.input_token_account);
            user_to_token = Some(e.output_token_account);
            from_vault = Some(e.input_vault);
            to_vault = Some(e.output_vault);
        }
        TxEvent::PancakeSwapSwapV2Event(e) => {
            from_mint = Some(e.input_mint);
            to_mint = Some(e.output_mint);
            user_from_token = Some(e.input_token_account);
            user_to_token = Some(e.output_token_account);
            from_vault = Some(e.input_vault);
            to_vault = Some(e.output_vault);
        }
        TxEvent::RaydiumCpmmSwapEvent(e) => {
            // user = Some(e.payer);
            from_mint = Some(e.input_token_mint);
            to_mint = Some(e.output_token_mint);
            user_from_token = Some(e.input_token_account);
            user_to_token = Some(e.output_token_account);
            from_vault = Some(e.input_vault);
            to_vault = Some(e.output_vault);
        }
        TxEvent::RaydiumClmmSwapEvent(e) => {
            // user = Some(e.payer);
            swap_data.description =
                Some("Unable to get from_mint and to_mint from RaydiumClmmSwapEvent".into());
            user_from_token = Some(e.input_token_account);
            user_to_token = Some(e.output_token_account);
            from_vault = Some(e.input_vault);
            to_vault = Some(e.output_vault);
        }
        TxEvent::RaydiumClmmSwapV2Event(e) => {
            // user = Some(e.payer);
            from_mint = Some(e.input_vault_mint);
            to_mint = Some(e.output_vault_mint);
            user_from_token = Some(e.input_token_account);
            user_to_token = Some(e.output_token_account);
            from_vault = Some(e.input_vault);
            to_vault = Some(e.output_vault);
        }
        TxEvent::RaydiumAmmV4SwapEvent(e) => {
            // user = Some(e.user_source_owner);
            swap_data.description =
                Some("Unable to get from_mint and to_mint from RaydiumAmmV4SwapEvent".into());
            user_from_token = Some(e.user_source_token_account);
            user_to_token = Some(e.user_destination_token_account);
            from_vault = Some(e.pool_pc_token_account);
            to_vault = Some(e.pool_coin_token_account);
        }
        TxEvent::MeteoraDlmmSwapEvent(e) => {
            if e.swap_for_y {
                from_mint = e.token_x_mint;
                to_mint = e.token_y_mint;
                from_vault = e.reserve_x;
                to_vault = e.reserve_y;
            } else {
                from_mint = e.token_y_mint;
                to_mint = e.token_x_mint;
                from_vault = e.reserve_y;
                to_vault = e.reserve_x;
            }
            user_from_token = e.user_token_in;
            user_to_token = e.user_token_out;
        }
        TxEvent::MeteoraDlmmSwap2Event(e) => {
            if e.swap_for_y {
                from_mint = e.token_x_mint;
                to_mint = e.token_y_mint;
                from_vault = e.reserve_x;
                to_vault = e.reserve_y;
            } else {
                from_mint = e.token_y_mint;
                to_mint = e.token_x_mint;
                from_vault = e.reserve_y;
                to_vault = e.reserve_x;
            }
            user_from_token = e.user_token_in;
            user_to_token = e.user_token_out;
        }
        TxEvent::WhirlpoolSwapEvent(e) => {
            swap_data.description =
                Some("Unable to get from_mint and to_mint from WhirlpoolSwapEvent".into());
            if e.a_to_b {
                user_from_token = Some(e.token_owner_account_a);
                user_to_token = Some(e.token_owner_account_b);
                from_vault = Some(e.token_vault_a);
                to_vault = Some(e.token_vault_b);
            } else {
                user_from_token = Some(e.token_owner_account_b);
                user_to_token = Some(e.token_owner_account_a);
                from_vault = Some(e.token_vault_b);
                to_vault = Some(e.token_vault_a);
            }
        }
        TxEvent::WhirlpoolSwapV2Event(e) => {
            if e.a_to_b {
                from_mint = Some(e.token_mint_a);
                to_mint = Some(e.token_mint_b);
                user_from_token = Some(e.token_owner_account_a);
                user_to_token = Some(e.token_owner_account_b);
                from_vault = Some(e.token_vault_a);
                to_vault = Some(e.token_vault_b);
            } else {
                from_mint = Some(e.token_mint_b);
                to_mint = Some(e.token_mint_a);
                user_from_token = Some(e.token_owner_account_b);
                user_to_token = Some(e.token_owner_account_a);
                from_vault = Some(e.token_vault_b);
                to_vault = Some(e.token_vault_a);
            }
        }
        _ => {}
    }

    let user_to_token = user_to_token.unwrap_or_default();
    let user_from_token = user_from_token.unwrap_or_default();
    let to_vault = to_vault.unwrap_or_default();
    let from_vault = from_vault.unwrap_or_default();
    let to_mint = to_mint.unwrap_or_default();
    let from_mint = from_mint.unwrap_or_default();

    // 单次循环完成提取和判断
    for instruction in inner_instruction.instructions.iter().skip((current_index + 1) as usize) {
        let compiled = &instruction.instruction;
        let program_id = accounts[compiled.program_id_index as usize];
        if !SYSTEM_PROGRAMS.contains(&program_id) {
            break;
        }
        let data = &compiled.data;

        // 使用 SIMD 验证数据格式
        if !SimdUtils::validate_data_format(data, 8) {
            continue;
        }

        let get_pubkey = |i: usize| accounts[compiled.accounts[i] as usize];
        let (source, destination, amount) = match data[0] {
            12 if compiled.accounts.len() >= 4 => {
                let amt = u64::from_le_bytes(data[1..9].try_into().unwrap());
                (get_pubkey(0), get_pubkey(2), amt)
            }
            3 if compiled.accounts.len() >= 3 => {
                let amt = u64::from_le_bytes(data[1..9].try_into().unwrap());
                (get_pubkey(0), get_pubkey(1), amt)
            }
            2 if compiled.accounts.len() >= 2 => {
                let amt = u64::from_le_bytes(data[4..12].try_into().unwrap());
                (get_pubkey(0), get_pubkey(1), amt)
            }
            _ => continue,
        };

        match (source, destination) {
            (s, d) if s == user_to_token && d == to_vault => {
                swap_data.from_mint = to_mint;
                swap_data.from_amount = amount;
            }
            (s, d) if s == from_vault && d == user_from_token => {
                swap_data.to_mint = from_mint;
                swap_data.to_amount = amount;
            }
            (s, d) if s == user_from_token && d == from_vault => {
                swap_data.from_mint = from_mint;
                swap_data.from_amount = amount;
            }
            (s, d) if s == to_vault && d == user_to_token => {
                swap_data.to_mint = to_mint;
                swap_data.to_amount = amount;
            }
            (s, d) if s == user_from_token && d == to_vault => {
                swap_data.from_mint = from_mint;
                swap_data.from_amount = amount;
            }
            (s, d) if s == from_vault && d == user_to_token => {
                swap_data.to_mint = to_mint;
                swap_data.to_amount = amount;
            }
            _ => {}
        }
        if swap_data.from_mint != Pubkey::default() && swap_data.to_mint != Pubkey::default() {
            break;
        }
        if swap_data.from_amount != 0 && swap_data.to_amount != 0 {
            break;
        }
    }

    if swap_data.from_mint != Pubkey::default()
        || swap_data.to_mint != Pubkey::default()
        || swap_data.from_amount != 0
        || swap_data.to_amount != 0
    {
        Some(swap_data)
    } else {
        None
    }
}
