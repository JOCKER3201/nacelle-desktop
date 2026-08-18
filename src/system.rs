//! System information sampling on a separate thread (sysinfo + /sys, /proc).

use std::net::{TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// The telemetry data model (Snapshot, formatting) is shared by every
// platform and lives in libnacelle; this file keeps the Linux
// collectors (/sys, /proc, sysinfo) that fill it.
pub use nacelle::telemetry::{ProcEntry, Snapshot};

fn read_sys(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn chassis_name(code: &str) -> &'static str {
    match code {
        "3" | "4" | "6" | "7" => "DESKTOP",
        "8" | "9" | "10" | "14" => "LAPTOP",
        "11" => "HANDHELD",
        "13" => "ALL IN ONE",
        "17" | "23" => "SERVER",
        "30" => "TABLET",
        "31" | "32" => "CONVERTIBLE",
        _ => "UNKNOWN",
    }
}

fn battery() -> Option<(u8, bool)> {
    for bat in ["BAT0", "BAT1", "BATT"] {
        let base = format!("/sys/class/power_supply/{bat}");
        if let Some(cap) = read_sys(&format!("{base}/capacity")) {
            let pct = cap.parse::<u8>().ok()?;
            let charging = read_sys(&format!("{base}/status"))
                .map(|s| s == "Charging" || s == "Full")
                .unwrap_or(false);
            return Some((pct, charging));
        }
    }
    None
}

fn local_ipv4() -> Option<String> {
    // UDP connect sends no packets — it only selects the interface.
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("1.1.1.1:80").ok()?;
    Some(sock.local_addr().ok()?.ip().to_string())
}

/// One process, as /proc/<pid>/stat describes it: the name, the CPU
/// time it has burned so far, and its resident pages.
struct RawProc {
    pid: u32,
    name: String,
    ticks: u64,
    rss_pages: u64,
}

/// Reads every process straight from /proc.
///
/// This used to be sysinfo's job, and sysinfo is thorough: for each
/// process it also walks /proc/<pid>/task and reads statm and io for
/// every THREAD, which on a running desktop was four fifths of every
/// system call the program made — for a widget that shows a pid, a
/// name and two percentages. One file per process gives all four.
fn read_procs() -> Vec<RawProc> {
    let Ok(dir) = std::fs::read_dir("/proc") else { return Vec::new() };
    let mut out = Vec::with_capacity(512);
    for entry in dir.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        // The command sits in parentheses and may contain anything at
        // all, spaces and brackets included — so the fields after it
        // are found from the LAST closing bracket, never by splitting
        // the whole line.
        let Some(close) = stat.rfind(')') else { continue };
        let Some(open) = stat.find('(') else { continue };
        let comm = stat[open + 1..close].to_string();
        let rest: Vec<&str> = stat[close + 2..].split_whitespace().collect();
        // After the state field, these are stat's fields 14, 15 and 24
        // (utime, stime, rss) counted from one.
        if rest.len() < 22 {
            continue;
        }
        let num = |i: usize| rest.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        out.push(RawProc {
            pid,
            name: comm,
            ticks: num(11) + num(12),
            rss_pages: num(21),
        });
    }
    out
}

/// The sysinfo handle this sampler keeps, holding what this sampler
/// actually asks it for: the cpus and the memory totals.
///
/// `System::new_all()` is what stood here, and `_all` means EVERY
/// PROCESS AND EVERY THREAD — which on Linux sysinfo does not merely
/// read but KEEPS OPEN: each `Process` it builds holds its
/// `/proc/<pid>/stat` file so that the next refresh can reuse the
/// handle instead of reopening it. There is no next refresh. The loop
/// below asks this handle for cpu and memory only and reads processes
/// itself through [`read_procs`], so 511 process and 1091 task handles
/// were opened at startup and held until the program exited: about
/// 1600 descriptors serving nobody, on a cache nothing ever hit again.
///
/// They were not free. sysinfo raises the process's soft descriptor
/// limit to the hard one before it starts storing them, and every
/// descriptor the program opened afterwards — the per-frame dance
/// around the driver included — was numbered above the plateau they
/// left behind.
///
/// The priming pair is what `new_all` also did and the loop still
/// needs: cpu usage is the difference between two readings, so the
/// first pass has to have something to subtract from.
fn probe() -> sysinfo::System {
    let mut sys = sysinfo::System::new();
    sys.refresh_cpu();
    sys.refresh_memory();
    sys
}

