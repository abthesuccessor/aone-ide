use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
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
    pub(super) host: String,
    pub(super) addresses: Vec<SocketAddr>,
    pub(super) was_dns_resolved: bool,
}

impl ValidatedDestination {
    pub(super) async fn resolve(url: &Url) -> AoneResult<Self> {
        let host = url
            .host_str()
            .ok_or_else(|| AoneError::InvalidRequest("request URL must include a host".into()))?
            .to_ascii_lowercase();
        if is_cloud_metadata_hostname(&host) {
            return Err(AoneError::InvalidRequest(
                "cloud metadata destinations are prohibited".into(),
            ));
        }
        let port = url.port_or_known_default().ok_or_else(|| {
            AoneError::InvalidRequest("request URL must include a valid port".into())
        })?;

        // `url::Url::host_str` retains brackets for IPv6 literals. Strip only
        // that canonical wrapper before parsing; DNS names cannot contain it.
        let literal_host = host
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .unwrap_or(&host);
        let (mut addresses, was_dns_resolved) = match literal_host.parse::<IpAddr>() {
            Ok(ip) => (vec![SocketAddr::new(ip, port)], false),
            Err(_) => {
                let lookup_host = host.clone();
                let resolution = tokio::task::spawn_blocking(move || {
                    (lookup_host.as_str(), port)
                        .to_socket_addrs()
                        .map(|addresses| addresses.collect::<Vec<_>>())
                });
                let addresses =
                    tokio::time::timeout(Duration::from_secs(DNS_TIMEOUT_SECONDS), resolution)
                        .await
                        .map_err(|_| AoneError::Task("destination DNS lookup timed out".into()))?
                        .map_err(|_| AoneError::Task("destination DNS lookup failed".into()))?
                        .map_err(|_| {
                            AoneError::Task("could not resolve the HTTP destination".into())
                        })?;
                (addresses, true)
            }
        };

        addresses.sort_unstable();
        addresses.dedup();
        if addresses.is_empty() {
            return Err(AoneError::InvalidRequest(
                "HTTP destination resolved to no addresses".into(),
            ));
        }

        for address in &addresses {
            match classify_destination(address.ip()) {
                DestinationScope::Public
                | DestinationScope::Private
                | DestinationScope::Loopback => {}
                DestinationScope::Prohibited(reason) => {
                    return Err(AoneError::InvalidRequest(format!(
                        "HTTP destination is prohibited: {reason}"
                    )));
                }
            }
        }

        Ok(Self {
            host,
            addresses,
            was_dns_resolved,
        })
    }

    pub(super) fn is_entirely_local_or_private(&self) -> bool {
        self.addresses.iter().all(|address| {
            matches!(
                classify_destination(address.ip()),
                DestinationScope::Private | DestinationScope::Loopback
            )
        })
    }
}

fn literal_destination_scope(url: &Url) -> Option<DestinationScope> {
    url.host_str()
        .map(str::to_ascii_lowercase)
        .and_then(|host| {
            host.strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
                .unwrap_or(&host)
                .parse::<IpAddr>()
                .ok()
        })
        .map(classify_destination)
}

pub(super) fn literal_loopback(url: &Url) -> bool {
    matches!(
        literal_destination_scope(url),
        Some(DestinationScope::Loopback)
    )
}

pub(super) fn literal_private_warning(url: &Url) -> bool {
    matches!(
        literal_destination_scope(url),
        Some(DestinationScope::Private | DestinationScope::Loopback)
    )
}
