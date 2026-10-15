# batten

Reads the values a terminal status bar shows, in one process per tick.

```console
$ batten
cpu=
ram=35.3
load=3.07 1.98 1.47
temp=
disk=
bat=
bat_icon=
```

## Why one process

A status bar that asks a separate program for each value spends its tick on
process setup rather than on reading anything. Seven values means seven
executions, and on a phone that is the whole cost of the bar.

batten reads every source once and prints them all, so the cost per tick is one
process regardless of how many values the bar shows. Each value is also
individually addressable, for bars that refresh at different rates:

```console
$ batten cpu ram
cpu=4.2
ram=31.8
```

`batten <key>` prints one value, and `batten probe` exits zero when the host has
something worth showing, which lets a bar hide a segment instead of rendering it
empty.

## Values

| key | source |
|---|---|
| `cpu` | `/proc/stat` delta, whole device |
| `ram` | `MemAvailable` against `MemTotal`, `vm_stat` on macOS |
| `load` | `getloadavg` |
| `temp` | `sensors`, thermal zones as a fallback |
| `disk` | `df` on the root filesystem |
| `bat` | sysfs, `pmset` on macOS, `rish` on Android |
| `bat_icon` | a glyph derived from `bat` and the charging flag |

`cpu`, `ram` and `load` are implemented. The rest print empty.

A value that cannot be read is printed empty. Nothing is guessed and nothing
exits non-zero for a missing source, so one unreadable value never blanks a bar.
An unknown key exits 2 and names the keys.

## Build

```console
$ cargo build --release
$ install -Dm755 target/release/batten ~/.local/bin/batten
```

No runtime dependencies.

## Status

Scaffold. The crate builds and `batten` runs, but the value sources are stubs
and there is no release yet. Linux is the only platform exercised.
