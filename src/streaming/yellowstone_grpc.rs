use crate::common::AnyResult;
use crate::streaming::{
    common::{
        event_queue::QueueSender, OwnedStreamEvent, PerformanceMetrics, QueueConfig, QueuedStream,
        StreamClientConfig, SubscriptionHandle,
    },
    event_parser::{
        protocols::block::block_meta_event::BlockMetaEvent, AccountFrame, AccountView, ParsePlan,
        TxFrame, TxParser, TxView,
    },
    grpc::{SubscriptionManager, TransactionPretty},
};
use anyhow::{anyhow, ensure};
use futures::{FutureExt, SinkExt, StreamExt};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex as SyncMutex},
    time::Instant,
};
use tokio::sync::{mpsc, oneshot, Mutex};
use yellowstone_grpc_client::{GeyserStream, SubscribeRequestSink};
use yellowstone_grpc_proto::geyser::{
    subscribe_update::UpdateOneof, CommitmentLevel, CuckooFilter, SubscribeRequest,
    SubscribeRequestFilterAccountsFilter, SubscribeRequestFilterBlocksMeta,
    SubscribeRequestFilterSlots, SubscribeRequestPing, SubscribeUpdateSlot,
};

#[derive(Debug, Clone, Default)]
pub struct TransactionFilter {
    pub account_include: Vec<String>,
    pub account_exclude: Vec<String>,
    pub account_required: Vec<String>,
}
#[derive(Debug, Clone, Default)]
pub struct AccountFilter {
    pub account: Vec<String>,
    pub owner: Vec<String>,
    pub filters: Vec<SubscribeRequestFilterAccountsFilter>,
    pub cuckoo_accounts_filter: Option<CuckooFilter>,
}

#[derive(Clone, Debug)]
pub struct SubscriptionRequest {
    pub plan: ParsePlan,
    pub transactions: Vec<TransactionFilter>,
    pub accounts: Vec<AccountFilter>,
    pub slots: Option<SubscribeRequestFilterSlots>,
    pub blocks_meta: bool,
    pub include_failed_transactions: bool,
    pub commitment: CommitmentLevel,
}
impl SubscriptionRequest {
    pub fn new(plan: ParsePlan) -> Self {
        Self {
            plan,
            transactions: Vec::new(),
            accounts: Vec::new(),
            slots: None,
            blocks_meta: false,
            include_failed_transactions: false,
            commitment: CommitmentLevel::Processed,
        }
    }
    fn wire_request(&self, manager: &SubscriptionManager) -> SubscribeRequest {
        let mut transactions = manager
            .get_subscribe_request_filter(self.transactions.clone(), None)
            .unwrap_or_default();
        if self.include_failed_transactions {
            for filter in transactions.values_mut() {
                filter.failed = None;
            }
        }
        SubscribeRequest {
            transactions,
            accounts: manager
                .subscribe_with_account_request(self.accounts.clone(), None)
                .unwrap_or_default(),
            slots: self
                .slots
                .clone()
                .map(|filter| HashMap::from([("slots".into(), filter)]))
                .unwrap_or_default(),
            blocks_meta: if self.blocks_meta {
                HashMap::from([("blocks".into(), SubscribeRequestFilterBlocksMeta {})])
            } else {
                HashMap::new()
            },
            commitment: Some(self.commitment as i32),
            ..SubscribeRequest::default()
        }
    }
}

