//! LAN discovery of SMB servers via mDNS/DNS-SD (`_smb._tcp`). Synology,
//! macOS, Samba with Avahi and most consumer NAS boxes advertise this.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent};
use serde::Serialize;

use crate::error::AppError;

const SERVICE_TYPE: &str = "_smb._tcp.local.";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredServer {
    /// Human-facing instance name, e.g. "DiskStation".
    pub name: String,
    /// Resolvable hostname without trailing dot, e.g. "DiskStation.local".
    pub hostname: String,
    pub addresses: Vec<String>,
    pub port: u16,
}

pub async fn discover(timeout: Duration) -> Result<Vec<DiscoveredServer>, AppError> {
    tokio::task::spawn_blocking(move || discover_blocking(timeout)).await?
}

fn discover_blocking(timeout: Duration) -> Result<Vec<DiscoveredServer>, AppError> {
    let mdns = ServiceDaemon::new()?;
    let receiver = mdns.browse(SERVICE_TYPE)?;
    let deadline = Instant::now() + timeout;
    let mut found: BTreeMap<String, DiscoveredServer> = BTreeMap::new();

    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        match receiver.recv_timeout(deadline - now) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let name = info
                    .get_fullname()
                    .trim_end_matches(SERVICE_TYPE)
                    .trim_end_matches('.')
                    .replace("\\032", " ");
                let hostname = info.get_hostname().trim_end_matches('.').to_string();
                let mut addresses: Vec<String> = info
                    .get_addresses_v4()
                    .into_iter()
                    .map(|a| a.to_string())
                    .collect();
                if addresses.is_empty() {
                    addresses = info
                        .get_addresses()
                        .iter()
                        .map(|a| a.to_ip_addr().to_string())
                        .collect();
                }
                addresses.sort();
                let entry = found
                    .entry(hostname.to_lowercase())
                    .or_insert_with(|| DiscoveredServer {
                        name: name.clone(),
                        hostname: hostname.clone(),
                        addresses: Vec::new(),
                        port: info.get_port(),
                    });
                for a in addresses {
                    if !entry.addresses.contains(&a) {
                        entry.addresses.push(a);
                    }
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }

    let _ = mdns.stop_browse(SERVICE_TYPE);
    let _ = mdns.shutdown();
    Ok(found.into_values().collect())
}
