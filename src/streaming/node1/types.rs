use crate::streaming::signal::{OrderGuarantee, TxSignalCapabilities};
use std::{
    fmt,
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

pub const NODE1_CAPABILITIES: TxSignalCapabilities = TxSignalCapabilities {
    pre_execution: true,
    raw_versioned_transaction: true,
    slot: true,
    parent_slot: false,
    global_order: OrderGuarantee::NotRequired,
    provider_resolved_alt: false,
    reconnect_cursor: false,
    execution_meta: false,
};

/// Parsed IP network used to reject unexpected UDP senders before decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IpCidr {
    network: IpAddr,
    prefix_len: u8,
}

impl IpCidr {
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(network), IpAddr::V4(ip)) => {
                let prefix = u32::from(self.prefix_len);
                let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
                (u32::from(network) & mask) == (u32::from(ip) & mask)
            }
            (IpAddr::V6(network), IpAddr::V6(ip)) => {
                let prefix = u32::from(self.prefix_len);
                let mask = if prefix == 0 { 0 } else { u128::MAX << (128 - prefix) };
                (u128::from(network) & mask) == (u128::from(ip) & mask)
            }
            _ => false,
        }
    }
}

impl FromStr for IpCidr {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (network, prefix_len) = match value.split_once('/') {
            Some((network, prefix)) => {
                let network = network
                    .parse::<IpAddr>()
                    .map_err(|error| anyhow::anyhow!("invalid IP in CIDR {value}: {error}"))?;
                let prefix_len = prefix
                    .parse::<u8>()
                    .map_err(|error| anyhow::anyhow!("invalid prefix in CIDR {value}: {error}"))?;
                (network, prefix_len)
            }
            None => {
                let network = value
                    .parse::<IpAddr>()
                    .map_err(|error| anyhow::anyhow!("invalid sender IP {value}: {error}"))?;
                let prefix_len = if network.is_ipv4() { 32 } else { 128 };
                (network, prefix_len)
            }
        };
        let max_prefix = if network.is_ipv4() { 32 } else { 128 };
        if prefix_len > max_prefix {
            anyhow::bail!(
                "CIDR prefix out of range: value={value}, prefix={prefix_len}, max={max_prefix}"
            );
        }
        Ok(Self { network, prefix_len })
    }
}

impl fmt::Display for IpCidr {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.network, self.prefix_len)
    }
}

/// Validated runtime configuration for one Node1 UDP receiver.
#[derive(Clone, Debug)]
pub struct Node1TxStreamConfig {
    pub bind_addr: SocketAddr,
    pub allowed_source_cidrs: Arc<[IpCidr]>,
    pub max_datagram_bytes: usize,
    pub socket_receive_buffer_bytes: usize,
    pub dedupe_ttl: Duration,
    pub dedupe_capacity: usize,
}

impl Node1TxStreamConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if !self.bind_addr.is_ipv4() {
            anyhow::bail!("Node1 UDP requires an IPv4 bind address: {}", self.bind_addr);
        }
        if self.max_datagram_bytes <= 8 || self.max_datagram_bytes > u16::MAX as usize {
            anyhow::bail!(
                "Node1 max_datagram_bytes must be in 9..={}: value={}",
                u16::MAX,
                self.max_datagram_bytes
            );
        }
        if self.socket_receive_buffer_bytes == 0 {
            anyhow::bail!("Node1 socket_receive_buffer_bytes must be greater than zero");
        }
        if self.dedupe_ttl.is_zero() {
            anyhow::bail!("Node1 dedupe_ttl must be greater than zero");
        }
        if self.dedupe_capacity == 0 {
            anyhow::bail!("Node1 dedupe_capacity must be greater than zero");
        }
        Ok(())
    }

    pub(crate) fn sender_allowed(&self, sender: SocketAddr) -> bool {
        self.allowed_source_cidrs.is_empty()
            || self.allowed_source_cidrs.iter().any(|network| network.contains(sender.ip()))
    }
}
