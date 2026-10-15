//! batten: reads the values a terminal status bar shows, in one process.
//!
//! One execution prints every value, so the cost per tick is one process
//! regardless of how many segments a bar draws. A key argument narrows it, and
//! an unreadable value prints empty rather than failing, so one missing source
//! never blanks a bar.

use std::fmt::Write as _;

const KEYS: [&str; 7] = ["cpu", "ram", "load", "temp", "disk", "bat", "bat_icon"];

fn cpu_counters(proc_stat: &std::path::Path) -> Option<[u64; 8]> {
    let content = std::fs::read_to_string(proc_stat).ok()?;
    let line = content.lines().find_map(|l| {
        let rest = l.strip_prefix("cpu")?;
        // aggregate line is exactly "cpu", not "cpu0"
        if rest.starts_with(|c: char| c.is_ascii_digit()) {
            return None;
        }
        Some(rest)
    })?;
    let mut out = [0u64; 8];
    let mut parts = line.split_whitespace();
    for slot in out.iter_mut() {
        *slot = parts.next()?.parse().ok()?;
    }
    Some(out)
}

fn cpu_usage(old: [u64; 8], now: [u64; 8]) -> Option<f64> {
    let old_total: u128 = old.iter().map(|&v| v as u128).sum();
    let now_total: u128 = now.iter().map(|&v| v as u128).sum();
    // busy excludes idle (idx 3) and iowait (idx 4), matching scripts/cpu.sh
    let busy = |c: [u64; 8]| {
        (c[0] as u128) + (c[1] as u128) + (c[2] as u128) + (c[5] as u128) + (c[6] as u128)
            + (c[7] as u128)
    };
    let dt = now_total.checked_sub(old_total)?;
    let db = busy(now).checked_sub(busy(old))?;
    (dt > 0).then(|| db as f64 * 100.0 / dt as f64)
}

fn cpu_cache_path() -> Option<std::path::PathBuf> {
    if let Ok(path) = std::env::var("BATTEN_CPU_CACHE") {
        if !path.is_empty() {
            return Some(path.into());
        }
    }
    let base = std::env::var("XDG_CACHE_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|h| std::path::PathBuf::from(h).join(".cache")))?;
    Some(base.join("batten").join("cpu.stat"))
}

fn cpu_with_paths(proc_stat: &std::path::Path, cache: &std::path::Path) -> Option<String> {
    let current = cpu_counters(proc_stat)?;
    let previous = std::fs::read_to_string(cache).ok().and_then(|s| {
        let mut nums = [0u64; 8];
        let mut parts = s.split_whitespace();
        for slot in nums.iter_mut() {
            *slot = parts.next()?.parse().ok()?;
        }
        Some(nums)
    });
    if let Some(parent) = cache.parent() {
        if std::fs::create_dir_all(parent).is_ok() {
            let mut current_line = String::new();
            for (i, v) in current.iter().enumerate() {
                if i > 0 {
                    current_line.push(' ');
                }
                let _ = std::fmt::Write::write_fmt(&mut current_line, format_args!("{v}"));
            }
            current_line.push('\n');
            let tmp = cache.with_extension("stat.tmp");
            if std::fs::write(&tmp, current_line).is_ok() {
                let _ = std::fs::rename(&tmp, cache);
            }
        }
    }
    previous.and_then(|old| cpu_usage(old, current)).map(|pct| format!("{pct:.1}"))
}

fn cpu() -> Option<String> {
    cpu_with_paths(
        &std::path::PathBuf::from(
            std::env::var("BATTEN_PROC").unwrap_or_else(|_| "/proc".to_string()),
        )
        .join("stat"),
        &cpu_cache_path()?,
    )
}

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
            "cpu" => cpu(),
            "ram" => ram(),
            "load" => load(),
            _ => None,
        };
        let _ = writeln!(out, "{key}={}", value.unwrap_or_default());
    }
    print!("{out}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_usage_matches_shell_backend_math() {
        let old = [100, 0, 100, 800, 0, 0, 0, 0];
        let now = [150, 0, 150, 900, 0, 0, 0, 0];
        assert_eq!(format!("{:.1}", cpu_usage(old, now).unwrap()), "50.0");
    }

    #[test]
    fn cpu_usage_rejects_zero_or_negative_delta() {
        let same = [100, 0, 100, 800, 0, 0, 0, 0];
        assert!(cpu_usage(same, same).is_none());
        assert!(cpu_usage([200, 0, 200, 800, 0, 0, 0, 0], [100, 0, 100, 900, 0, 0, 0, 0]).is_none());
    }

    #[test]
    fn cpu_second_tick_reports_delta_and_updates_cache() {
        let dir = std::env::temp_dir().join(format!("batten-cpu-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let proc = dir.join("stat");
        let cache = dir.join("cpu.stat");
        std::fs::write(&proc, "cpu  100 0 100 800 0 0 0 0\ncpu0 50 0 50 400 0 0 0 0\n").unwrap();
        assert!(cpu_with_paths(&proc, &cache).is_none());
        std::fs::write(&proc, "cpu  150 0 150 900 0 0 0 0\n").unwrap();
        assert_eq!(cpu_with_paths(&proc, &cache).as_deref(), Some("50.0"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
