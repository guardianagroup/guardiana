//! Work done in the background, at the lowest cost to what the person is doing (1.0.55, «ligero»; the owner, 2 Nov
//! 2026, 16:53: «se me traba», the simple PCs most).
//!
//! The two slow looks start a read-only PowerShell every few minutes (the watch's `vigilancia.ps1`, the vigía's
//! `defensa.ps1`), already at below-normal priority. That only orders the processor among the programs that want it at
//! once; here Windows is also told, where it can be, two more things about that process, both documented
//! (`SetProcessInformation`):
//!
//! - efficiency mode (`ProcessPowerThrottling`, «EcoQoS», Windows 10 1709 and later): its threads run at the
//!   processor's efficient speed and, on processors with two kinds of cores, on the efficient ones;
//! - low memory priority (`ProcessMemoryPriority`): when memory runs short, its pages are the first to leave, not
//!   those of the program in front of the person.
//!
//! Neither changes what the look reads or finds; it may take a little longer, within the limit it already had. Where
//! Windows does not know one of them it says no, and the look runs as before. Never for the resolver: names must keep
//! coming back at once.
//!
//! `unsafe` is denied in this project unless justified, and this is the justification: `SetProcessInformation` is a C
//! function with no safe wrapper. It is given the child's own handle, which lives as long as the `Child`, and a
//! structure of the size it is told.

#![allow(unsafe_code)]

use std::os::windows::io::AsRawHandle;

use windows_sys::Win32::System::Threading::{
    ProcessMemoryPriority, ProcessPowerThrottling, SetProcessInformation,
    MEMORY_PRIORITY_INFORMATION, MEMORY_PRIORITY_LOW, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
    PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
};

/// Efficiency mode and low memory priority for a process this program just started for a background look. Returns
/// whether Windows took each one (efficiency, memory).
pub(crate) fn de_fondo(hijo: &std::process::Child) -> (bool, bool) {
    let h = hijo.as_raw_handle();
    let eco = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    };
    // SAFETY: `h` is the child's handle, open while `hijo` lives; `eco` is ours and its size goes with it.
    let eco_ok = unsafe {
        SetProcessInformation(
            h,
            ProcessPowerThrottling,
            (&raw const eco).cast(),
            u32::try_from(std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>()).unwrap_or(0),
        )
    } != 0;
    let memoria = MEMORY_PRIORITY_INFORMATION {
        MemoryPriority: MEMORY_PRIORITY_LOW,
    };
    // SAFETY: as above.
    let memoria_ok = unsafe {
        SetProcessInformation(
            h,
            ProcessMemoryPriority,
            (&raw const memoria).cast(),
            u32::try_from(std::mem::size_of::<MEMORY_PRIORITY_INFORMATION>()).unwrap_or(0),
        )
    } != 0;
    (eco_ok, memoria_ok)
}

#[cfg(test)]
mod tests {
    /// On a real Windows: a child started for a look takes both, and keeps running and ending as it would.
    #[test]
    fn un_hijo_de_fondo_corre_y_acaba_igual() {
        let raiz = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_owned());
        let hijo = std::process::Command::new(format!("{raiz}\\System32\\cmd.exe"))
            .args(["/c", "exit", "7"])
            .spawn();
        assert!(hijo.is_ok(), "{hijo:?}");
        let Ok(mut hijo) = hijo else {
            return;
        };
        let (eco, memoria) = super::de_fondo(&hijo);
        let fin = hijo.wait();
        assert!(fin.is_ok_and(|s| s.code() == Some(7)));
        // Whether Windows took them depends on its version and on the child still running when asked: said, not
        // required (the look runs either way).
        println!("eficiencia {eco}, memoria baja {memoria}");
    }
}
