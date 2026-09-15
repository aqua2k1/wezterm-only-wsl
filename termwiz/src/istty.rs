//! Making it a little more convenient and safe to query whether
//! something is a terminal teletype or not.
//! This module defines the IsTty trait and the is_tty method to
//! return true if the item represents a terminal.
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use windows_sys::Win32::System::Console::GetConsoleMode;

/// Adds the is_tty method to types that might represent a terminal
pub trait IsTty {
    /// Returns true if the instance is a terminal teletype, false
    /// otherwise.
    fn is_tty(&self) -> bool;
}

#[cfg(not(windows))]
impl<S: std::io::IsTerminal> IsTty for S {
    fn is_tty(&self) -> bool {
        self.is_terminal()
    }
}

#[cfg(windows)]
impl<S: AsRawHandle> IsTty for S {
    fn is_tty(&self) -> bool {
        let mut mode = 0;
        let ok = unsafe { GetConsoleMode(self.as_raw_handle(), &mut mode) };
        ok == 1
    }
}
