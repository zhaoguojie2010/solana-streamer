use tokio::{sync::oneshot, task::JoinHandle};

/// A subscription owns one reader/parser task. Graceful shutdown finishes the current
/// synchronous callback, closes production, and leaves accepted queue items available to drain.
pub struct SubscriptionHandle {
    task: JoinHandle<anyhow::Result<()>>,
    shutdown: Option<oneshot::Sender<()>>,
}
impl SubscriptionHandle {
    pub(crate) fn new(task: JoinHandle<anyhow::Result<()>>, shutdown: oneshot::Sender<()>) -> Self {
        Self { task, shutdown: Some(shutdown) }
    }
    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }
    pub async fn shutdown(mut self) -> anyhow::Result<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.await?
    }
    pub async fn join(self) -> anyhow::Result<()> {
        self.task.await?
    }
    pub fn abort(self) {
        self.task.abort();
    }
}
