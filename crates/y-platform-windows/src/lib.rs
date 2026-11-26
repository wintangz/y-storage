pub mod dep_scan;
pub mod icons;
pub mod paths;
pub mod registry;
pub mod scanner;
pub mod services;

use windows_sys::Win32::System::ProcessStatus::{
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

#[must_use]
pub fn query_process_memory_bytes() -> (u64, u64) {
    let mut pmc: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    let ok = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&mut pmc as *mut PROCESS_MEMORY_COUNTERS_EX).cast::<PROCESS_MEMORY_COUNTERS>(),
            pmc.cb,
        )
    };
    if ok != 0 {
        (pmc.WorkingSetSize as u64, pmc.PrivateUsage as u64)
    } else {
        (0, 0)
    }
}
