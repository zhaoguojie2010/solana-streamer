//! Bounded owned delivery with explicit overload and stale-delivery errors.
use crate::streaming::{
    event_parser::protocols::block::block_meta_event::BlockMetaEvent,
    event_parser::{AccountFrame, TxBatch},
};
use std::{
    ops::Deref,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};
use yellowstone_grpc_proto::geyser::SubscribeUpdateSlot;

#[derive(Debug)]
pub enum OwnedStreamEvent {
    Transaction(TxBatch),
    Account(AccountFrame),
    BlockMeta(BlockMetaEvent),
    Slot(SubscribeUpdateSlot),
}
impl OwnedStreamEvent {
    pub fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Transaction(tx) => tx.retained_bytes() - std::mem::size_of_val(tx),
                Self::Account(a) => a.data.len(),
                Self::BlockMeta(b) => b.block_hash.capacity(),
                Self::Slot(s) => s.dead_error.as_ref().map_or(0, String::capacity),
            }
    }
}

#[derive(Clone, Debug)]
pub struct QueueConfig {
    pub capacity: usize,
    /// Payload capacity budget, including received items until their envelope is dropped.
    /// Bytes' backing allocation and allocator/channel overhead are not observable here;
    /// this bounds charged payload bytes, not process RSS.
    pub max_bytes: usize,
    /// A stale item produces an error instead of being delivered.
    pub max_age: Duration,
}
impl Default for QueueConfig {
    fn default() -> Self {
        Self { capacity: 256, max_bytes: 64 * 1024 * 1024, max_age: Duration::from_secs(5) }
    }
}

/// Keep this envelope alive while processing. Its byte permit is released on drop.
#[derive(Debug)]
pub struct QueuedEvent {
    event: OwnedStreamEvent,
    received_at: Instant,
    _permit: OwnedSemaphorePermit,
}
impl Deref for QueuedEvent {
    type Target = OwnedStreamEvent;
    fn deref(&self) -> &Self::Target {
        &self.event
    }
}

pub struct QueuedStream {
    receiver: mpsc::Receiver<QueuedEvent>,
    failure: Arc<Mutex<Option<String>>>,
    max_age: Duration,
    ended: bool,
}
impl QueuedStream {
    /// On overload, drain already accepted items in order, then return the failure.
    /// After a stale-delivery error the receiver is closed and remaining items are discarded.
    pub async fn recv(&mut self) -> anyhow::Result<Option<QueuedEvent>> {
        if self.ended {
            return Ok(None);
        }
        match self.receiver.recv().await {
            Some(event) if event.received_at.elapsed() <= self.max_age => Ok(Some(event)),
            Some(_) => {
                self.ended = true;
                *self.failure.lock().unwrap() =
                    Some("event queue maximum delivery age exceeded".into());
                self.receiver.close();
                while self.receiver.try_recv().is_ok() {}
                anyhow::bail!("event queue maximum delivery age exceeded");
            }
            None => {
                self.ended = true;
                if let Some(error) = self.failure.lock().unwrap().as_ref() {
                    anyhow::bail!("{error}");
                }
                Ok(None)
            }
        }
    }
}

pub(crate) struct QueueSender {
    sender: mpsc::Sender<QueuedEvent>,
    budget: Arc<Semaphore>,
    failure: Arc<Mutex<Option<String>>>,
}
impl QueueSender {
    pub fn channel(config: QueueConfig) -> anyhow::Result<(Self, QueuedStream)> {
        anyhow::ensure!(
            config.capacity > 0
                && config.max_bytes > 0
                && config.max_bytes <= u32::MAX as usize
                && !config.max_age.is_zero(),
            "queue limits must be positive and max_bytes must fit u32"
        );
        let (sender, receiver) = mpsc::channel(config.capacity);
        let failure = Arc::new(Mutex::new(None));
        Ok((
            Self {
                sender,
                budget: Arc::new(Semaphore::new(config.max_bytes)),
                failure: failure.clone(),
            },
            QueuedStream { receiver, failure, max_age: config.max_age, ended: false },
        ))
    }
    pub fn send(&self, event: OwnedStreamEvent, received_at: Instant) -> anyhow::Result<()> {
        let bytes = u32::try_from(event.retained_bytes())
            .map_err(|_| anyhow::anyhow!("event exceeds queue byte budget"))?;
        let permit = self.budget.clone().try_acquire_many_owned(bytes).map_err(|_| {
            anyhow::anyhow!("event queue byte budget exceeded; subscription stopped")
        })?;
        self.sender.try_send(QueuedEvent { event, received_at, _permit: permit }).map_err(|error| {
            anyhow::anyhow!("event queue unavailable ({error}); subscription stopped")
        })
    }
    pub async fn closed(&self) -> anyhow::Result<()> {
        self.sender.closed().await;
        if let Some(error) = self.failure.lock().unwrap().as_ref() {
            anyhow::bail!("{error}");
        }
        Ok(())
    }
    pub fn fail(&self, error: &str) {
        *self.failure.lock().unwrap() = Some(error.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(slot: u64) -> OwnedStreamEvent {
        OwnedStreamEvent::Account(AccountFrame {
            slot,
            data: vec![0; 100].into(),
            ..AccountFrame::default()
        })
    }
    #[tokio::test]
    async fn queue_capacity_is_fail_fast_and_accepted_items_drain_before_error() {
        let (sender, mut receiver) =
            QueueSender::channel(QueueConfig { capacity: 1, ..QueueConfig::default() }).unwrap();
        sender.send(event(7), Instant::now()).unwrap();
        let error = sender.send(event(8), Instant::now()).unwrap_err();
        sender.fail(&error.to_string());
        drop(sender);
        let value = receiver.recv().await.unwrap().unwrap();
        let OwnedStreamEvent::Account(account) = &*value else { panic!("wrong item") };
        assert_eq!(account.slot, 7);
        assert!(receiver.recv().await.unwrap_err().to_string().contains("queue unavailable"));
    }
    #[tokio::test]
    async fn received_envelopes_keep_their_byte_permits_until_dropped() {
        let size = event(0).retained_bytes();
        let (sender, mut receiver) = QueueSender::channel(QueueConfig {
            capacity: 2,
            max_bytes: size,
            max_age: Duration::from_secs(1),
        })
        .unwrap();
        sender.send(event(1), Instant::now()).unwrap();
        let held = receiver.recv().await.unwrap().unwrap();
        assert!(sender.send(event(2), Instant::now()).is_err());
        drop(held);
        sender.send(event(3), Instant::now()).unwrap();
        drop(sender);
        assert!(receiver.recv().await.unwrap().is_some());
        assert!(receiver.recv().await.unwrap().is_none());
    }
    #[tokio::test]
    async fn stale_queue_items_are_explicit_errors() {
        let (sender, mut receiver) = QueueSender::channel(QueueConfig::default()).unwrap();
        sender.send(event(1), Instant::now() - Duration::from_secs(10)).unwrap();
        assert!(receiver.recv().await.unwrap_err().to_string().contains("maximum delivery age"));
        assert!(sender.send(event(2), Instant::now()).is_err());
    }
}
