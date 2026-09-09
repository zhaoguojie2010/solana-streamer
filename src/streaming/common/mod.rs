// 公用模块 - 包含流处理相关的通用功能
pub mod config;
pub mod constants;
pub mod metrics;
pub mod simd_utils;
pub mod subscription;

// 重新导出主要类型
pub use config::*;
pub use constants::*;
pub use metrics::*;
pub use simd_utils::*;
pub use subscription::*;

pub mod event_queue;
pub use event_queue::{OwnedStreamEvent, QueueConfig, QueuedEvent, QueuedStream};
