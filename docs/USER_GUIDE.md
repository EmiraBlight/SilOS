# SilOS User Guide

## 1. Prerequisites

| Requirement | Why |
| --- | --- |
| Rust nightly `2026-07-01` | Pinned in [`rust-toolchain.toml`](../rust-toolchain.toml); the kernel uses unstable features (`abi_x86_interrupt`, `custom_test_frameworks`, `build-std`) |
| `rust-src`, `llvm-tools-preview` | Needed to cross-compile `core`/`alloc` for the bare-metal target |
| `bootimage` (`cargo install bootimage`) | Links the kernel with the bootloader into a bootable disk image |
| `qemu-system-x86_64` | The machine SilOS runs on |

`rustup` picks the toolchain up automatically from `rust-toolchain.toml`; the components are
declared there too, so a plain `rustup toolchain install` in the repo pulls everything.

## 2. The disk image

SilOS mounts a second IDE disk as its filesystem. That disk is the file `storage.bin` in the repo
root. It is **git-ignored and not created for you** — make it before the first boot:

```bash
dd if=/dev/zero of=storage.bin bs=1M count=32
```

> **Gotcha:** the path to this file is hard-coded as an absolute path in
> [`Cargo.toml`](../Cargo.toml) under `[package.metadata.bootimage].run-args`. If you cloned this
> repo anywhere other than `/home/sammy/SilOS`, edit that line to match your checkout, or QEMU
> will fail to open the drive.

## 3. Build and run

```bash
cargo build          # compile the kernel only
cargo run            # build a bootimage and launch it in QEMU
cargo test           # run the integration test suite headless in QEMU
```

`cargo run` uses the `bootimage runner` configured in [`.cargo/config.toml`](../.cargo/config.toml),
which builds `target/x86_64-myOs/debug/bootimage-myOS.bin` and boots QEMU with:

- `storage.bin` attached as IDE disk index 1,
- the `isa-debug-exit` device on port `0xf4`, which lets the kernel shut QEMU down cleanly.

Tests run with `-display none -serial stdio`, so results stream to your terminal. A successful run
exits QEMU with code `33`; `cargo test` is configured to treat that as success.

## 4. First boot

On boot the kernel prints its FAT16 status line. If you made a blank `storage.bin`, it will say
the BPB is invalid — that is expected on an unformatted disk. Run `formatd` once:

```
> formatd
Formatted drive!
```

Then reboot (`quit`, then `cargo run` again) so the filesystem is mounted from the new BPB.
`fat16::init()` only runs at startup, so formatting does not retroactively mount the volume in the
current session.

## 5. Argument separator

**SilOS splits a command line on `?`, not on whitespace.** Each `?`-separated field is trimmed and
becomes one argument. This is the single most surprising thing about the shell:

```
> echo?hello world          # one argument: "hello world"
> mkdir?notes?txt?my text   # four arguments
> cat?notes?txt
```

Spaces inside a field are preserved, which is how multi-word file contents and whole Lisp programs
get passed as a single argument.

## 6. Command reference

Registered in `init_cmds()` in [`src/commands.rs`](../src/commands.rs).

### Filesystem

| Command | Usage | Notes |
| --- | --- | --- |
| `formatd` | `formatd` | Writes a fresh FAT16 boot sector, two FATs and an empty root directory to the disk. **Destroys all data.** |
| `mkdir` | `mkdir?NAME?EXT?data...` | Creates a *file* (the name is a misnomer — there are no directories). Everything from the 4th field on is joined with spaces as the contents. |
| `cat` | `cat?NAME?EXT` | Prints a file. Fails on non-UTF-8 contents. |
| `edit` | `edit?NAME?EXT?data` | Replaces a file's contents in one shot, non-interactively. |
| `eden` | `eden?NAME?EXT` | Opens the full-screen editor (see below). Creates the file if absent. |
| `run` | `run?NAME?EXT?args...` | Reads a file and executes its contents as a Lisp program. |

