use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sysinfo::{
    Components, CpuRefreshKind, DiskKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind,
    ProcessesToUpdate, RefreshKind, System, UpdateKind, Users,
};
use tf_plugin_api::PluginResult;

const MAX_PROCESSES: usize = 500;

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    #[default]
    Cpu,
    Memory,
    Name,
    Pid,
}

fn default_limit() -> usize {
    100
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// 是否刷新并返回进程列表（进程刷新相对耗时，仅在查看进程时开启）
    #[serde(default)]
    pub processes: bool,
    #[serde(default)]
    pub sort: SortBy,
    #[serde(default)]
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Os {
    pub name: Option<String>,
    pub version: Option<String>,
    pub long_version: Option<String>,
    pub kernel: Option<String>,
    pub hostname: Option<String>,
    pub arch: String,
    pub uptime: u64,
    pub boot_time: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cpu {
    pub brand: String,
    pub vendor: String,
    pub physical_cores: Option<usize>,
    pub logical_cores: usize,
    /// MHz
    pub frequency: u64,
    pub usage: f32,
    pub cores: Vec<f32>,
    /// 1 / 5 / 15 分钟负载；Windows 上为 0
    pub load: [f64; 3],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Disk {
    pub name: String,
    pub mount: String,
    pub file_system: String,
    /// ssd / hdd / unknown
    pub kind: &'static str,
    pub total: u64,
    pub available: u64,
    pub removable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Interface {
    pub name: String,
    pub mac: String,
    pub addresses: Vec<String>,
    /// 字节 / 秒
    pub rx_rate: u64,
    pub tx_rate: u64,
    pub rx_total: u64,
    pub tx_total: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sensor {
    pub label: String,
    pub temperature: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
    pub status: String,
    pub user: Option<String>,
    pub run_time: u64,
    pub exe: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Processes {
    pub total: usize,
    pub matched: usize,
    pub list: Vec<ProcessInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub os: Os,
    pub cpu: Cpu,
    pub memory: Memory,
    pub disks: Vec<Disk>,
    pub networks: Vec<Interface>,
    pub sensors: Vec<Sensor>,
    pub processes: Option<Processes>,
}

struct State {
    system: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    users: Users,
    last_network: Instant,
}

/// 长期持有的采样器：CPU 占用与网络速率都需要两次采样之间的差值
#[derive(Default)]
pub struct Monitor {
    state: Mutex<Option<State>>,
}

fn disk_kind(kind: DiskKind) -> &'static str {
    match kind {
        DiskKind::SSD => "ssd",
        DiskKind::HDD => "hdd",
        _ => "unknown",
    }
}

impl Monitor {
    fn init() -> State {
        let mut system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        // 首次采样后需要间隔一小段时间才能得到有效的 CPU 占用
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        system.refresh_cpu_usage();
        State {
            system,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            users: Users::new_with_refreshed_list(),
            last_network: Instant::now(),
        }
    }

    pub fn snapshot(&self, args: Args) -> PluginResult<Snapshot> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let state = guard.get_or_insert_with(Self::init);

        state.system.refresh_cpu_all();
        state.system.refresh_memory();
        state.disks.refresh(true);
        state.components.refresh(true);
        let elapsed = state.last_network.elapsed().as_secs_f64().max(0.001);
        state.networks.refresh(true);
        state.last_network = Instant::now();

        let system = &state.system;
        let cpus = system.cpus();
        let load = System::load_average();
        let cpu = Cpu {
            brand: cpus
                .first()
                .map(|c| c.brand().trim().to_owned())
                .unwrap_or_default(),
            vendor: cpus
                .first()
                .map(|c| c.vendor_id().to_owned())
                .unwrap_or_default(),
            physical_cores: System::physical_core_count(),
            logical_cores: cpus.len(),
            frequency: cpus.first().map(|c| c.frequency()).unwrap_or(0),
            usage: system.global_cpu_usage(),
            cores: cpus.iter().map(|c| c.cpu_usage()).collect(),
            load: [load.one, load.five, load.fifteen],
        };
        let memory = Memory {
            total: system.total_memory(),
            used: system.used_memory(),
            available: system.available_memory(),
            swap_total: system.total_swap(),
            swap_used: system.used_swap(),
        };

        let mut disks: Vec<Disk> = state
            .disks
            .list()
            .iter()
            .filter(|d| d.total_space() > 0)
            .map(|d| Disk {
                name: d.name().to_string_lossy().into_owned(),
                mount: d.mount_point().to_string_lossy().into_owned(),
                file_system: d.file_system().to_string_lossy().into_owned(),
                kind: disk_kind(d.kind()),
                total: d.total_space(),
                available: d.available_space(),
                removable: d.is_removable(),
            })
            .collect();
        disks.sort_by(|a, b| a.mount.cmp(&b.mount));
        dedup_disks(&mut disks);

        let mut networks: Vec<Interface> = state
            .networks
            .iter()
            .map(|(name, data)| Interface {
                name: name.clone(),
                mac: real_mac(&data.mac_address().to_string()),
                addresses: data
                    .ip_networks()
                    .iter()
                    .map(|ip| format!("{}/{}", ip.addr, ip.prefix))
                    .collect(),
                rx_rate: (data.received() as f64 / elapsed) as u64,
                tx_rate: (data.transmitted() as f64 / elapsed) as u64,
                rx_total: data.total_received(),
                tx_total: data.total_transmitted(),
            })
            .collect();
        tidy_networks(&mut networks);

        let sensors = state
            .components
            .list()
            .iter()
            .filter_map(|c| {
                c.temperature()
                    .filter(|t| t.is_finite() && *t > 0.0)
                    .map(|temperature| Sensor {
                        label: c.label().to_owned(),
                        temperature,
                    })
            })
            .collect();

        let processes = args.processes.then(|| {
            state.system.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing()
                    .with_cpu()
                    .with_memory()
                    .with_user(UpdateKind::OnlyIfNotSet)
                    .with_exe(UpdateKind::OnlyIfNotSet),
            );
            collect_processes(&state.system, &state.users, &args)
        });

        Ok(Snapshot {
            os: Os {
                name: System::name(),
                version: System::os_version(),
                long_version: System::long_os_version(),
                kernel: System::kernel_version(),
                hostname: System::host_name(),
                arch: System::cpu_arch(),
                uptime: System::uptime(),
                boot_time: System::boot_time(),
            },
            cpu,
            memory,
            disks,
            networks,
            sensors,
            processes,
        })
    }
}

fn collect_processes(system: &System, users: &Users, args: &Args) -> Processes {
    let query = args.query.trim().to_lowercase();
    let all = system.processes();
    let mut list: Vec<ProcessInfo> = all
        .values()
        .filter(|p| {
            query.is_empty()
                || p.name().to_string_lossy().to_lowercase().contains(&query)
                || p.pid().to_string() == query
        })
        .map(|p| ProcessInfo {
            pid: p.pid().as_u32(),
            parent: p.parent().map(|pid| pid.as_u32()),
            name: p.name().to_string_lossy().into_owned(),
            cpu: p.cpu_usage(),
            memory: p.memory(),
            status: p.status().to_string(),
            user: p
                .user_id()
                .and_then(|uid| users.get_user_by_id(uid))
                .map(|u| u.name().to_owned()),
            run_time: p.run_time(),
            exe: p.exe().map(|e| e.to_string_lossy().into_owned()),
        })
        .collect();
    let matched = list.len();
    sort_processes(&mut list, args.sort);
    list.truncate(args.limit.clamp(1, MAX_PROCESSES));
    Processes {
        total: all.len(),
        matched,
        list,
    }
}

/// 同一存储容器挂载在多个位置时（如 macOS 的 / 与 /System/Volumes/Data）只保留一个
pub fn dedup_disks(disks: &mut Vec<Disk>) {
    let mut seen = std::collections::HashSet::new();
    disks.retain(|d| seen.insert((d.total, d.available, d.file_system.clone())));
}

/// 系统无法读取时返回的占位 MAC 不展示
pub fn real_mac(mac: &str) -> String {
    match mac {
        "00:00:00:00:00:00" | "02:00:00:00:00:00" => String::new(),
        other => other.to_owned(),
    }
}

fn is_loopback(interface: &Interface) -> bool {
    interface
        .addresses
        .iter()
        .any(|a| a.starts_with("127.") || a.starts_with("::1/"))
}

/// 隐藏没有地址也没有流量的虚拟网卡；有 IPv4 的物理网卡在前，回环放最后
pub fn tidy_networks(networks: &mut Vec<Interface>) {
    networks.retain(|i| !i.addresses.is_empty() || i.rx_total + i.tx_total > 0);
    networks.sort_by(|a, b| {
        let rank = |i: &Interface| {
            let has_v4 = i.addresses.iter().any(|a| !a.contains(':'));
            (
                is_loopback(i),
                !has_v4,
                std::cmp::Reverse(i.rx_total + i.tx_total),
            )
        };
        rank(a).cmp(&rank(b)).then_with(|| a.name.cmp(&b.name))
    });
}

pub fn sort_processes(list: &mut [ProcessInfo], sort: SortBy) {
    match sort {
        SortBy::Cpu => list.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(b.memory.cmp(&a.memory))),
        SortBy::Memory => list.sort_by_key(|p| std::cmp::Reverse(p.memory)),
        SortBy::Name => list.sort_by_key(|p| p.name.to_lowercase()),
        SortBy::Pid => list.sort_by_key(|p| p.pid),
    }
}

#[cfg(test)]
#[path = "monitor_test.rs"]
mod tests;
