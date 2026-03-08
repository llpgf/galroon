//! Platform-specific handling (R12, R14).

pub mod macos;
pub mod windows;

/// Perform platform-specific initialization.
pub fn init() {
    #[cfg(windows)]
    windows::init();

    #[cfg(target_os = "macos")]
    macos::init();
}
