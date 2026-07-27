use super::{decode_datagram, Node1TxStreamConfig};
use crate::streaming::{
    event_parser::common::high_performance_clock::get_high_perf_clock,
    signal::PreExecutionTxEnvelope,
};
use anyhow::Context;
use socket2::{Domain, Protocol, Socket, Type};
use solana_sdk::signature::Signature;
use std::{
    collections::{HashMap, VecDeque},
    time::Instant,
};
use tokio::{net::UdpSocket, sync::mpsc};
use tracing::{debug, trace, warn};

/// Node1 data-plane receiver. Control-plane subscription remains Dashboard-owned.
#[derive(Clone)]
pub struct Node1UdpSignalAdapter {
    config: Node1TxStreamConfig,
    output: mpsc::Sender<PreExecutionTxEnvelope>,
}

impl Node1UdpSignalAdapter {
    pub fn new(
        config: Node1TxStreamConfig,
        output: mpsc::Sender<PreExecutionTxEnvelope>,
    ) -> anyhow::Result<Self> {
        config.validate()?;
        Ok(Self { config, output })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let socket = self.bind_socket()?;
        debug!(
            bind_addr = %self.config.bind_addr,
            allowed_source_count = self.config.allowed_source_cidrs.len(),
            max_datagram_bytes = self.config.max_datagram_bytes,
            socket_receive_buffer_bytes = self.config.socket_receive_buffer_bytes,
            "Node1 UDP receiver bound"
        );

        let buffer_len = self
            .config
            .max_datagram_bytes
            .checked_add(1)
            .context("Node1 datagram buffer length overflow")?;
        let mut buffer = vec![0u8; buffer_len];
        let mut dedupe = SignatureDedupe::new(self.config.dedupe_ttl, self.config.dedupe_capacity);

        loop {
            let (datagram_bytes, sender_addr) =
                socket.recv_from(&mut buffer).await.with_context(|| {
                    format!("Node1 UDP receive failed: bind_addr={}", self.config.bind_addr)
                })?;
            let recv_us = get_high_perf_clock();

            if !self.config.sender_allowed(sender_addr) {
                warn!(
                    bind_addr = %self.config.bind_addr,
                    %sender_addr,
                    datagram_bytes,
                    drop_reason = "source_not_allowed",
                    "Node1 UDP packet rejected"
                );
                continue;
            }
            if datagram_bytes > self.config.max_datagram_bytes {
                warn!(
                    bind_addr = %self.config.bind_addr,
                    %sender_addr,
                    datagram_bytes,
                    max_datagram_bytes = self.config.max_datagram_bytes,
                    drop_reason = "datagram_too_large",
                    "Node1 UDP packet rejected"
                );
                continue;
            }

            let decode_started = Instant::now();
            let envelope = match decode_datagram(&buffer[..datagram_bytes], recv_us) {
                Ok(envelope) => envelope,
                Err(error) => {
                    warn!(
                        bind_addr = %self.config.bind_addr,
                        %sender_addr,
                        datagram_bytes,
                        error = %error,
                        drop_reason = "decode_error",
                        "Node1 UDP packet rejected"
                    );
                    continue;
                }
            };
            let decode_us = decode_started.elapsed().as_micros() as u64;

            if !dedupe.insert(envelope.signature, Instant::now()) {
                debug!(
                    %sender_addr,
                    source_slot = envelope.slot,
                    signature = %envelope.signature,
                    datagram_bytes,
                    decode_us,
                    drop_reason = "duplicate",
                    "Node1 duplicate transaction dropped"
                );
                continue;
            }

            let signature = envelope.signature;
            let source_slot = envelope.slot;
            match self.output.try_send(envelope) {
                Ok(()) => trace!(
                    %sender_addr,
                    source_slot,
                    %signature,
                    datagram_bytes,
                    transaction_bytes = datagram_bytes.saturating_sub(8),
                    decode_us,
                    queue_result = "accepted",
                    "Node1 transaction normalized"
                ),
                Err(mpsc::error::TrySendError::Full(_)) => warn!(
                    %sender_addr,
                    source_slot,
                    %signature,
                    datagram_bytes,
                    decode_us,
                    queue_result = "full",
                    drop_reason = "receive_queue_full",
                    "Node1 transaction dropped"
                ),
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    anyhow::bail!("Node1 output queue closed: bind_addr={}", self.config.bind_addr);
                }
            }
        }
    }

    fn bind_socket(&self) -> anyhow::Result<UdpSocket> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
            .context("create Node1 IPv4 UDP socket")?;
        socket.set_reuse_address(true).context("set Node1 UDP SO_REUSEADDR")?;
        socket.set_recv_buffer_size(self.config.socket_receive_buffer_bytes).with_context(
            || {
                format!(
                    "set Node1 UDP receive buffer: bytes={}",
                    self.config.socket_receive_buffer_bytes
                )
            },
        )?;
        socket
            .bind(&self.config.bind_addr.into())
            .with_context(|| format!("bind Node1 UDP socket: {}", self.config.bind_addr))?;
        socket.set_nonblocking(true).context("set Node1 UDP socket nonblocking")?;
        let std_socket: std::net::UdpSocket = socket.into();
        UdpSocket::from_std(std_socket).context("register Node1 UDP socket with Tokio")
    }
}

struct SignatureDedupe {
    ttl: std::time::Duration,
    capacity: usize,
    seen: HashMap<Signature, Instant>,
    order: VecDeque<(Instant, Signature)>,
}

impl SignatureDedupe {
    fn new(ttl: std::time::Duration, capacity: usize) -> Self {
        Self {
            ttl,
            capacity,
            seen: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
        }
    }

    fn insert(&mut self, signature: Signature, now: Instant) -> bool {
        self.evict_expired(now);
        if self.seen.contains_key(&signature) {
            return false;
        }
        while self.seen.len() >= self.capacity {
            self.evict_oldest();
        }
        self.seen.insert(signature, now);
        self.order.push_back((now, signature));
        true
    }

    fn evict_expired(&mut self, now: Instant) {
        while self
            .order
            .front()
            .is_some_and(|(seen_at, _)| now.duration_since(*seen_at) >= self.ttl)
        {
            self.evict_oldest();
        }
    }

    fn evict_oldest(&mut self) {
        if let Some((seen_at, signature)) = self.order.pop_front() {
            if self.seen.get(&signature) == Some(&seen_at) {
                self.seen.remove(&signature);
            }
        }
    }
}
