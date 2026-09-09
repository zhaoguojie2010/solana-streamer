//! Per-client snapshots. Workers aggregate locally; disabled streams do no metrics work.
#[derive(Clone, Debug, Default)]
pub struct PerformanceMetrics {
    pub transactions: u64,
    pub accounts: u64,
    pub blocks: u64,
    pub events: u64,
    /// Parsing and synchronous delivery time, excluding network waits.
    pub processing_us: u64,
    pub max_processing_us: u64,
}
impl PerformanceMetrics {
    pub(crate) fn merge(&mut self, delta: &mut Self) {
        self.transactions += delta.transactions;
        self.accounts += delta.accounts;
        self.blocks += delta.blocks;
        self.events += delta.events;
        self.processing_us += delta.processing_us;
        self.max_processing_us = self.max_processing_us.max(delta.max_processing_us);
        *delta = Self::default();
    }
}
