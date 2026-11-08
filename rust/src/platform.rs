//! Platform glue: Android logging and the request flags handed back to Kotlin each frame.

/// Bits returned from `frame()`; mirrored in NativeBridge.kt.
pub const REQ_EXIT: i32 = 1;
pub const REQ_HAPTIC_TICK: i32 = 2;
pub const REQ_HAPTIC_CONFIRM: i32 = 4;
pub const REQ_HAPTIC_REJECT: i32 = 8;
pub const REQ_HAPTIC_HEAVY: i32 = 16;
/// Ask Kotlin to look for a newer release on GitHub (it rate-limits and answers via setUpdate).
pub const REQ_CHECK_UPDATE: i32 = 64;
/// Open the newest release's download in the browser.
pub const REQ_OPEN_UPDATE: i32 = 128;

#[cfg(target_os = "android")]
#[link(name = "log")]
extern "C" {
    fn __android_log_write(prio: i32, tag: *const std::os::raw::c_char, text: *const std::os::raw::c_char) -> i32;
}

pub fn log(msg: &str) {
    #[cfg(target_os = "android")]
    {
        let tag = b"Ribbit\0";
        let text = std::ffi::CString::new(msg.replace('\0', " ")).unwrap_or_default();
        unsafe {
            __android_log_write(4, tag.as_ptr() as *const _, text.as_ptr());
        }
    }
    #[cfg(not(target_os = "android"))]
    eprintln!("[Ribbit] {msg}");
}

#[macro_export]
macro_rules! logi {
    ($($t:tt)*) => { $crate::platform::log(&format!($($t)*)) };
}
