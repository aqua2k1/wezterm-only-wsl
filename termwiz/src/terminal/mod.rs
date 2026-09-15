//! An abstraction over a terminal device

#[cfg(not(windows))]
use crate::bail;
use crate::caps::probed::ProbeCapabilities;
use crate::caps::Capabilities;
#[cfg(windows)]
use crate::format_err;
use crate::input::InputEvent;
use crate::surface::Change;
use crate::Result;
#[cfg(windows)]
use num_traits::NumCast;
#[cfg(windows)]
use std::fmt::Display;
use std::time::Duration;

#[cfg(feature = "use_serde")]
use serde::Deserialize;
#[cfg(feature = "use_serde")]
use serde::Serialize;

#[cfg(windows)]
pub mod windows;

pub mod buffered;

#[cfg(windows)]
pub use self::windows::{WindowsTerminal, WindowsTerminalWaker as TerminalWaker};

#[cfg(not(windows))]
#[derive(Debug, Clone, Copy)]
pub struct TerminalWaker;

/// Represents the size of the terminal screen.
/// The number of rows and columns of character cells are expressed.
/// Some implementations populate the size of those cells in pixels.
// On Windows, GetConsoleFontSize() can return the size of a cell in
// logical units and we can probably use this to populate xpixel, ypixel.
// GetConsoleScreenBufferInfo() can return the rows and cols.
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSize {
    /// The number of rows of text
    pub rows: usize,
    /// The number of columns per row
    pub cols: usize,
    /// The width of a cell in pixels.  Some implementations never
    /// set this to anything other than zero.
    pub xpixel: usize,
    /// The height of a cell in pixels.  Some implementations never
    /// set this to anything other than zero.
    pub ypixel: usize,
}

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocking {
    DoNotWait,
    Wait,
}

/// `Terminal` abstracts over some basic terminal capabilities.
/// If the `set_raw_mode` or `set_cooked_mode` functions are used in
/// any combination, the implementation is required to restore the
/// terminal mode that was in effect when it was created.
pub trait Terminal {
    /// Raw mode disables input line buffering, allowing data to be
    /// read as the user presses keys, disables local echo, so keys
    /// pressed by the user do not implicitly render to the terminal
    /// output, and disables canonicalization of unix newlines to CRLF.
    fn set_raw_mode(&mut self) -> Result<()>;
    fn set_cooked_mode(&mut self) -> Result<()>;

    /// Enter the alternate screen.  The alternate screen will be left
    /// automatically when the `Terminal` is dropped.
    fn enter_alternate_screen(&mut self) -> Result<()>;

    /// Exit the alternate screen.
    fn exit_alternate_screen(&mut self) -> Result<()>;

    /// Queries the current screen size, returning width, height.
    fn get_screen_size(&mut self) -> Result<ScreenSize>;

    /// Returns a capability probing helper that will use escape
    /// sequences to attempt to probe information from the terminal
    fn probe_capabilities(&mut self) -> Option<ProbeCapabilities<'_>> {
        None
    }

    /// Sets the current screen size
    fn set_screen_size(&mut self, size: ScreenSize) -> Result<()>;

    /// Render a series of changes to the terminal output
    fn render(&mut self, changes: &[Change]) -> Result<()>;

    /// Flush any buffered output
    fn flush(&mut self) -> Result<()>;

    /// Check for a parsed input event.
    /// `wait` indicates the behavior in the case that no input is
    /// immediately available.  If wait is `None` then `poll_input`
    /// will not return until an event is available.  If wait is
    /// `Some(duration)` then `poll_input` will wait up to the given
    /// duration for an event before returning with a value of
    /// `Ok(None)`.  If wait is `Some(Duration::ZERO)` then the
    /// poll is non-blocking.
    ///
    /// The possible values returned as `InputEvent`s depend on the
    /// mode of the terminal.  Most values are not returned unless
    /// the terminal is set to raw mode.
    fn poll_input(&mut self, wait: Option<Duration>) -> Result<Option<InputEvent>>;

    fn waker(&self) -> TerminalWaker;
}

/// `SystemTerminal` is a concrete implementation of `Terminal`.
/// Ideally you wouldn't reference `SystemTerminal` in consuming
/// code.  This type is exposed for convenience if you are doing
/// something unusual and want easier access to the constructors.
#[cfg(windows)]
pub type SystemTerminal = WindowsTerminal;

#[cfg(not(windows))]
pub struct UnsupportedTerminal;
#[cfg(not(windows))]
pub type SystemTerminal = UnsupportedTerminal;

/// Construct a new instance of Terminal.
/// The terminal will have a renderer that is influenced by the configuration
/// in the provided `Capabilities` instance.
/// The Windows implementation explicitly opens `CONIN$` and `CONOUT$` so
/// that it yields a functioning console with minimal headaches.
#[cfg(windows)]
pub fn new_terminal(caps: Capabilities) -> Result<impl Terminal> {
    SystemTerminal::new(caps)
}

#[cfg(not(windows))]
pub fn new_terminal(_caps: Capabilities) -> Result<UnsupportedTerminal> {
    bail!("this build only supports the Windows terminal backend")
}

#[cfg(not(windows))]
impl Terminal for UnsupportedTerminal {
    fn set_raw_mode(&mut self) -> Result<()> {
        bail!("this build only supports the Windows terminal backend")
    }

    fn set_cooked_mode(&mut self) -> Result<()> {
        bail!("this build only supports the Windows terminal backend")
    }

    fn enter_alternate_screen(&mut self) -> Result<()> {
        bail!("this build only supports the Windows terminal backend")
    }

    fn exit_alternate_screen(&mut self) -> Result<()> {
        bail!("this build only supports the Windows terminal backend")
    }

    fn get_screen_size(&mut self) -> Result<ScreenSize> {
        bail!("this build only supports the Windows terminal backend")
    }

    fn set_screen_size(&mut self, _size: ScreenSize) -> Result<()> {
        bail!("this build only supports the Windows terminal backend")
    }

    fn render(&mut self, _changes: &[Change]) -> Result<()> {
        bail!("this build only supports the Windows terminal backend")
    }

    fn flush(&mut self) -> Result<()> {
        bail!("this build only supports the Windows terminal backend")
    }

    fn poll_input(&mut self, _wait: Option<Duration>) -> Result<Option<InputEvent>> {
        bail!("this build only supports the Windows terminal backend")
    }

    fn waker(&self) -> TerminalWaker {
        TerminalWaker
    }
}

#[cfg(windows)]
pub(crate) fn cast<T: NumCast + Display + Copy, U: NumCast>(n: T) -> Result<U> {
    num_traits::cast(n).ok_or_else(|| format_err!("{} is out of bounds for this system", n))
}
