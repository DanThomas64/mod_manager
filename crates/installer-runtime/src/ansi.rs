//! Minimal, dependency-free ANSI styling for the installer's console output.
//! Colors are skipped entirely when stdout isn't a real terminal (e.g.
//! piped/redirected), and on Windows we explicitly enable virtual terminal
//! processing first since older `cmd.exe` sessions don't have it on by
//! default — if that call fails, we fall back to plain text rather than
//! printing raw escape codes.

use std::io::{self, IsTerminal};
use std::sync::OnceLock;

fn supported() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        if !io::stdout().is_terminal() {
            return false;
        }
        #[cfg(windows)]
        {
            enable_windows_vt()
        }
        #[cfg(not(windows))]
        {
            true
        }
    })
}

#[cfg(windows)]
fn enable_windows_vt() -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(n_std_handle: i32) -> *mut core::ffi::c_void;
        fn GetConsoleMode(h_console_handle: *mut core::ffi::c_void, lp_mode: *mut u32) -> i32;
        fn SetConsoleMode(h_console_handle: *mut core::ffi::c_void, dw_mode: u32) -> i32;
    }

    const STD_OUTPUT_HANDLE: i32 = -11;
    const INVALID_HANDLE_VALUE: *mut core::ffi::c_void = -1isize as *mut core::ffi::c_void;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut mode: u32 = 0;
        if GetConsoleMode(handle, &mut mode) == 0 {
            return false;
        }
        if mode & ENABLE_VIRTUAL_TERMINAL_PROCESSING != 0 {
            return true;
        }
        SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) != 0
    }
}

fn wrap(code: &str, s: &str) -> String {
    if supported() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn bold(s: &str) -> String {
    wrap("1", s)
}
pub fn dim(s: &str) -> String {
    wrap("2", s)
}
pub fn cyan_bold(s: &str) -> String {
    wrap("1;36", s)
}
pub fn green_bold(s: &str) -> String {
    wrap("1;32", s)
}
pub fn yellow_bold(s: &str) -> String {
    wrap("1;33", s)
}
pub fn red_bold(s: &str) -> String {
    wrap("1;31", s)
}
