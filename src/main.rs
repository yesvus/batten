//! batten: reads the values a terminal status bar shows, in one process.
//!
//! One execution prints every value, so the cost per tick is one process
//! regardless of how many segments a bar draws. A key argument narrows it, and
//! an unreadable value prints empty rather than failing, so one missing source
//! never blanks a bar.

use std::fmt::Write as _;

const KEYS: [&str; 7] = ["cpu", "ram", "load", "temp", "disk", "bat", "bat_icon"];

fn ram() -> Option<String> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    let field = |name: &str| -> Option<u64> {
        meminfo
            .lines()
            .find_map(|l| l.strip_prefix(name))
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse().ok())
    };
    let (total, avail) = (field("MemTotal:")?, field("MemAvailable:")?);
    (total > 0).then(|| format!("{:.1}", (total - avail) as f64 * 100.0 / total as f64))
}

fn load() -> Option<String> {
    let line = std::fs::read_to_string("/proc/loadavg").ok()?;
    let mut parts = line.split_whitespace().take(3);
    let mut out = String::new();
    for _ in 0..3 {
        let _ = write!(out, "{} ", parts.next()?);
    }
    Some(out.trim_end().to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "probe") {
        // nothing is hidden yet: every key resolves, empty or not
        std::process::exit(0);
    }

    let wanted: Vec<&str> = if args.is_empty() {
        KEYS.to_vec()
    } else {
        let mut wanted = Vec::with_capacity(args.len());
        for arg in &args {
            match KEYS.iter().find(|k| **k == arg.as_str()) {
                Some(key) => wanted.push(*key),
                None => {
                    eprintln!("batten: unknown key {arg:?}, expected one of: {}", KEYS.join(" "));
                    std::process::exit(2);
                }
            }
        }
        wanted
    };

    let mut out = String::new();
    for key in wanted {
        let value = match key {
            "ram" => ram(),
            "load" => load(),
            _ => None,
        };
        let _ = writeln!(out, "{key}={}", value.unwrap_or_default());
    }
    print!("{out}");
}
