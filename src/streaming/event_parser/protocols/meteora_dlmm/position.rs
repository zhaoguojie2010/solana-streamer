//! Minimal quote-relevant PositionV2 state, including the official dynamic-position tail.
use anyhow::{Context, Result};
use solana_sdk::pubkey::Pubkey;

pub const POSITION_V2_DISCRIMINATOR: [u8; 8] = [117, 176, 212, 199, 245, 180, 133, 182];
pub const POSITION_V2_PAYLOAD_SIZE: usize = 8112;
pub const POSITION_BIN_DATA_SIZE: usize = 112;
pub const MAX_POSITION_BINS: usize = 1400;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PositionLiquidityState {
    pub lb_pair: Pubkey,
    pub lower_bin_id: i32,
    pub width: i32,
    /// Q64 LP shares; these must use the same units as Bin::liquidity_supply.
    pub liquidity_shares: Vec<u128>,
}

/// Decode the fixed 70-bin prefix plus PositionBinData entries for wider positions.
/// Layout follows the official SDK's POSITION_MIN_SIZE and decodeExtendedPosition.
pub fn decode_position_liquidity(data: &[u8]) -> Result<PositionLiquidityState> {
    anyhow::ensure!(
        data.get(..8) == Some(POSITION_V2_DISCRIMINATOR.as_slice()),
        "DLMM PositionV2 discriminator mismatch"
    );
    anyhow::ensure!(
        data.len() >= 8 + POSITION_V2_PAYLOAD_SIZE,
        "DLMM PositionV2 truncated: bytes={}",
        data.len()
    );
    let lb_pair =
        Pubkey::new_from_array(data.get(8..40).context("PositionV2 pool missing")?.try_into()?);
    let lower_bin_id = i32::from_le_bytes(
        data.get(7912..7916).context("PositionV2 lower bin missing")?.try_into()?,
    );
    let upper_bin_id = i32::from_le_bytes(
        data.get(7916..7920).context("PositionV2 upper bin missing")?.try_into()?,
    );
    let width = upper_bin_id
        .checked_sub(lower_bin_id)
        .and_then(|n| n.checked_add(1))
        .context("PositionV2 range overflow")?;
    anyhow::ensure!(
        lower_bin_id >= -443636
            && upper_bin_id <= 443636
            && width > 0
            && usize::try_from(width)? <= MAX_POSITION_BINS,
        "DLMM PositionV2 invalid range: lower={lower_bin_id} upper={upper_bin_id}"
    );
    let count = usize::try_from(width)?;
    let required = (8 + POSITION_V2_PAYLOAD_SIZE)
        .checked_add(
            count
                .saturating_sub(70)
                .checked_mul(POSITION_BIN_DATA_SIZE)
                .context("PositionV2 tail overflow")?,
        )
        .context("PositionV2 size overflow")?;
    anyhow::ensure!(
        data.len() >= required,
        "DLMM PositionV2 extended shares truncated: width={width} bytes={} required={required}",
        data.len()
    );
    let mut liquidity_shares = Vec::with_capacity(count);
    for index in 0..count {
        let offset = if index < 70 {
            72 + index * 16
        } else {
            8 + POSITION_V2_PAYLOAD_SIZE + (index - 70) * POSITION_BIN_DATA_SIZE
        };
        liquidity_shares.push(u128::from_le_bytes(
            data.get(offset..offset + 16).context("PositionV2 share missing")?.try_into()?,
        ));
    }
    Ok(PositionLiquidityState { lb_pair, lower_bin_id, width, liquidity_shares })
}