/// Synchronous borrowed delivery. Keep callbacks short; use `subscribe_queued` for I/O.
#[derive(Clone, Copy, Debug)]
pub enum StreamEvent<'a> {
    Transaction(TxView<'a>),
    Account(AccountView<'a>),
    BlockMeta(&'a BlockMetaEvent),
    Slot(&'a SubscribeUpdateSlot),
}

struct Control {
    request: SubscriptionRequest,
    wire: SubscribeRequest,
    ack: oneshot::Sender<Result<(), String>>,
}

#[derive(Clone)]
pub struct YellowstoneGrpc {
    pub endpoint: String,
    pub x_token: Option<String>,
    pub config: StreamClientConfig,
    pub subscription_manager: SubscriptionManager,
    pub subscription_handle: Arc<Mutex<Option<SubscriptionHandle>>>,
    control: Arc<Mutex<Option<mpsc::Sender<Control>>>>,
    metrics: Arc<SyncMutex<PerformanceMetrics>>,
    failure: Arc<SyncMutex<Option<String>>>,
}
impl YellowstoneGrpc {
    pub fn new(endpoint: String, x_token: Option<String>) -> AnyResult<Self> {
        Self::new_with_config(endpoint, x_token, StreamClientConfig::default())
    }
    pub fn new_with_config(
        endpoint: String,
        x_token: Option<String>,
        config: StreamClientConfig,
    ) -> AnyResult<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        Ok(Self {
            subscription_manager: SubscriptionManager::new(
                endpoint.clone(),
                x_token.clone(),
                config.clone(),
            ),
            endpoint,
            x_token,
            config,
            subscription_handle: Arc::new(Mutex::new(None)),
            control: Arc::new(Mutex::new(None)),
            metrics: Arc::new(SyncMutex::new(PerformanceMetrics::default())),
            failure: Arc::new(SyncMutex::new(None)),
        })
    }
    pub fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics.lock().unwrap().clone()
    }
    pub fn print_metrics(&self) {
        log::info!("stream metrics: {:?}", self.get_metrics());
    }
    pub fn last_error(&self) -> Option<String> {
        self.failure.lock().unwrap().clone()
    }
    pub fn get_config(&self) -> &StreamClientConfig {
        &self.config
    }

    pub async fn subscribe<F>(&self, request: SubscriptionRequest, callback: F) -> AnyResult<()>
    where
        F: for<'a> FnMut(StreamEvent<'a>) + Send + 'static,
    {
        self.start(request, BorrowedDelivery(callback)).await
    }
    pub async fn subscribe_queued(
        &self,
        request: SubscriptionRequest,
        queue: QueueConfig,
    ) -> AnyResult<QueuedStream> {
        let (sender, stream) = QueueSender::channel(queue)?;
        self.start(request, sender).await?;
        Ok(stream)
    }
    async fn start<D: Delivery>(
        &self,
        request: SubscriptionRequest,
        mut delivery: D,
    ) -> AnyResult<()> {
        let mut handle = self.subscription_handle.lock().await;
        ensure!(
            handle.as_ref().is_none_or(SubscriptionHandle::is_finished),
            "already subscribed; use update_subscription"
        );
        let wire = request.wire_request(&self.subscription_manager);
        let mut client = self.subscription_manager.connect().await?;
        let (sink, stream) = client.subscribe_with_request(Some(wire)).await?;
        let (control, rx) = mpsc::channel(32);
        let (shutdown, shutdown_rx) = oneshot::channel();
        let metrics = self.metrics.clone();
        let failure = self.failure.clone();
        *failure.lock().unwrap() = None;
        let enabled = self.config.enable_metrics;
        let mut control_guard = self.control.lock().await;
        let task = tokio::spawn(async move {
            let mut delta = PerformanceMetrics::default();
            let operation = async {
                if enabled {
                    run::<true, D>(
                        sink,
                        stream,
                        rx,
                        shutdown_rx,
                        request,
                        &mut delivery,
                        &metrics,
                        &mut delta,
                    )
                    .await
                } else {
                    run::<false, D>(
                        sink,
                        stream,
                        rx,
                        shutdown_rx,
                        request,
                        &mut delivery,
                        &metrics,
                        &mut delta,
                    )
                    .await
                }
            };
            let result = std::panic::AssertUnwindSafe(operation)
                .catch_unwind()
                .await
                .unwrap_or_else(|panic| {
                    let message = panic
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| panic.downcast_ref::<&str>().copied())
                        .unwrap_or("unknown panic");
                    Err(anyhow!("subscription worker panicked: {message}"))
                });
            if enabled {
                metrics.lock().unwrap().merge(&mut delta);
            }
            if let Err(error) = &result {
                let error = format!("{error:#}");
                log::error!("subscription stopped: {error}");
                delivery.fail(&error);
                *failure.lock().unwrap() = Some(error);
            }
            result
        });
        *control_guard = Some(control);
        *handle = Some(SubscriptionHandle::new(task, shutdown));
        Ok(())
    }
    /// Apply a complete plan between messages and send its wire filters on the same stream.
    /// The acknowledgement confirms the send, not a server-side transaction boundary.
    pub async fn update_subscription(&self, request: SubscriptionRequest) -> AnyResult<()> {
        let sender =
            self.control.lock().await.clone().ok_or_else(|| anyhow!("no active subscription"))?;
        let wire = request.wire_request(&self.subscription_manager);
        let (ack, response) = oneshot::channel();
        sender
            .send(Control { request, wire, ack })
            .await
            .map_err(|_| anyhow!("subscription ended"))?;
        response
            .await
            .map_err(|_| anyhow!("subscription ended before update"))?
            .map_err(anyhow::Error::msg)
    }
    /// Close production and wait for the worker. Queued consumers may drain accepted items.
    pub async fn stop(&self) -> AnyResult<()> {
        let mut guard = self.subscription_handle.lock().await;
        *self.control.lock().await = None;
        if let Some(handle) = guard.take() {
            handle.shutdown().await?;
        }
        Ok(())
    }
}

