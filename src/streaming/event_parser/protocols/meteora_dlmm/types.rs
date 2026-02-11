use borsh::BorshDeserialize;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use solana_sdk::pubkey::Pubkey;

use crate::streaming::{
    event_parser::{
        common::{EventMetadata, EventType},
        protocols::meteora_dlmm::{MeteoraDlmmBinArrayAccountEvent, MeteoraDlmmBinArrayBitmapExtensionAccountEvent, MeteoraDlmmLbPairAccountEvent},
        DexEvent,
    },
    grpc::AccountPretty,
};

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct StaticParameters {
    pub base_factor: u16,
    pub filter_period: u16,
    pub decay_period: u16,
    pub reduction_factor: u16,
    pub variable_fee_control: u32,
    pub max_volatility_accumulator: u32,
    pub min_bin_id: i32,
    pub max_bin_id: i32,
    pub protocol_share: u16,
    pub base_fee_power_factor: u8,
    pub padding: [u8; 5],
}

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct VariableParameters {
    pub volatility_accumulator: u32,
    pub volatility_reference: u32,
    pub index_reference: i32,
    pub padding: [u8; 4],
    pub last_update_timestamp: i64,
    pub padding1: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct ProtocolFee {
    pub amount_x: u64,
    pub amount_y: u64,
}

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct RewardInfo {
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub funder: Pubkey,
    pub reward_duration: u64,
    pub reward_duration_end: u64,
    pub reward_rate: u128,
    pub last_update_time: u64,
    pub cumulative_seconds_with_empty_liquidity_reward: u64,
}

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct LbPair {
    pub parameters: StaticParameters,
    pub v_parameters: VariableParameters,
    pub bump_seed: [u8; 1],
    pub bin_step_seed: [u8; 2],
    pub pair_type: u8,
    pub active_id: i32,
    pub bin_step: u16,
    pub status: u8,
    pub require_base_factor_seed: u8,
    pub base_factor_seed: [u8; 2],
    pub activation_type: u8,
    pub creator_pool_on_off_control: u8,
    pub token_x_mint: Pubkey,
    pub token_y_mint: Pubkey,
    pub reserve_x: Pubkey,
    pub reserve_y: Pubkey,
    pub protocol_fee: ProtocolFee,
    pub padding1: [u8; 32],
    pub reward_infos: [RewardInfo; 2],
    pub oracle: Pubkey,
    pub bin_array_bitmap: [u64; 16],
    pub last_updated_at: i64,
    pub padding2: [u8; 32],
    pub pre_activation_swap_address: Pubkey,
    pub base_key: Pubkey,
    pub activation_point: u64,
    pub pre_activation_duration: u64,
    pub padding3: [u8; 8],
    pub padding4: u64,
    pub creator: Pubkey,
    pub token_mint_x_program_flag: u8,
    pub token_mint_y_program_flag: u8,
    pub reserved: [u8; 22],
}

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct BinArrayBitmapExtension {
    pub lb_pair: Pubkey,
    pub positive_bin_array_bitmap: [[u64; 8]; 12],
    pub negative_bin_array_bitmap: [[u64; 8]; 12],
}

/// Bin 结构体 - 表示一个价格区间内的流动性
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize, Copy)]
pub struct Bin {
    /// Token X 的数量（已排除协议费用）
    pub amount_x: u64,
    /// Token Y 的数量（已排除协议费用）
    pub amount_y: u64,
    /// Bin 价格
    pub price: u128,
    /// Bin 的流动性供应量（与 LP mint supply 相同）
    pub liquidity_supply: u128,
    /// 每个代币存储的奖励（reward_a_per_token_stored）
    pub reward_per_token_stored: [u128; 2],
    /// 每个流动性存入的 Token X 的交换费用金额
    pub fee_amount_x_per_token_stored: u128,
    /// 每个流动性存入的 Token Y 的交换费用金额
    pub fee_amount_y_per_token_stored: u128,
    /// 交换到 bin 的总 Token X 数量（仅用于跟踪）
    pub amount_x_in: u128,
    /// 交换到 bin 的总 Token Y 数量（仅用于跟踪）
    pub amount_y_in: u128,
}

/// BinArray 结构体 - 包含一个范围内的 bin
/// 例如：BinArray index: 0 包含 bin 0 <-> 599
/// index: 2 包含 bin 600 <-> 1199, ...
#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, BorshDeserialize)]
pub struct BinArray {
    /// BinArray 的索引
    pub index: i64,
    /// BinArray 的版本
    pub version: u8,
    /// 填充字节
    #[serde(with = "BigArray")]
    pub _padding: [u8; 7],
    /// 关联的 LbPair 地址
    pub lb_pair: Pubkey,
    /// Bin 数组（最多 70 个）
    #[serde(with = "BigArray")]
    pub bins: [Bin; 70],
}

impl Default for BinArray {
    fn default() -> Self {
        BinArray {
            index: 0,
            version: 0,
            _padding: [0; 7],
            lb_pair: Pubkey::default(),
            bins: [Bin::default(); 70],
        }
    }
}

pub const LB_PAIR_SIZE: usize = std::mem::size_of::<LbPair>();
pub const BIN_ARRAY_BITMAP_EXTENSION_SIZE: usize = std::mem::size_of::<BinArrayBitmapExtension>();
pub const BIN_ARRAY_SIZE: usize = std::mem::size_of::<BinArray>();

pub fn lb_pair_decode(data: &[u8]) -> Option<LbPair> {
    if data.len() < LB_PAIR_SIZE {
        return None;
    }
    borsh::from_slice::<LbPair>(&data[..LB_PAIR_SIZE]).ok()
}

