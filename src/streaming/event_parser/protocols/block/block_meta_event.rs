use serde::{Deserialize, Serialize};
/// Block metadata is a control event, independent of transaction payloads.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockMetaEvent {
    pub slot: u64,
    pub block_hash: String,
    pub block_time_ms: Option<i64>,
    pub recv_us: i64,
}
