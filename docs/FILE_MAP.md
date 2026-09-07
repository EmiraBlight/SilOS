# SilOS File Map

Every file in the repository, what it is responsible for, its key symbols, and what depends on it.
Line counts are approximate and drift; symbol names are the stable reference.

Machine-readable form: [project-graph.json](project-graph.json).

## Navigation index

| I want to change… | Go to |
| --- | --- |
| A shell command, or add a new one | [`src/commands.rs`](#srccommandsrs) |
| The Lisp language | [`src/parser.rs`](#srcparserrs) |
| How the prompt/line buffer behaves | [`src/shell.rs`](#srcshellrs) |
| Screen output, colours, scrolling | [`src/vga_buffer.rs`](#srcvga_bufferrs) |
| The text editor | [`src/editor/mod.rs`](#srceditormodrs) |
| Files on disk | [`src/fat16.rs`](#srcfat16rs) |
| Raw sector I/O | [`src/ide.rs`](#srcidesrs) |
| Task scheduling / async | [`src/task/executor.rs`](#srctaskexecutorrs) |
| Keyboard handling | [`src/interrupts.rs`](#srcinterruptsrs) → [`src/task/keyboard.rs`](#srctaskkeyboardrs) → [`src/input.rs`](#srcinputrs) |
| Exceptions, IRQs, the IDT | [`src/interrupts.rs`](#srcinterruptsrs) |
| Paging or the heap | [`src/memory.rs`](#srcmemoryrs), [`src/allocator.rs`](#srcallocatorrs) |
| Boot order | [`src/main.rs`](#srcmainrs) |
| Build/QEMU flags | [`Cargo.toml`](#cargotoml), [`.cargo/config.toml`](#cargoconfigtoml) |

---

## Entry points

### `src/main.rs`
*~71 lines · binary crate*

The kernel entry point. Declares `entry_point!(kernel_main)` and performs subsystem bring-up in a
strictly ordered sequence (see [ARCHITECTURE §2](ARCHITECTURE.md#2-boot-sequence)), then spawns
`keyboard::print_keypresses` and `commands::shell_task` on an `Executor` and runs it forever.

- **Key symbols:** `kernel_main`, `panic`
- **Depends on:** everything, via `myOS::` (it is a separate crate from the library)
- **Note:** contains a leftover smoke test that writes a 3-byte `TEST.TXT` on every boot and prints
  the result. Harmless, but it means every boot dirties the disk.

### `src/lib.rs`
*~128 lines · library crate root*

Module declarations plus the test infrastructure. Everything else in `src/` hangs off this.

- **Key symbols:** `hlt_loop`, `Testable`, `test_runner`, `test_panic_handler`, `QemuExitCode`,
  `exit_qemu`, `init`
- **`init()`** is the CPU bring-up trio: GDT, IDT, PIC remap, `sti`.
- **`exit_qemu`** writes to port `0xf4` (the `isa-debug-exit` device) — the mechanism behind both
  test results and `quit`.
- **`test_panic_handler`** prints in red to both serial and VGA before failing the run.

---

## Shell layer

### `src/commands.rs`
*~559 lines · the busiest file in the project*

Owns the command table, the shell task, and the implementation of every built-in command.

- **Key symbols:** `CommandFuture` (the central type alias), `COMMANDS`, `shell_task`,
  `run_cmd`, `init_cmds`, `get_command_list`, `COMMAND_PENDING`
- **Commands defined here:** `pong`, `clear`, `history`, `echo`, `parse`, `bind`, `read`, `write`,
  `show`, `formatd`, `mkdir`, `cat`, `run`, `edit`, `eden`, `quit`
- **Adding a command:** write `fn name(args: Vec<String>) -> CommandFuture` returning
  `Box::pin(async move { … })`, then register it in `init_cmds()`. `args[0]` is the command name
  itself.
- **`run_cmd` clones the `Arc` and drops the lock before calling** — required for reentrancy from
  Lisp's `sys`.
- **Gotcha:** `mkdir` creates a *file*, not a directory. There are no directories.

### `src/shell.rs`
*~61 lines*

The line editor state for the prompt: the in-progress command string and the history of past ones.

- **Key symbols:** `Shell`, `SHELL` (global `Mutex<Shell>`), `add`, `backspace`, `getcmd`, `clear`,
  `history`
- **`getcmd` is where `?` becomes the argument separator** — it splits the buffer on `"?"` and trims
  each field. Change argument syntax here and nowhere else.
- **Gotcha:** `clear()` pushes the current command into history *and* clears it; it is not a
  "discard" method despite the name.

### `src/programReturn.rs`
*~29 lines*

The result types every command returns.

- **Key symbols:** `Success { success_code, print_code }`, `ProcessError { error_code }`
- `print_code` controls whether the shell echoes the success string — most commands set it `false`
  and print their own output.
- **Note:** the filename is `camelCase`, which is why `Cargo.toml` sets
  `[lints.rust] nonstandard_style = "allow"`.

### `src/parser.rs`
*~1041 lines · the Lisp interpreter*

A complete tree-walking Lisp: tokenizer, reader, evaluator, environment and standard library.

- **Key symbols:** `RispExp` (the value enum), `RispEnv`, `RispErr`, `RispLambda`, `tokenize`,
  `parse`, `parse_atom`, `eval`, `default_env`, `env_for_lambda`, `parse_eval`, `interpret`
- **`interpret(expr: Vec<String>)`** is the public entry point and is itself registered as the
  `parse` command. `expr[1]` is the source; `expr[2..]` are arguments injected as `n0`/`n1`
  (numbers) and `b0`/`b1` (booleans).
- **Special forms handled in `eval`:** `if`, `def`, `fn`, `sys`, `quote`, `and`, `or`, `for`,
  `append`, `pop`, `mset`, `mdel`, `do`, `error`
- **Builtins in `default_env`:** `+ - * / & = != > >= < <=`, `list`, `[]`, `len`, `!!`, `map`,
  `mkeys`
- **`eval` returns a boxed future rather than being an `async fn`** so it can recurse — do not
  "simplify" this.
- **`sys`** evaluates its arguments, stringifies them and calls `commands::run_cmd`. This is the
  Lisp→kernel bridge and the reason `parser` and `commands` are mutually dependent.
- **Gotcha:** `interpret` splits the source on `;` and then calls `statments.pop()`, discarding the
  final fragment. Source must therefore end with `;`, and anything after the last `;` is silently
  dropped.
- **See also:** [lisp.md](lisp.md), and its
  [implementation-status note](lisp.md#implementation-status) — several documented
  features are not implemented.

---

## Async runtime

### `src/task/mod.rs`
*~38 lines*

- **Key symbols:** `Task`, `TaskId`
- `Task::new` pins any `Future<Output = ()>` into a box; `TaskId` is a monotonic `AtomicU64`.

### `src/task/executor.rs`
*~129 lines*

The cooperative scheduler.

- **Key symbols:** `Executor` (`new`, `spawn`, `run`, `run_ready_tasks`, `sleep_if_idle`),
  `TaskWaker`, `yield_now`, `YieldNow`
- **`yield_now()` is the cooperation primitive.** Every long-running loop in the kernel must await
  it or the machine hangs — there is no preemption.
- **`sleep_if_idle`** performs the `cli` / check-queue / `sti;hlt` sequence that avoids losing a
  wakeup that arrives between the check and the halt.
- **Limit:** the ready queue is a fixed `ArrayQueue` of 100; `spawn` and `wake` both `expect` on
  overflow, so exceeding it panics.

### `src/task/keyboard.rs`
*~86 lines*

Turns the interrupt handler's raw scancodes into decoded key events.

- **Key symbols:** `add_scancode` (called from the IRQ — must stay lock-free), `ScancodeStream`
  (a `futures_util::Stream<Item = u8>`), `print_keypresses` (the decoder task), `WAKER`,
  `SCANCODE_QUEUE`
- **`print_keypresses`** decodes with `pc_keyboard` and pushes `KeyEvent`s onto
  `input::KEY_EVENT_QUEUE`. Despite the name it prints nothing.
- **Dead code:** `INPUT_QUEUE` and `add_processed_char` are never read.

### `src/input.rs`
*~13 lines*

The global inbox that foreground applications drain.

- **Key symbols:** `KEY_EVENT_QUEUE` (`Mutex<VecDeque<KeyEvent>>`), `pop_key()`
- **Contract:** exactly one consumer at a time. `shell_task`, `run_editor` and `PongGame::run` all
  call `pop_key()`; only the innermost currently-awaited one is live.

---

## Devices and output

### `src/vga_buffer.rs`
*~249 lines*

The primary text output device and the home of `print!`/`println!`.

- **Key symbols:** `print!`, `println!`, `_print`, `Writer`, `WRITER`, `Color`, `ColorCode`,
  `ScreenChar`, `BUFFER_WIDTH`/`BUFFER_HEIGHT` (80×25), `_backspace`, `write_char_at`,
  `clear_screen`, `update_cursor`
- Writes always land on row 24; `new_line` scrolls the whole buffer up one row.
- **`_print` wraps access in `without_interrupts`** — required, or a print inside an interrupt
  handler deadlocks against an interrupted print.
- `update_cursor` drives the real hardware cursor via ports `0x3D4`/`0x3D5`; used by the editor.
- **Gotcha:** `Writer::clear` only clears rows 0–23, leaving row 24 intact.
- Contains three inline `#[test_case]`s.

### `src/canvas.rs`
*~42 lines*

A second, stateless view of the VGA buffer for random-access drawing.

- **Key symbols:** `TextCanvas` (`new`, `set_char`, `clear`), `SCREEN_WIDTH`, `SCREEN_HEIGHT`,
  `VGA_TEXT_ADDR`
- **Only consumer:** `pong.rs`. It aliases `0xb8000` with `WRITER`, which is safe only because the
  two are never active simultaneously.

### `src/serial.rs`
*~41 lines*

UART 16550 on port `0x3F8`, for output the host can capture. This is how test results escape QEMU.

- **Key symbols:** `SERIAL1`, `serial_print!`, `serial_println!`

### `src/ide.rs`
*~124 lines*

A polling PIO ATA driver for the primary bus.

- **Key symbols:** `AtaDrive` (`new`, `read_sector`, `write_sector_bytes`, `get_max_lba`,
  `flush_cache`), `IDE` (global `Mutex<AtaDrive>`)
- Ports `0x1F0`–`0x1F7`; commands `0x20` (read), `0x30` (write), `0xE7` (flush), `0xEC` (identify).
- **Blocking by design:** every operation busy-waits on BSY/DRQ with no timeout, so a
  non-responding drive hangs the kernel. The ATA IRQ handler exists but is not used for completion.
- Sectors are fixed at 512 bytes throughout.

### `src/fat16.rs`
*~469 lines*

The FAT16 filesystem, implemented directly against `ide`.

- **Key symbols:** `Bpb`, `DirectoryEntry`, `Fat16FileSystem`, `FS` (`Mutex<Option<…>>`), `init`
- **Public operations:** `read_file`, `write_new_file`, `overwrite_file`, `find_file`,
  `find_file_location`, `format_drive`
- **Internal:** `cluster_to_lba`, `get_fat_entry`, `set_fat_entry`, `find_free_cluster`,
  `find_empty_root_slot`, `free_cluster_chain`
- **`init` reads LBA 0 and validates `bytes_per_sector == 512`**; on failure `FS` stays `None` and
  every filesystem command reports "File system not mounted".
- **Scope limits:** root directory only, no subdirectories, no long names, no delete, 8.3 names
  only (upper-cased and space-padded by the caller).
- **Lock rule:** these methods take `IDE.lock()` internally. Never hold `IDE.lock()` across a call
  into `fat16`.

### `src/power.rs`
*~47 lines*

Clean shutdown.

- **Key symbols:** `shutdown()`, `teardown()`
- `teardown` acquires and drops `FS.lock()` (proving no I/O is in flight), flushes the drive cache,
  and only then masks interrupts — the ordering is deliberate and documented in the source: a
  spinlock held past `cli` could never be released.
- Tries QEMU (`0x604`), Bochs (`0xB004`) and VirtualBox (`0x4004`) power-off ports, then
  `isa-debug-exit`, then halts forever. Real-hardware ACPI is not implemented.

---

## CPU and memory

### `src/gdt.rs`
*~53 lines*

- **Key symbols:** `GDT`, `TSS`, `Selectors`, `DOUBLE_FAULT_IST_INDEX`, `init`
- Provides a 20 KiB dedicated stack for the double-fault handler via IST entry 0, which is what
  makes stack-overflow detection possible. **`gdt::init()` must run before `init_idt()`.**

### `src/interrupts.rs`
*~115 lines*

The IDT and PIC configuration.

- **Key symbols:** `IDT`, `init_idt`, `PICS`, `PIC_1_OFFSET` (32), `PIC_2_OFFSET` (40),
  `InterruptIndex`, `keyboard_interrupt_handler`, `timer_interrupt_handler`,
  `ata_interrupt_handler`, `page_fault_handler`, `breakpoint_handler`, `double_fault_handler`
- **The keyboard handler is the one that matters:** read port `0x60`, `add_scancode`, EOI. It must
  perform no locking beyond the lock-free queue.
- **Dead code:** `LAUNCH_PONG` is written by `commands::pong` but never read.

### `src/memory.rs`
*~129 lines*

Paging and physical frame allocation.

- **Key symbols:** `init` (builds an `OffsetPageTable`), `BootInfoFrameAllocator`,
  `active_level_4_table`, `translate_addr`, `EmptyFrameAllocator`, `create_example_mapping`
- **`BootInfoFrameAllocator::allocate_frame` is O(n)** — it re-runs the usable-frames iterator and
  takes the *n*th element each call, so heap init is quadratic. Fine at this scale; the first thing
  to fix if allocation gets slow.
- **Never frees frames.** There is no deallocation path.
- **Dead code:** `translate_addr`, `EmptyFrameAllocator` and `create_example_mapping` are tutorial
  leftovers with no callers.

### `src/allocator.rs`
*~54 lines*

The kernel heap.

- **Key symbols:** `ALLOCATOR` (`LockedHeap`, the `#[global_allocator]`), `HEAP_START`
  (`0x4444_4444_0000`), `HEAP_SIZE` (8 MiB), `init_heap`, `Dummy`
- `init_heap` maps 2048 pages then hands the region to `linked_list_allocator`.
- **Nothing may allocate before this runs.**
- **Dead code:** `Dummy` is an unused example allocator.

### `src/hashmap.rs`
*~87 lines*

A hand-written hash map. **Entirely unused** — nothing imports it; the kernel uses
`alloc::collections::BTreeMap` throughout.

- **Key symbols:** `HashMap`, `Tuple`, `Hashable`
- **Bug:** `put` and `remove` index `self.buckets[key.hash()]` without `% buckets.len()`, while
  `get` does apply the modulo. Any hash ≥ 128 panics on insert. Fix before using.

---

## Applications

### `src/editor/mod.rs`
*~279 lines*

`eden`, the full-screen text editor.

- **Key symbols:** `run_editor` (the async entry point), `Editor` (`insert_char`, `insert_newline`,
  `backspace`, `draw`, `move_cursor_*`), `TerminalDevice` (trait), `VgaTerminal`,
  `format_fat16_name`
- Loads the file on entry (empty if absent), edits an in-memory `Vec<String>` of lines, and on
  <kbd>Esc</kbd> joins with `\n` and writes back via `overwrite_file` or `write_new_file`.
- Rendering goes through the `TerminalDevice` trait, so the editor core is testable against a mock
  backend without a VGA buffer.
- **Limits:** no scrolling (draws from line 0, so content past row 24 is invisible), no
  quit-without-saving, no undo, no line-length clamping to 80 columns.
- **Dead code:** `EditorModel` is an unused richer struct with `row_offset`/`col_offset` fields —
  evidently the intended scrolling design.

### `src/pong.rs`
*~183 lines*

Two-player Pong on the text console.

- **Key symbols:** `PongGame` (`new`, `run`, `update`, `wait`)
- Reads raw `KeyEvent`s (not `DecodedKey`) so it can track key-down/key-up for held paddles:
  W/S for player 1, O/L for player 2, <kbd>Esc</kbd> to quit.
- **`wait()` is a 1,000,000-iteration `nop` spin**, so game speed depends on host CPU speed rather
  than wall-clock time.
- Draws through `TextCanvas`, not `WRITER`.

---

## Tests

All tests boot a real kernel image in QEMU and report over the serial port.

| File | Harness | Purpose |
| --- | --- | --- |
| `tests/basic_boot.rs` | default | `println!` works on a bare boot with no subsystem init |
| `tests/heap_allocations.rs` | default | Box/Vec allocation, reallocation, and reuse of freed memory |
| `tests/stack_overflow.rs` | `harness = false` | Infinite recursion triggers the double-fault handler on the IST stack rather than a triple fault |
| `tests/should_panic.rs` | `harness = false` | A failing assertion is correctly detected as a panic |

The two `harness = false` binaries each contain exactly one test because the kernel does not
survive it.

---

## Configuration and assets

### `Cargo.toml`
Package manifest. Beyond dependencies, three sections carry real behaviour:

- **`[package.metadata.bootimage]`** — QEMU flags. `run-args` attaches `storage.bin` as IDE disk 1
  **using a hard-coded absolute path** (`/home/sammy/SilOS/storage.bin`); this must be edited for
  any other checkout. `test-args` runs headless with serial on stdio;
  `test-success-exit-code = 33` corresponds to `QemuExitCode::Success` (`0x10`) shifted by the
  `isa-debug-exit` protocol.
- **`[[test]]` entries** — mark `should_panic` and `stack_overflow` as `harness = false`.
- **`[lints.rust] nonstandard_style = "allow"`** — permits `myOS` and `programReturn.rs`.
- Panic strategy is `abort` in both profiles; there is no unwinding.

### `rust-toolchain.toml`
Pins nightly `2026-07-01` with `rust-src` and `llvm-tools-preview`. Unstable features in use:
`abi_x86_interrupt`, `custom_test_frameworks`, `build-std`, edition 2024.

### `.cargo/config.toml`
Sets the default target to `x86_64-myOs.json`, enables `build-std` for `core`/`compiler_builtins`/
`alloc`, turns on `panic-abort-tests`, and registers `bootimage runner` as the runner for
`cfg(target_os = "none")` — this is what makes `cargo run` boot QEMU.

### `x86_64-myOs.json`
The custom bare-metal target: `os: none`, no red zone, SSE/MMX disabled with soft floats (so the
kernel never touches FP registers in an interrupt), `rust-lld`, panic strategy abort.

### `storage.bin`
The FAT16 disk image. Git-ignored, not created for you — see the
[user guide](USER_GUIDE.md#2-the-disk-image).

### `qemu-system-x86_64`
An empty zero-byte file in the repo root. It has no purpose and appears to be an accident;
the real binary is found on `PATH`.

### `docs/lisp.md`
The Lisp language reference. Note its
[implementation-status section](lisp.md#implementation-status) — several
documented features do not exist in `parser.rs`.

---

## Known rough edges

Recorded so agents do not mistake them for things to "discover" or accidentally depend on.

| Location | Issue |
| --- | --- |
| `Cargo.toml` | `storage.bin` path is absolute and machine-specific — breaks on any other checkout |
| `docs/lisp.md` | Documents `let`, `not`, `concat` and bitwise `\| ^ << >>`; none are implemented |
| `docs/lisp.md` | Calls the loop form a "while loop" but the keyword is `for` |
| `src/hashmap.rs` | `put`/`remove` omit the `% buckets.len()` that `get` applies — panics on hash ≥ 128. Module is unused |
| `src/commands.rs` | `mkdir` creates a file, not a directory |
| `src/parser.rs` | `interpret` discards everything after the final `;` |
| `src/vga_buffer.rs` | `Writer::clear` skips row 24 |
| `src/main.rs` | Writes a leftover `TEST.TXT` smoke-test file on every boot |
| `src/memory.rs` | `allocate_frame` is O(n) per call; frames are never freed |
| `src/task/executor.rs` | Ready queue is capped at 100 and panics on overflow |
| `src/ide.rs` | Every I/O busy-waits with no timeout; a stuck drive hangs the kernel |
| `src/editor/mod.rs` | No scrolling, no quit-without-save, no 80-column clamp |
| `src/interrupts.rs` | `LAUNCH_PONG` is written but never read |
| `src/task/keyboard.rs` | `INPUT_QUEUE` / `add_processed_char` are unreachable |
| `src/memory.rs`, `src/allocator.rs`, `src/editor/mod.rs` | `translate_addr`, `EmptyFrameAllocator`, `create_example_mapping`, `Dummy`, `EditorModel` are unused leftovers |
| root | `qemu-system-x86_64` is a stray empty file |
