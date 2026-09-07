# CLAUDE.md

Working agreement for AI agents in the SilOS repository.

## What this is

SilOS is a bare-metal **x86_64 OS kernel written in Rust** — `no_std`, freestanding, booted by the
`bootloader` 0.9 crate and run under QEMU. It is a single-core, ring-0, cooperatively scheduled
kernel with a shell, a FAT16 filesystem on a real ATA disk, a VGA text UI, a Lisp interpreter and a
text editor.

**There is no operating system underneath this code.** No `std`, no threads, no processes, no
memory protection, no syscall boundary. Reflexes from userspace Rust will be wrong here.

## Read this first

| Question | Document |
| --- | --- |
| How is the kernel structured, and what are the invariants? | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| What does file X do, and what are its known bugs? | [docs/FILE_MAP.md](docs/FILE_MAP.md) |
| Machine-readable module graph, flows, recipes | [docs/project-graph.json](docs/project-graph.json) |
| How do I build, run and drive it? | [docs/USER_GUIDE.md](docs/USER_GUIDE.md) |
| What is the Lisp dialect? | [docs/lisp.md](docs/lisp.md) |

`docs/project-graph.json` is the fastest way to orient: it has a node per module with role, key
symbols, dependencies and gotchas, plus named flows (`boot`, `keystroke`, `command-execution`,
`file-write`, `lisp-syscall`) and task recipes.

## Commands

```bash
cargo check     # fast — use this to verify a change compiles
cargo build     # build the kernel binary
cargo run       # boots QEMU — interactive, will not exit on its own
cargo test      # boots QEMU per test binary; headless but slow
```

**Prefer `cargo check` when you only need to know whether it compiles.** `cargo run` launches an
interactive QEMU window that never terminates by itself — do not run it expecting output to come
back. If you need to verify behaviour end to end, ask the user to run it, or state clearly what
you would expect to see.

`cargo test` requires `bootimage`, `qemu-system-x86_64` and a `storage.bin` disk image. First
compilation cross-builds `core` and `alloc` and takes minutes.

## Non-negotiable invariants

Breaking any of these produces a kernel that hangs or triple-faults, usually with no diagnostic.

1. **No allocation before `allocator::init_heap`.** Anything using `Vec`, `String`, `Box`,
   `BTreeMap` or `format!` must run after that point in `kernel_main`.
2. **Every loop awaits `yield_now()`.** There is no preemption. A loop without
   `crate::task::executor::yield_now().await` freezes the whole machine.
3. **Interrupt handlers stay lock-free.** They may touch `ArrayQueue` and the PIC and nothing else.
   Adding a `println!` to a handler can deadlock against interrupted code that holds `WRITER`.
4. **Lock order is `FS` → `IDE`.** `fat16` methods take `IDE.lock()` internally, so never call into
   `fat16` while holding `IDE.lock()`.
5. **Never hold a spinlock across `interrupts::disable()`.** See `power::teardown` for the correct
   pattern.
6. **`WRITER` access from interruptible code goes through `without_interrupts`.** Use `print!` /
   `println!`, which already do.
7. **`gdt::init()` before `interrupts::init_idt()`** — the double-fault handler depends on the IST
   entry the GDT installs.

## Conventions specific to this codebase

- **Two crates, one tree.** `src/lib.rs` is the library (`myOS`); `src/main.rs` is a separate
  binary that consumes it. Inside the library use `crate::`; in `main.rs` and `tests/` use `myOS::`.
- **Shell arguments are separated by `?`, not spaces.** Defined solely in `Shell::getcmd`
  ([src/shell.rs](src/shell.rs)). `echo?hello world` passes one argument, `"hello world"`.
- **Commands are `fn(Vec<String>) -> CommandFuture`**, a `Pin<Box<dyn Future<Output = Result<Success, ProcessError>> + Send>>`.
  `args[0]` is the command name itself.
- **`Success.print_code`** decides whether the shell echoes the success string. Most commands print
  their own output and set it `false`.
- **`parser::eval` is deliberately not an `async fn`.** It returns a boxed future so it can recurse.
  Do not "simplify" it — a plain `async fn` cannot name its own infinitely-sized future type.
- **`commands` and `parser` are mutually dependent by design.** The shell runs Lisp; Lisp's `sys`
  form runs shell commands. `run_cmd` releases `COMMANDS.lock()` before invoking to permit this.
- **`nonstandard_style` is allowed** crate-wide (`myOS`, `programReturn.rs`). Do not "fix" the
  casing; it would break every import.
- Magic numbers are pervasive and intentional (`0xb8000`, `0x1F0`–`0x1F7`, `0x3F8`, `0x60`,
  `0x3D4`/`0x3D5`, `0xf4`). Leave a comment when adding one.

## Where things live

```
src/main.rs          boot order        src/commands.rs   command table + shell loop
src/lib.rs           modules + tests   src/shell.rs      line buffer, '?' splitting
src/parser.rs        Lisp              src/editor/       eden text editor
src/fat16.rs         filesystem        src/pong.rs       game
src/ide.rs           ATA PIO driver    src/vga_buffer.rs print!/println!, terminal
src/task/            async runtime     src/canvas.rs     raw VGA drawing
src/interrupts.rs    IDT, IRQs         src/serial.rs     host debug output
src/gdt.rs           GDT + TSS         src/memory.rs     paging, frames
src/allocator.rs     heap              src/power.rs      shutdown
src/input.rs         key inbox         src/hashmap.rs    UNUSED, has a bug
```

## Known-broken things — do not "discover" these as new

Full list with detail in [docs/FILE_MAP.md](docs/FILE_MAP.md#known-rough-edges). The ones most
likely to mislead you:

- `Cargo.toml` hard-codes `/home/sammy/SilOS/storage.bin` as an absolute path.
- `docs/lisp.md` documents `let`, `not`, `concat` and bitwise `| ^ << >>`, **none of which
  are implemented**. It also calls the `for` form a "while loop".
- `src/hashmap.rs` is unused and its `put` panics for any hash ≥ 128.
- `mkdir` creates a file. There are no directories.
- `parser::interpret` discards everything after the final `;`.
- Dead code that looks live: `LAUNCH_PONG`, `INPUT_QUEUE`, `add_processed_char`, `translate_addr`,
  `EmptyFrameAllocator`, `create_example_mapping`, `Dummy`, `EditorModel`.

## When you change things

- **Update the docs you invalidate.** Adding a command means updating
  [docs/USER_GUIDE.md](docs/USER_GUIDE.md#6-command-reference) and the `commands` node in
  `docs/project-graph.json`. Adding a Lisp form means updating
  [docs/lisp.md](docs/lisp.md) and the `parser` node's `builtins` /
  `specialForms` arrays. Adding a module means adding a node, its edges and a `docs/FILE_MAP.md`
  section.
- **Fixing something on the known-issues list?** Remove it from
  [docs/FILE_MAP.md](docs/FILE_MAP.md#known-rough-edges) and from `knownIssues` in the graph.
- **Prefer `cargo check` over `cargo build`** in a loop; the bootimage link step is slow.
- **Do not delete `storage.bin`** — it holds the user's files.
- **Do not commit or push** unless asked.

## Recipes

`docs/project-graph.json` → `recipes` has step-by-step procedures for: adding a shell command,
adding a Lisp builtin, adding a background task, adding a filesystem operation, and adding an
interrupt handler. Consult it before improvising.
