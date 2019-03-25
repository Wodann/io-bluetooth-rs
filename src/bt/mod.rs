cfg_if! {
    if #[cfg(windows)] {
        mod windows;
        pub use self::windows::*;
    } else {
        compile_error!("io_bluetooth doesn't compile for this platform yet");
    }
}