trait Delivery: Send + 'static {
    const TIMED: bool;
    fn closed(&mut self) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
    fn transaction(
        &mut self,
        parser: &mut TxParser,
        frame: TxFrame,
        plan: &ParsePlan,
        received: Option<Instant>,
    ) -> anyhow::Result<usize>;
    fn account(
        &mut self,
        frame: AccountFrame,
        plan: &ParsePlan,
        received: Option<Instant>,
    ) -> anyhow::Result<usize>;
    fn block(&mut self, block: BlockMetaEvent, received: Option<Instant>) -> anyhow::Result<()>;
    fn slot(&mut self, slot: SubscribeUpdateSlot, received: Option<Instant>) -> anyhow::Result<()>;
    fn fail(&self, _error: &str) {}
}
struct BorrowedDelivery<F>(F);
impl<F> Delivery for BorrowedDelivery<F>
where
    F: for<'a> FnMut(StreamEvent<'a>) + Send + 'static,
{
    const TIMED: bool = false;
    fn closed(&mut self) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        std::future::pending()
    }
    fn transaction(
        &mut self,
        parser: &mut TxParser,
        frame: TxFrame,
        plan: &ParsePlan,
        _: Option<Instant>,
    ) -> anyhow::Result<usize> {
        let mut count = 0;
        parser.visit(&frame, plan, |view| {
            count = view.events.len();
            self.0(StreamEvent::Transaction(view));
        })?;
        Ok(count)
    }
    fn account(
        &mut self,
        frame: AccountFrame,
        plan: &ParsePlan,
        _: Option<Instant>,
    ) -> anyhow::Result<usize> {
        if let Some(view) = frame.view(plan) {
            self.0(StreamEvent::Account(view));
            return Ok(1);
        }
        Ok(0)
    }
    fn block(&mut self, block: BlockMetaEvent, _: Option<Instant>) -> anyhow::Result<()> {
        self.0(StreamEvent::BlockMeta(&block));
        Ok(())
    }
    fn slot(&mut self, slot: SubscribeUpdateSlot, _: Option<Instant>) -> anyhow::Result<()> {
        self.0(StreamEvent::Slot(&slot));
        Ok(())
    }
}
impl Delivery for QueueSender {
    const TIMED: bool = true;
    fn closed(&mut self) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        QueueSender::closed(self)
    }
    fn transaction(
        &mut self,
        parser: &mut TxParser,
        frame: TxFrame,
        plan: &ParsePlan,
        received: Option<Instant>,
    ) -> anyhow::Result<usize> {
        if let Some(batch) = parser.parse_owned(frame, plan)? {
            let count = batch.events.len();
            self.send(
                OwnedStreamEvent::Transaction(batch),
                received.expect("queued delivery timestamp"),
            )?;
            return Ok(count);
        }
        Ok(0)
    }
    fn account(
        &mut self,
        frame: AccountFrame,
        plan: &ParsePlan,
        received: Option<Instant>,
    ) -> anyhow::Result<usize> {
        if frame.view(plan).is_some() {
            self.send(
                OwnedStreamEvent::Account(frame),
                received.expect("queued delivery timestamp"),
            )?;
            return Ok(1);
        }
        Ok(0)
    }
    fn block(&mut self, block: BlockMetaEvent, received: Option<Instant>) -> anyhow::Result<()> {
        self.send(OwnedStreamEvent::BlockMeta(block), received.expect("queued delivery timestamp"))
    }
    fn slot(&mut self, slot: SubscribeUpdateSlot, received: Option<Instant>) -> anyhow::Result<()> {
        self.send(OwnedStreamEvent::Slot(slot), received.expect("queued delivery timestamp"))
    }
    fn fail(&self, error: &str) {
        QueueSender::fail(self, error);
    }
}

