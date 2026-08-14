//! Persistent, cross-platform runtime diagnostics.
//!
//! `sysinfo` refreshes are synchronous and some values (notably CPU usage)
//! require consecutive samples. The sampler therefore owns one long-lived
//! collector and performs refreshes on Tokio's blocking pool.

use serde::Serialize;
#[cfg(feature = "diagnostics")]
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

#[cfg(feature = "diagnostics")]
struct DiagnosticsCollector {
    system: sysinfo::System,
    disks: sysinfo::Disks,
    networks: sysinfo::Networks,
}

#[cfg(feature = "diagnostics")]
impl DiagnosticsCollector {
    fn new() -> Self {
        // `System::new()`, never `new_all()`: that walks the whole process table,
        // and the only process this reports on is our own. On FreeBSD it also
        // means `kinfo_getfile` — the same class of sysinfo call as the disk
        // enumeration below, which faults the same way under QEMU. Every
        // `AppState` builds one of these, including the ones in tests that never
        // take a sample. The cost is that the first sample reports no CPU usage,
        // having no earlier reading to difference against.
        let mut system = sysinfo::System::new();
        // The CPU list is fixed for the life of the process and is not refreshed
        // by `refresh_cpu_usage`, so take it once here.
        system.refresh_cpu_list(sysinfo::CpuRefreshKind::nothing().with_cpu_usage());

        Self {
            system,
            // sysinfo 0.39 FreeBSD disk enumeration calls getmntinfo and then
            // slice::from_raw_parts with a pointer that fails Rust's alignment
            // UB checks (abort in debug; real UB risk in release). Skip disks
            // on FreeBSD until upstream is fixed; metrics stay at zeros.
            #[cfg(target_os = "freebsd")]
            disks: sysinfo::Disks::new(),
            #[cfg(not(target_os = "freebsd"))]
            disks: sysinfo::Disks::new_with_refreshed_list(),
            networks: sysinfo::Networks::new_with_refreshed_list(),
        }
    }

    fn refresh(&mut self) -> RuntimeDiagnostics {
        let pid = sysinfo::get_current_pid().ok();

        // Refresh what is actually read below, rather than `refresh_all()`. That
        // walked and stored every process on the host — a couple of megabytes
        // rebuilt on every sample, and `/metrics/json` samples every five seconds
        // while a dashboard is open — to answer questions about one pid.
        self.system.refresh_memory();
        self.system.refresh_cpu_usage();
        if let Some(pid) = pid {
            self.system.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[pid]),
                true,
                sysinfo::ProcessRefreshKind::nothing()
                    .with_memory()
                    .with_cpu()
                    .with_tasks(),
            );
        }
        #[cfg(not(target_os = "freebsd"))]
        self.disks.refresh(true);
        self.networks.refresh(true);

        let load = sysinfo::System::load_average();
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

/// Samples the running system.
///
/// The shape of this type does not depend on the `diagnostics` feature, so
/// `AppState` and the status endpoints are the same either way. Without the
/// feature there is nothing behind it, and [`snapshot`](Self::snapshot) says
/// so — callers already treat a failed sample as "no data available".
#[derive(Clone)]
pub struct SystemDiagnosticsSampler {
    #[cfg(feature = "diagnostics")]
    collector: Arc<Mutex<DiagnosticsCollector>>,
}

impl SystemDiagnosticsSampler {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "diagnostics")]
            collector: Arc::new(Mutex::new(DiagnosticsCollector::new())),
        }
    }

    #[cfg(not(feature = "diagnostics"))]
    pub async fn snapshot(&self) -> Result<RuntimeDiagnostics, String> {
        Err("this build of vuio-core was compiled without the `diagnostics` feature".to_string())
    }

    #[cfg(feature = "diagnostics")]
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

#[cfg(all(test, feature = "diagnostics"))]
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

        // The sampler refreshes only the specifics these fields need, rather than
        // everything sysinfo can collect, so each one is a thing that silently
        // becomes zero or `None` if the wrong refresh is dropped.
        assert!(
            second.system.cpu_count > 0,
            "the CPU list must be populated: refresh_cpu_usage does not build it"
        );
        assert!(
            second.system.total_memory_bytes > 0,
            "system memory must be refreshed"
        );
        assert!(
            second.process.memory_bytes.is_some_and(|bytes| bytes > 0),
            "our own process must still be in the table after a targeted refresh"
        );
        assert!(
            second.process.runtime_seconds.is_some(),
            "our own process must still be in the table after a targeted refresh"
        );
    }
}