pub fn lb_pair_parser(account: AccountPretty, mut metadata: EventMetadata) -> Option<DexEvent> {
    metadata.event_type = EventType::AccountMeteoraDlmmLbPair;

    if account.data.len() < LB_PAIR_SIZE + 8 {
        return None;
    }
    if let Some(lb_pair) = lb_pair_decode(&account.data[8..LB_PAIR_SIZE + 8]) {
        Some(DexEvent::MeteoraDlmmLbPairAccountEvent(
            MeteoraDlmmLbPairAccountEvent {
                metadata,
                pubkey: account.pubkey,
                executable: account.executable,
                lamports: account.lamports,
                owner: account.owner,
                rent_epoch: account.rent_epoch,
                raw_account_data: account.data,
                lb_pair,
            },
        ))
    } else {
        None
    }
}

pub fn bin_array_bitmap_extension_decode(data: &[u8]) -> Option<BinArrayBitmapExtension> {
    if data.len() < BIN_ARRAY_BITMAP_EXTENSION_SIZE {
        return None;
    }
    borsh::from_slice::<BinArrayBitmapExtension>(&data[..BIN_ARRAY_BITMAP_EXTENSION_SIZE]).ok()
}

pub fn bin_array_bitmap_extension_parser(account: AccountPretty, mut metadata: EventMetadata) -> Option<DexEvent> {
    metadata.event_type = EventType::AccountMeteoraDlmmBinArrayBitmapExtension;

    if account.data.len() < BIN_ARRAY_BITMAP_EXTENSION_SIZE + 8 {
        return None;
    }
    if let Some(bin_array_bitmap_extension) = bin_array_bitmap_extension_decode(&account.data[8..BIN_ARRAY_BITMAP_EXTENSION_SIZE + 8]) {
        Some(DexEvent::MeteoraDlmmBinArrayBitmapExtensionAccountEvent(
            MeteoraDlmmBinArrayBitmapExtensionAccountEvent {
                metadata,
                pubkey: account.pubkey,
                executable: account.executable,
                lamports: account.lamports,
                owner: account.owner,
                rent_epoch: account.rent_epoch,
                raw_account_data: account.data,
                bin_array_bitmap_extension,
            },
        ))
    } else {
        None
    }
}

pub fn bin_array_decode(data: &[u8]) -> Option<BinArray> {
    if data.len() < BIN_ARRAY_SIZE {
        return None;
    }
    
    // BinArray 使用 bytemuck 序列化，手动解析字节数组
    // 由于 Pubkey 不满足 Pod 要求，我们需要手动解析
    let mut offset = 0;
    
    // 解析 index (i64, 8 bytes)
    let index = i64::from_le_bytes([
        data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
        data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
    ]);
    offset += 8;
    
    // 解析 version (u8, 1 byte)
    let version = data[offset];
    offset += 1;
    
    // 解析 _padding (7 bytes)
    let mut _padding = [0u8; 7];
    _padding.copy_from_slice(&data[offset..offset + 7]);
    offset += 7;
    
    // 解析 lb_pair (Pubkey, 32 bytes)
    let lb_pair_bytes: [u8; 32] = data[offset..offset + 32].try_into().ok()?;
    let lb_pair = Pubkey::new_from_array(lb_pair_bytes);
    offset += 32;
    
    // 解析 bins (70 * Bin size)
    const BIN_SIZE: usize = std::mem::size_of::<Bin>();
    let mut bins = [Bin::default(); 70];
    for i in 0..70 {
        let bin_start = offset + i * BIN_SIZE;
        if bin_start + BIN_SIZE > data.len() {
            return None;
        }
        
        // 解析单个 Bin
        let bin_data = &data[bin_start..bin_start + BIN_SIZE];
        bins[i] = Bin {
            amount_x: u64::from_le_bytes(bin_data[0..8].try_into().ok()?),
            amount_y: u64::from_le_bytes(bin_data[8..16].try_into().ok()?),
            price: u128::from_le_bytes(bin_data[16..32].try_into().ok()?),
            liquidity_supply: u128::from_le_bytes(bin_data[32..48].try_into().ok()?),
            reward_per_token_stored: [
                u128::from_le_bytes(bin_data[48..64].try_into().ok()?),
                u128::from_le_bytes(bin_data[64..80].try_into().ok()?),
            ],
            fee_amount_x_per_token_stored: u128::from_le_bytes(bin_data[80..96].try_into().ok()?),
            fee_amount_y_per_token_stored: u128::from_le_bytes(bin_data[96..112].try_into().ok()?),
            amount_x_in: u128::from_le_bytes(bin_data[112..128].try_into().ok()?),
            amount_y_in: u128::from_le_bytes(bin_data[128..144].try_into().ok()?),
        };
    }
    
    Some(BinArray {
        index,
        version,
        _padding,
        lb_pair,
        bins,
    })
}

pub fn bin_array_parser(account: AccountPretty, mut metadata: EventMetadata) -> Option<DexEvent> {
    metadata.event_type = EventType::AccountMeteoraDlmmBinArray;

    // 跳过前 8 字节的 discriminator
    if account.data.len() < BIN_ARRAY_SIZE + 8 {
        return None;
    }
    if let Some(bin_array) = bin_array_decode(&account.data[8..BIN_ARRAY_SIZE + 8]) {
        Some(DexEvent::MeteoraDlmmBinArrayAccountEvent(
            MeteoraDlmmBinArrayAccountEvent {
                metadata,
                pubkey: account.pubkey,
                executable: account.executable,
                lamports: account.lamports,
                owner: account.owner,
                rent_epoch: account.rent_epoch,
                raw_account_data: account.data,
                bin_array,
            },
        ))
    } else {
        None
    }
}
