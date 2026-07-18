//! Persistent, cross-platform runtime diagnostics.
//!
//! `sysinfo` refreshes are synchronous and some values (notably CPU usage)
//! require consecutive samples. The sampler therefore owns one long-lived
//! collector and performs refreshes on Tokio's blocking pool.

use serde::Serialize;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDiagnostics {
    pub system: SystemDiagnostics,
    pub process: ProcessDiagnostics,
    pub disks: DiskDiagnostics,
    pub network: NetworkDiagnostics,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemDiagnostics {
    pub uptime_seconds: u64,
    pub total_memory_bytes: u64,
    pub available_memory_bytes: u64,
    pub cpu_count: usize,
    pub global_cpu_usage_percent: f32,
    pub load_average_one: f64,
    pub load_average_five: f64,
    pub load_average_fifteen: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessDiagnostics {
    pub pid: u32,
    pub memory_bytes: Option<u64>,
    pub virtual_memory_bytes: Option<u64>,
    pub cpu_usage_percent: Option<f32>,
    pub runtime_seconds: Option<u64>,
    pub thread_count: Option<usize>,
    pub open_files: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskDiagnostics {
    pub filesystems: usize,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkDiagnostics {
    pub interfaces: usize,
    pub total_received_bytes: u64,
    pub total_transmitted_bytes: u64,
    pub receive_errors: u64,
    pub transmit_errors: u64,
    pub maximum_mtu: u64,
}

struct DiagnosticsCollector {
    system: sysinfo::System,
    disks: sysinfo::Disks,
    networks: sysinfo::Networks,
}

impl DiagnosticsCollector {
    fn new() -> Self {
        Self {
            system: sysinfo::System::new_all(),
            disks: sysinfo::Disks::new_with_refreshed_list(),
            networks: sysinfo::Networks::new_with_refreshed_list(),
        }
    }

    fn refresh(&mut self) -> RuntimeDiagnostics {
        self.system.refresh_all();
        self.disks.refresh(true);
        self.networks.refresh(true);

        let load = sysinfo::System::load_average();
        let pid = sysinfo::get_current_pid().ok();
        let process = pid.and_then(|pid| self.system.process(pid));

        let disk_total = self
            .disks
            .iter()
            .fold((0_u64, 0_u64), |(total, available), disk| {
                (
                    total.saturating_add(disk.total_space()),
                    available.saturating_add(disk.available_space()),
                )
            });
        let network_total = self.networks.iter().fold(
            (0_u64, 0_u64, 0_u64, 0_u64, 0_u64),
            |(received, transmitted, receive_errors, transmit_errors, mtu), (_, network)| {
                (
                    received.saturating_add(network.total_received()),
                    transmitted.saturating_add(network.total_transmitted()),
                    receive_errors.saturating_add(network.total_errors_on_received()),
                    transmit_errors.saturating_add(network.total_errors_on_transmitted()),
                    mtu.max(network.mtu()),
                )
            },
        );

        RuntimeDiagnostics {
            system: SystemDiagnostics {
                uptime_seconds: sysinfo::System::uptime(),
                total_memory_bytes: self.system.total_memory(),
                available_memory_bytes: self.system.available_memory(),
                cpu_count: self.system.cpus().len(),
                global_cpu_usage_percent: self.system.global_cpu_usage(),
                load_average_one: load.one,
                load_average_five: load.five,
                load_average_fifteen: load.fifteen,
            },
            process: ProcessDiagnostics {
                pid: pid
                    .map(|value| value.as_u32())
                    .unwrap_or_else(std::process::id),
                memory_bytes: process.map(sysinfo::Process::memory),
                virtual_memory_bytes: process.map(sysinfo::Process::virtual_memory),
                cpu_usage_percent: process.map(sysinfo::Process::cpu_usage),
                runtime_seconds: process.map(sysinfo::Process::run_time),
                thread_count: process.and_then(|value| value.tasks().map(|tasks| tasks.len())),
                open_files: process.and_then(sysinfo::Process::open_files),
            },
            disks: DiskDiagnostics {
                filesystems: self.disks.len(),
                total_bytes: disk_total.0,
                available_bytes: disk_total.1,
            },
            network: NetworkDiagnostics {
                interfaces: self.networks.len(),
                total_received_bytes: network_total.0,
                total_transmitted_bytes: network_total.1,
                receive_errors: network_total.2,
                transmit_errors: network_total.3,
                maximum_mtu: network_total.4,
            },
        }
    }
}

#[derive(Clone)]
pub struct SystemDiagnosticsSampler {
    collector: Arc<Mutex<DiagnosticsCollector>>,
}

impl SystemDiagnosticsSampler {
    pub fn new() -> Self {
        Self {
            collector: Arc::new(Mutex::new(DiagnosticsCollector::new())),
        }
    }

    pub async fn snapshot(&self) -> Result<RuntimeDiagnostics, String> {
        let collector = Arc::clone(&self.collector);
        tokio::task::spawn_blocking(move || {
            collector
                .lock()
                .map_err(|_| "runtime diagnostics sampler lock was poisoned".to_string())
                .map(|mut collector| collector.refresh())
        })
        .await
        .map_err(|error| format!("runtime diagnostics task failed: {error}"))?
    }
}

impl Default for SystemDiagnosticsSampler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sampler_reports_consistent_numeric_invariants() {
        let sampler = SystemDiagnosticsSampler::new();
        let first = sampler.snapshot().await.expect("first diagnostics sample");
        let second = sampler.snapshot().await.expect("second diagnostics sample");

        assert_eq!(first.process.pid, std::process::id());
        assert_eq!(second.process.pid, std::process::id());
        assert!(second.system.available_memory_bytes <= second.system.total_memory_bytes);
        assert!(second.disks.available_bytes <= second.disks.total_bytes);
    }
}
