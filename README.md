# SilOS

A hobby x86_64 operating system kernel written from scratch in Rust — `no_std`, freestanding,
booted by the [`bootloader`](https://crates.io/crates/bootloader) crate and run under QEMU.

It boots to an interactive shell with a cooperative async runtime, a FAT16 filesystem on a real
ATA/IDE disk, a full-screen text editor, a Lisp interpreter that can call back into the shell,
and a game of Pong.

```
[FS] FAT16 Initialized. Sectors per cluster: 4
> echo?hello world
hello world
> eden?notes?txt
```

## Contents

| Document | For |
| --- | --- |
| [docs/USER_GUIDE.md](docs/USER_GUIDE.md) | Building, running, and every shell command |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | How the kernel is layered and how boot works |
| [docs/FILE_MAP.md](docs/FILE_MAP.md) | Every file, what it owns, and its key symbols |
| [docs/project-graph.json](docs/project-graph.json) | Machine-readable module graph for agents |
| [docs/lisp.md](docs/lisp.md) | The Lisp dialect reference |
| [CLAUDE.md](CLAUDE.md) | Working agreement for AI agents in this repo |

## Quick start

```bash
# One-time: the tools that turn a kernel binary into a bootable image
cargo install bootimage
rustup component add rust-src llvm-tools-preview

# Create the 32 MiB disk image the kernel mounts as its FAT16 volume
dd if=/dev/zero of=storage.bin bs=1M count=32

# Boot it
cargo run
```

Then at the `>` prompt type `formatd` once to lay down a FAT16 filesystem on the blank disk.

> **Arguments are separated by `?`, not spaces.** `echo?hello` — see
> [docs/USER_GUIDE.md](docs/USER_GUIDE.md#argument-separator).

## Feature tour

- **Cooperative async kernel** — a hand-rolled `Future` executor with a waker queue; tasks yield
  with `yield_now().await` and the CPU `hlt`s when nothing is runnable.
- **Real disk I/O** — PIO-mode ATA driver talking to ports `0x1F0–0x1F7`, with a FAT16 driver on
  top supporting create, read, overwrite and format.
- **`eden`, a modal-less text editor** — full-screen editing over the VGA text buffer, saves on
  <kbd>Esc</kbd>.
- **A Lisp** — closures, maps, lists, recursion, and a `sys` form that invokes shell commands, so
  Lisp scripts stored on disk can be bound as new shell commands at runtime.
- **VGA text output** with colour, hardware cursor control and scrolling.
- **Interrupt handling** — GDT/TSS with an IST for double faults, PIC-driven timer, keyboard and
  ATA interrupts, and a page-fault handler.

## Status

This is a learning project, closely following (and then departing from) Philipp Oppermann's
*Writing an OS in Rust*. It is not hardened, has no memory protection between "programs", no
process isolation, and runs everything in ring 0. Known rough edges are listed per-file in
[docs/FILE_MAP.md](docs/FILE_MAP.md#known-rough-edges).

## Licence

See [LICENSE](LICENSE).
