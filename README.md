<div align="center">

# nosleep

**Keep your Linux laptop awake. Cleanly.**

A terminal app that holds a `systemd-logind` sleep lock for as long as you need, keeps the
screen on and ignores the lid if you ask it to, and puts everything back when it's done.

[![crates.io](https://img.shields.io/crates/v/nosleep-tui?logo=rust&color=a78bfa&label=crates.io)](https://crates.io/crates/nosleep-tui)
[![CI](https://github.com/AngheloBR/nosleep-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/AngheloBR/nosleep-tui/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-22d3ee)](#license)

[Español](README.es.md) · English

![nosleep TUI holding a 2-hour lock](docs/tui-active.svg)

</div>

## Highlights

- **Interactive TUI.** Big countdown, progress bar, one-key presets, live toggles and a live
  view of every inhibitor on the system. Run `nosleep` and you're in.
- **Scriptable CLI.** `nosleep -t 2h`, `nosleep --while cargo build`, `nosleep --list`.
- **Can't leak a lock.** The logind lock lives in a file descriptor: if the process dies, even
  by `SIGKILL`, the kernel closes it and logind lets go.
- **Screen stays on (X11).** Turns off the screensaver and DPMS by speaking the X11 protocol
  directly, then puts your exact previous settings back. No `xset` needed.
- **Crash-safe restore.** Screen settings are saved to disk *before* anything changes; after a
  hard kill, `nosleep --restore` puts them back.
- **One static binary.** No Python, no `libdbus`, no `libX11`, no runtime dependencies.

## Install

### Prebuilt binary (recommended)

Download one file. No Rust and no extra packages needed:

```sh
curl -fsSL https://github.com/AngheloBR/nosleep-tui/releases/latest/download/nosleep-x86_64-unknown-linux-musl.tar.gz \
  | tar xz --strip-components=1 -C ~/.local/bin nosleep-x86_64-unknown-linux-musl/nosleep
```

On ARM (Raspberry Pi, ARM laptops) use `aarch64-unknown-linux-musl` instead.
Every release also ships a `.sha256` checksum.

### Fedora / RHEL / openSUSE (.rpm) · Debian / Ubuntu / Mint (.deb)

Download the package for your system from the
[latest release](https://github.com/AngheloBR/nosleep-tui/releases/latest) and install it
with your package manager:

```sh
sudo dnf install ./nosleep-tui-*.x86_64.rpm     # Fedora, RHEL
sudo apt install ./nosleep-tui_*_amd64.deb      # Debian, Ubuntu, Mint
```

The binary inside is static, so the same package works on old and new releases alike,
Debian stable included. Uninstall with `sudo dnf remove nosleep-tui` / `sudo apt remove nosleep-tui`.

### With Cargo (crates.io)

If you already have Rust (1.88+):

```sh
cargo install nosleep-tui     # installs the `nosleep` command
```

Run the same command again to update, and `cargo uninstall nosleep-tui` to remove it.
Cargo puts the binary in `~/.cargo/bin`; if your shell can't find `nosleep`, add that folder
to your `PATH`.

### From source

For the latest unreleased changes on `main`:

```sh
cargo install --git https://github.com/AngheloBR/nosleep-tui
```

### Requirements

- Linux with **systemd-logind** (Fedora, Ubuntu, Arch, Debian and most others).
- `--screen` / "keep screen on" also needs an **X11** session.

## The TUI

Run `nosleep` with no arguments.

![Custom duration with a live preview](docs/tui-custom.svg)

| Key              | Action                                   |
| ---------------- | ---------------------------------------- |
| `Enter` / `s`    | start, or restart the countdown          |
| `x`              | stop and restore everything              |
| `+` / `-`        | add or remove 5 minutes                  |
| `1` … `7`        | 10m · 30m · 1h · 2h · 4h · 8h · ∞        |
| `c`              | custom duration, with a live preview     |
| `p`              | keep the screen on (X11)                 |
| `t`              | ignore the lid switch                    |
| `r`              | change the reason shown to logind        |
| `?`              | help                                     |
| `q`              | quit (asks first if a lock is active)    |

Changing options while the lock is active applies them right away. The new lock is taken
*before* the old one is released, so the laptop is never left unprotected in between.

To open the TUI with some values already set: `nosleep --tui -t 4h --screen`.

## The CLI

Any flag runs `nosleep` as a plain command, which is handy in scripts:

```sh
nosleep -t 1h30m                # block sleep for 1h30m, Ctrl+C to stop early
nosleep -t inf --screen --lid   # until Ctrl+C, screen on, lid ignored
nosleep --while cargo build --release   # only while the command runs; keeps its exit code
nosleep --list                  # every active inhibitor on the system
nosleep --restore               # repair screen settings after a hard kill
```

![nosleep in CLI mode](docs/demo.svg)

### Durations

| Input             | Meaning                     |
| ----------------- | --------------------------- |
| `45s` `90m` `2h`  | seconds, minutes, hours     |
| `1d`              | days (maximum `365d`)       |
| `1h30m`           | combined                    |
| `90`              | bare number = minutes       |
| `1.5h` / `1,5h`   | decimals, with `.` or `,`   |
| `inf` / `0`       | no time limit               |

### Options

| Flag              | Description                                              |
| ----------------- | -------------------------------------------------------- |
| `-t, --time DUR`  | How long to hold the lock (default `1h`)                 |
| `-s, --screen`    | Keep the screen on (X11 / DPMS)                          |
| `--lid`           | Also inhibit `handle-lid-switch`                         |
| `--reason TEXT`   | Reason shown to logind and in `--list`                   |
| `--while CMD...`  | Hold the lock only while `CMD` runs                      |
| `--tui`           | Open the TUI using the flags above as starting values    |
| `--restore`       | Restore screen settings left behind by a hard kill       |
| `--list`          | List active inhibitors                                   |

## How it works

```
nosleep ──D-Bus──▶ org.freedesktop.login1.Manager.Inhibit("idle:sleep[:handle-lid-switch]", …, "block")
                     └─▶ returns a file descriptor; the lock lasts while that fd is open

screen:  read screensaver + DPMS ──▶ $XDG_RUNTIME_DIR/nosleep/screen.json   (1. save)
         screensaver off · no blanking · DPMS off  (X11 protocol, x11rb)    (2. change)
         on exit: restore the saved values and delete the file
```

Locks are released in reverse order (screen first, then the inhibitor). That happens on normal
exit, `q`, `Ctrl+C`, `SIGTERM`, `SIGHUP` and panics.

## Limitations

- Keeping the screen on only works on **X11**. Under Wayland it refuses instead of failing
  silently. The sleep lock itself works on any session.
- Without `--lid` / `t`, closing the lid still suspends if your logind config says so.

## Development

```sh
cargo test
cargo clippy --all-targets
cargo test snapshots -- --ignored   # regenerate the TUI screenshots in docs/
```

The screenshots are produced by the TUI's own drawing code, so they always match the app.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