Names are upper-cased and padded to the 8.3 layout, so `notes` becomes `NOTES   ` / `TXT`.

### Raw disk

| Command | Usage | Notes |
| --- | --- | --- |
| `read` | `read?SECTOR` | Dumps 512 raw bytes of a sector as decimal numbers. |
| `show` | `show?SECTOR` | Prints a sector interpreted as a UTF-8 string. |
| `write` | `write?SECTOR?data` | Writes up to 512 bytes to a raw sector. Bypasses the filesystem entirely. |

### Shell and system

| Command | Usage | Notes |
| --- | --- | --- |
| `echo` | `echo?text` | Prints its first argument. |
| `clear` | `clear` | Clears the screen. |
| `history` | `history` | Prints previously entered commands. |
| `pong` | `pong` | Launches the game. |
| `quit` | `quit` | Flushes the disk, masks interrupts and powers off. |
| `parse` | `parse?(lisp code);` | Evaluates Lisp source given inline. |
| `bind` | `bind?NAME?(lisp code);` | Registers `NAME` as a new shell command that runs the given Lisp. Persists for the session only. |

### Example session

```
> formatd
> mkdir?greet?lsp?(sys echo "hello from disk");
> run?greet?lsp
hello from disk
> bind?greet?(sys echo "hello from a binding");
> greet
hello from a binding
```

## 7. The `eden` editor

`eden?NAME?EXT` opens a full-screen editor on the VGA text buffer.

| Key | Action |
| --- | --- |
| Printable characters | Insert at cursor |
| <kbd>Backspace</kbd> | Delete backwards; joins lines at column 0 |
| <kbd>Enter</kbd> | Split the line at the cursor |
| Arrow keys | Move the cursor, wrapping between lines |
| <kbd>Esc</kbd> | **Save and exit** |

There is no way to quit without saving, and no scrolling — the buffer is drawn from line 0, so
content past row 24 is edited blind. The file is written on exit with `overwrite_file` if it
exists, or `write_new_file` if it does not.

## 8. Pong

`pong` starts a two-player game on the 80×25 text screen.

| Key | Action |
| --- | --- |
| <kbd>W</kbd> / <kbd>S</kbd> | Player 1 (left paddle) up / down |
| <kbd>O</kbd> / <kbd>L</kbd> | Player 2 (right paddle) up / down |
| <kbd>Esc</kbd> | Quit back to the shell |

The ball reaching either edge ends the game and prints the winner. Speed is governed by a busy
`nop` loop, so it scales with host speed rather than wall-clock time.

## 9. Scripting in Lisp

See [lisp.md](lisp.md) for the language reference. Practical notes:

- **Every statement must end with `;`.** The interpreter splits on `;` and discards the trailing
  fragment, so text after the final `;` is never executed.
- Numeric and boolean command-line arguments are injected as `n0`, `n1`, … and `b0`, `b1`, …
  Only the first two of each are pre-seeded with defaults.
- `(sys <command> <args...>)` calls back into the shell command table — this is how Lisp reaches
  the filesystem and the screen.
- `(error "msg")` calls the current `error` binding and then aborts the program.

```
> parse?(def a 3);(for (> a 0) (do (sys echo a) (def a (- a 1))));
3
2
1
```

## 10. Troubleshooting

| Symptom | Cause |
| --- | --- |
| QEMU: `could not open disk image .../storage.bin` | `storage.bin` missing, or the absolute path in `Cargo.toml` does not match your checkout |
| `[FS] Error: Invalid BPB found` | Disk is blank or not FAT16 — run `formatd`, then reboot |
| `'foo' command not found` | Command name is case-sensitive and must be the first `?`-field |
| Commands silently take the wrong arguments | You used spaces instead of `?` |
| `statements must end with a semicolon` | A Lisp program with no `;` at all |
| Nothing happens after `cargo run` | `bootimage` not installed, or `qemu-system-x86_64` not on `PATH` |
