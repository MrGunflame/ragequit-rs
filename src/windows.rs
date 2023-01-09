use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;

pub(crate) fn init() {
    unsafe {
        let _ = SetConsoleCtrlHandler(Some(terminate), 1);
    }
}

extern "system" fn terminate(_: u32) -> BOOL {
    super::terminate();
    1
}
