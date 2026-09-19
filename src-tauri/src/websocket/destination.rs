use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use reqwest::Url;

use crate::{
    error::{AoneError, AoneResult},
    network_policy::{DestinationScope, classify_destination, is_cloud_metadata_hostname},
};

const DNS_TIMEOUT_SECONDS: u64 = 5;

#[derive(Debug)]
pub(super) struct ValidatedDestination {
    pub(super) addresses: Vec<SocketAddr>,
}

impl ValidatedDestination {
    pub(super) async fn resolve(url: &Url) -> AoneResult<Self> {
        let host = url
            .host_str()
            .ok_or_else(|| invalid("WebSocket URL must include a host".into()))?
            .to_ascii_lowercase();
        if is_cloud_metadata_hostname(&host) {
            return Err(invalid(
                "cloud metadata WebSocket destinations are prohibited".into(),
            ));
        }
        let port = url
            .port_or_known_default()
            .ok_or_else(|| invalid("WebSocket URL must include a valid port".into()))?;
        let literal_host = host
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .unwrap_or(&host);

        let mut addresses = match literal_host.parse::<IpAddr>() {
            Ok(ip) => vec![SocketAddr::new(ip, port)],
            Err(_) => tokio::time::timeout(
                Duration::from_secs(DNS_TIMEOUT_SECONDS),
                tokio::net::lookup_host((literal_host, port)),
            )
            .await
            .map_err(|_| AoneError::Task("WebSocket destination DNS lookup timed out".into()))?
            .map_err(|_| AoneError::Task("could not resolve WebSocket destination".into()))?
            .collect::<Vec<_>>(),
        };
        addresses.sort_unstable();
        addresses.dedup();
        if addresses.is_empty() {
            return Err(invalid(
                "WebSocket destination resolved to no addresses".into(),
            ));
        }

        for address in &addresses {
            match classify_destination(address.ip()) {
                DestinationScope::Public
                | DestinationScope::Private
                | DestinationScope::Loopback => {}
                DestinationScope::Prohibited(reason) => {
                    return Err(invalid(format!(
                        "WebSocket destination is prohibited: {reason}"
                    )));
                }
            }
        }
        Ok(Self { addresses })
    }
}

pub(super) fn literal_private_warning(url: &Url) -> bool {
    url.host_str()
        .map(str::to_ascii_lowercase)
        .and_then(|host| {
            host.strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
                .unwrap_or(&host)
                .parse::<IpAddr>()
                .ok()
        })
        .is_some_and(|ip| {
            matches!(
                classify_destination(ip),
                DestinationScope::Private | DestinationScope::Loopback
            )
        })
}

fn invalid(message: String) -> AoneError {
    AoneError::InvalidRequest(message)
}
