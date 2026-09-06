use x86_64::instructions::port::Port;

use crate::{QemuExitCode, exit_qemu, hlt_loop, println};

/// Shut the machine down.
///
/// Flushes the disk, masks interrupts, then tries the well-known emulator
/// power-off ports in turn. On real hardware none of them respond (proper ACPI
/// shutdown would mean parsing the FADT), so we fall back to halting forever.
pub fn shutdown() -> ! {
    teardown();

    unsafe {
        // QEMU (i440fx/q35): ACPI PM1a_CNT with SLP_TYP=S5 | SLP_EN.
        let mut qemu: Port<u16> = Port::new(0x604);
        qemu.write(0x2000);

        // Bochs and very old QEMU.
        let mut bochs: Port<u16> = Port::new(0xB004);
        bochs.write(0x2000);

        // VirtualBox.
        let mut vbox: Port<u16> = Port::new(0x4004);
        vbox.write(0x3400);
    }

    // Last resort for QEMU: the isa-debug-exit device, if it was attached.
    exit_qemu(QemuExitCode::Success);

    println!("It is now safe to power off your computer.");
    hlt_loop();
}

/// Quiesce the system: make sure nothing is mid-write to the disk and stop
/// interrupts from firing.
///
/// The locks are spinlocks, so they must be taken *before* interrupts are
/// masked - otherwise a lock held by a preempted task could never be released.
fn teardown() {
    // Waiting for the lock is the point: it means no filesystem operation is
    // in flight. The filesystem keeps no dirty cache of its own.
    drop(crate::fat16::FS.lock());

    crate::ide::IDE.lock().flush_cache();

    x86_64::instructions::interrupts::disable();
}