#[allow(clippy::too_many_arguments)]
async fn run<const METRICS: bool, D: Delivery>(
    mut sink: SubscribeRequestSink,
    mut stream: GeyserStream,
    mut controls: mpsc::Receiver<Control>,
    mut shutdown: oneshot::Receiver<()>,
    mut request: SubscriptionRequest,
    delivery: &mut D,
    metrics: &SyncMutex<PerformanceMetrics>,
    delta: &mut PerformanceMetrics,
) -> anyhow::Result<()> {
    let mut parser = TxParser::default();
    let mut flush = METRICS.then(|| {
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        timer
    });
    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown => return Ok(()),
            result = delivery.closed() => return result,
            _ = async { flush.as_mut().unwrap().tick().await }, if METRICS => metrics.lock().unwrap().merge(delta),
            Some(control) = controls.recv() => {
                match sink.send(control.wire).await {
                    Ok(()) => { request = control.request; let _ = control.ack.send(Ok(())); }
                    Err(error) => { let _ = control.ack.send(Err(error.to_string())); return Err(error.into()); }
                }
            }
            message = stream.next() => {
                let message = message.ok_or_else(|| anyhow!("gRPC stream ended"))??;
                let received = (METRICS || D::TIMED).then(Instant::now);
                let mut count = 0;
                match message.update_oneof {
                    Some(UpdateOneof::Transaction(update)) => {
                        // Yellowstone created_at is a transport timestamp, not the block timestamp.
                        let frame = TxFrame::try_from(TransactionPretty::try_from((update, None))?)?;
                        count = delivery.transaction(&mut parser, frame, &request.plan, received)?;
                        if METRICS { delta.transactions += 1; }
                    }
                    Some(UpdateOneof::Account(update)) => {
                        count = delivery.account(AccountFrame::try_from(update)?, &request.plan, received)?;
                        if METRICS { delta.accounts += 1; }
                    }
                    Some(UpdateOneof::BlockMeta(update)) if request.blocks_meta => {
                        delivery.block(BlockMetaEvent { slot: update.slot, block_hash: update.blockhash,
                            block_time_ms: update.block_time.map(|t| t.timestamp.saturating_mul(1000)),
                            recv_us: crate::streaming::event_parser::common::high_performance_clock::get_high_perf_clock() }, received)?;
                        if METRICS { delta.blocks += 1; }
                    }
                    Some(UpdateOneof::Slot(slot)) if request.slots.is_some() => delivery.slot(slot, received)?,
                    Some(UpdateOneof::Ping(_)) => {
                        sink.send(SubscribeRequest { ping: Some(SubscribeRequestPing { id: 1 }), ..SubscribeRequest::default() }).await?;
                        continue;
                    }
                    _ => continue,
                }
                if METRICS {
                    let elapsed = received.unwrap().elapsed().as_micros().min(u64::MAX as u128) as u64;
                    delta.events += count as u64; delta.processing_us += elapsed;
                    delta.max_processing_us = delta.max_processing_us.max(elapsed);
                    if delta.transactions + delta.accounts + delta.blocks >= 128 {
                        metrics.lock().unwrap().merge(delta);
                    }
                }
            }
        }
    }
}