pub fn start() -> Arc<Mutex<Snapshot>> {
    let snap = Arc::new(Mutex::new(Snapshot::default()));
    let shared = snap.clone();
    let offline_mode = std::env::var("NACELLE_OFFLINE").is_ok();

    std::thread::spawn(move || {
        use sysinfo::System;
        let mut sys = probe();
        let mut networks = sysinfo::Networks::new_with_refreshed_list();
        let mut components = sysinfo::Components::new_with_refreshed_list();

        // Static data
        let manufacturer = read_sys("/sys/class/dmi/id/sys_vendor").unwrap_or("UNKNOWN".into());
        let model = read_sys("/sys/class/dmi/id/product_name").unwrap_or("UNKNOWN".into());
        let chassis = read_sys("/sys/class/dmi/id/chassis_type")
            .map(|c| chassis_name(&c).to_string())
            .unwrap_or("UNKNOWN".into());
        let hostname = System::host_name().unwrap_or("localhost".into());
        let username = std::env::var("USER").unwrap_or("user".into());
        let os_name = System::name().unwrap_or("Linux".into());
        let kernel = System::kernel_version().unwrap_or_default();

        let mut last_net: Option<(u64, u64, Instant)> = None;
        let mut last_ping = Instant::now() - Duration::from_secs(60);
        // What each process had burned at the previous pass, for the
        // rate; plus the two constants the arithmetic needs.
        let mut last_ticks: std::collections::HashMap<u32, u64> =
            std::collections::HashMap::new();
        let mut last_procs_at = Instant::now();
        let tick_hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as f32;
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(1) as u64;
        let mut ping_ms: Option<u32> = None;
        let mut online = false;
        let mut ipv4 = local_ipv4();

        loop {
            sys.refresh_cpu();
            sys.refresh_memory();
            networks.refresh();
            components.refresh();

            let cpus = sys.cpus();
            let per_core: Vec<f32> = cpus.iter().map(|c| c.cpu_usage()).collect();
            let cpu_name = cpus
                .first()
                .map(|c| c.brand().trim().to_string())
                .unwrap_or_default();

            let temp_c = (&components)
                .into_iter()
                .find(|c| {
                    let n = c.label().to_lowercase();
                    n.contains("tctl") || n.contains("package") || n.contains("cpu")
                })
                .map(|c| c.temperature());

            // Total traffic across all interfaces except lo.
            let mut total_rx = 0u64;
            let mut total_tx = 0u64;
            let mut iface = String::new();
            let mut best = 0u64;
            for (name, data) in &networks {
                if name.as_str() == "lo" {
                    continue;
                }
                total_rx += data.total_received();
                total_tx += data.total_transmitted();
                if data.total_received() > best {
                    best = data.total_received();
                    iface = name.clone();
                }
            }
            let now = Instant::now();
            let (up_rate, down_rate) = if let Some((rx0, tx0, t0)) = last_net {
                let dt = now.duration_since(t0).as_secs_f64().max(0.001);
                (
                    (total_tx.saturating_sub(tx0)) as f64 / dt,
                    (total_rx.saturating_sub(rx0)) as f64 / dt,
                )
            } else {
                (0.0, 0.0)
            };
            last_net = Some((total_rx, total_tx, now));

            // Ping every 5 s (TCP to 1.1.1.1:80) — disabled via NACELLE_OFFLINE.
            if !offline_mode && now.duration_since(last_ping) > Duration::from_secs(5) {
                last_ping = now;
                let t0 = Instant::now();
                match TcpStream::connect_timeout(
                    &"1.1.1.1:80".parse().unwrap(),
                    Duration::from_millis(1500),
                ) {
                    Ok(_) => {
                        ping_ms = Some(t0.elapsed().as_millis() as u32);
                        online = true;
                    }
                    Err(_) => {
                        ping_ms = None;
                        online = false;
                    }
                }
                ipv4 = local_ipv4();
            }

            // Processes, read here rather than through sysinfo — see
            // read_procs. The percentage is time burned since the last
            // pass over wall time since the last pass, so a process
            // saturating one core reads a hundred, as everywhere else.
            let procs = read_procs();
            let elapsed = last_procs_at.elapsed().as_secs_f32().max(0.001);
            last_procs_at = Instant::now();
            let mut top: Vec<ProcEntry> = procs
                .iter()
                .map(|p| {
                    // A process that was not there last time has no
                    // rate yet; claiming one would be inventing it.
                    let cpu = match last_ticks.get(&p.pid) {
                        None => 0.0,
                        Some(t0) => {
                            p.ticks.saturating_sub(*t0) as f32 / tick_hz / elapsed * 100.0
                        }
                    };
                    ProcEntry {
                        pid: p.pid,
                        name: p.name.clone(),
                        cpu,
                        mem_pct: if sys.total_memory() > 0 {
                            (p.rss_pages * page_size) as f32 / sys.total_memory() as f32
                                * 100.0
                        } else {
                            0.0
                        },
                    }
                })
                .collect();
            last_ticks.clear();
            last_ticks.extend(procs.iter().map(|p| (p.pid, p.ticks)));
            let proc_count = top.len();
            top.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
            // Enough to fill a full-height panel on a large screen. The
            // widget shows as many as its box has room for; eight was a
            // fixed-layout leftover, and left a tall process panel with
            // empty space no data could reach.
            top.truncate(128);

            let load = System::load_average();
            // Read the battery (blocking /sys I/O) BEFORE taking the lock,
            // so the UI thread is never blocked waiting on sysfs.
            let battery = battery();

            {
                let mut s = shared.lock().unwrap();
                let generation = s.generation.wrapping_add(1);
                *s = Snapshot {
                    generation,
                    cpu_name: cpu_name.clone(),
                    cpu_per_core: per_core,
                    load_avg: [load.one, load.five, load.fifteen],
                    temp_c,
                    mem_total: sys.total_memory(),
                    mem_used: sys.used_memory(),
                    swap_total: sys.total_swap(),
                    swap_used: sys.used_swap(),
                    uptime: System::uptime(),
                    top,
                    proc_count,
                    net_up_rate: up_rate,
                    net_down_rate: down_rate,
                    iface: iface.clone(),
                    ipv4: ipv4.clone(),
                    ping_ms,
                    online: if offline_mode { false } else { online },
                    battery,
                    manufacturer: manufacturer.clone(),
                    model: model.clone(),
                    chassis: chassis.clone(),
                    hostname: hostname.clone(),
                    username: username.clone(),
                    os_name: os_name.clone(),
                    kernel: kernel.clone(),
                };
            }
            std::thread::sleep(Duration::from_millis(1000));
        }
    });

    snap
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every descriptor this process is holding on ANOTHER process's
    /// `/proc` entry, by what it points at.
    ///
    /// The names rather than the count, because the count alone cannot
    /// say what leaked; `/proc/self/fd` is read through its own
    /// directory handle, which points at `/proc/<us>/fd` and so matches
    /// none of the files below.
    fn proc_handles() -> Vec<String> {
        let Ok(rd) = std::fs::read_dir("/proc/self/fd") else { return Vec::new() };
        let mut out: Vec<String> = rd
            .flatten()
            .filter_map(|e| std::fs::read_link(e.path()).ok())
            .map(|t| t.to_string_lossy().into_owned())
            .filter(|t| {
                t.starts_with("/proc/")
                    && (t.ends_with("/stat")
                        || t.ends_with("/statm")
                        || t.ends_with("/io")
                        || t.contains("/task/"))
            })
            .collect();
        out.sort();
        out
    }

    /// THE TELEMETRY LEAVES NO DESCRIPTOR BEHIND.
    ///
    /// Both halves of the sampler are asked here, and only one of them
    /// was ever wrong: the process scan opens a file per process and
    /// closes it, while the sysinfo handle used to be built with
    /// `new_all` and kept a `/proc/<pid>/stat` open for every process
    /// and every thread on the machine — about 1600 of them — for the
    /// whole life of the program. The handle is alive at the assertion
    /// on purpose: this is not a question about `Drop`, it is about
    /// what the sampler carries while it runs.
    #[test]
    fn the_telemetry_holds_no_proc_handles_while_it_runs() {
        let before = proc_handles();
        let mut sys = probe();
        sys.refresh_cpu();
        sys.refresh_memory();
        assert_eq!(
            proc_handles(),
            before,
            "the sysinfo handle is holding /proc files open (was {} before)",
            before.len()
        );

        // And the scan that DOES read every process gives back what it
        // takes. Fail-closed: a scan that read nothing would satisfy
        // any assertion about descriptors.
        let seen = read_procs().len();
        assert!(seen > 0, "no process was read at all — the assertion below proves nothing");
        assert_eq!(proc_handles(), before, "the process scan left handles open");

        // Held to here so the assertions above are about a LIVE probe.
        assert!(sys.total_memory() > 0, "the probe must still answer what the loop asks it");
    }
}
