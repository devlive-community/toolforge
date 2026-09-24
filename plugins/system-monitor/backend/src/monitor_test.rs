use super::*;

fn args(processes: bool) -> Args {
    Args {
        processes,
        sort: SortBy::Cpu,
        query: String::new(),
        limit: 50,
    }
}

#[test]
fn snapshot_reports_system_state() {
    let monitor = Monitor::default();
    let snap = monitor.snapshot(args(false)).unwrap();
    assert!(snap.cpu.logical_cores > 0);
    assert_eq!(snap.cpu.cores.len(), snap.cpu.logical_cores);
    assert!(snap.memory.total > 0 && snap.memory.used <= snap.memory.total);
    assert!(!snap.os.arch.is_empty());
    assert!(snap.processes.is_none());
    assert!(snap.disks.iter().all(|d| d.available <= d.total));
}

#[test]
fn processes_are_filtered_sorted_and_limited() {
    let monitor = Monitor::default();
    let own = std::process::id();
    let mut a = args(true);
    a.query = own.to_string();
    let snap = monitor.snapshot(a).unwrap();
    let processes = snap.processes.unwrap();
    assert!(processes.total > 1);
    assert_eq!(processes.matched, 1);
    assert_eq!(processes.list[0].pid, own);
    assert!(processes.list[0].memory > 0);

    let mut limited = args(true);
    limited.limit = 3;
    limited.sort = SortBy::Memory;
    let list = monitor.snapshot(limited).unwrap().processes.unwrap().list;
    assert_eq!(list.len(), 3);
    assert!(list[0].memory >= list[1].memory && list[1].memory >= list[2].memory);
}

fn process(pid: u32, name: &str, cpu: f32, memory: u64) -> ProcessInfo {
    ProcessInfo {
        pid,
        parent: None,
        name: name.into(),
        cpu,
        memory,
        status: "Run".into(),
        user: None,
        run_time: 0,
        exe: None,
    }
}

#[test]
fn sorting_orders() {
    let mut list = vec![
        process(3, "b", 1.0, 10),
        process(1, "A", 5.0, 5),
        process(2, "c", 1.0, 50),
    ];
    sort_processes(&mut list, SortBy::Cpu);
    assert_eq!(
        list.iter().map(|p| p.pid).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    sort_processes(&mut list, SortBy::Memory);
    assert_eq!(
        list.iter().map(|p| p.pid).collect::<Vec<_>>(),
        vec![2, 3, 1]
    );
    sort_processes(&mut list, SortBy::Name);
    assert_eq!(
        list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        vec!["A", "b", "c"]
    );
    sort_processes(&mut list, SortBy::Pid);
    assert_eq!(
        list.iter().map(|p| p.pid).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}

fn disk(mount: &str, total: u64, available: u64) -> Disk {
    Disk {
        name: "d".into(),
        mount: mount.into(),
        file_system: "apfs".into(),
        kind: "ssd",
        total,
        available,
        removable: false,
    }
}

#[test]
fn duplicate_volumes_are_merged() {
    let mut disks = vec![
        disk("/", 100, 10),
        disk("/System/Volumes/Data", 100, 10),
        disk("/Volumes/USB", 50, 5),
    ];
    dedup_disks(&mut disks);
    assert_eq!(
        disks.iter().map(|d| d.mount.as_str()).collect::<Vec<_>>(),
        vec!["/", "/Volumes/USB"]
    );
}

fn iface(name: &str, addresses: &[&str], traffic: u64) -> Interface {
    Interface {
        name: name.into(),
        mac: String::new(),
        addresses: addresses.iter().map(|a| a.to_string()).collect(),
        rx_rate: 0,
        tx_rate: 0,
        rx_total: traffic,
        tx_total: 0,
    }
}

#[test]
fn networks_are_tidied() {
    let mut list = vec![
        iface("lo0", &["127.0.0.1/8", "::1/128"], 1000),
        iface("awdl0", &[], 0),
        iface("utun4", &["fe80::1/64"], 50),
        iface("en0", &["192.168.1.2/24"], 10),
    ];
    tidy_networks(&mut list);
    assert_eq!(
        list.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
        vec!["en0", "utun4", "lo0"]
    );
    assert_eq!(real_mac("02:00:00:00:00:00"), "");
    assert_eq!(real_mac("a4:83:e7:00:11:22"), "a4:83:e7:00:11:22");
}
