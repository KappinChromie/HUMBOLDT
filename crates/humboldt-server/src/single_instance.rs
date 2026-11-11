#[cfg(windows)]
pub struct SingleInstanceMutex {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl SingleInstanceMutex {
    /// Attempts to acquire the named Win32 mutex.
    /// Returns Ok(Some(guard)) if acquired, Ok(None) if already held by another process.
    pub fn try_acquire(name: &str) -> std::result::Result<Option<Self>, u32> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
        use windows_sys::Win32::System::Threading::CreateMutexW;

        let wide_name: Vec<u16> = OsStr::new(name).encode_wide().chain(Some(0)).collect();
        unsafe {
            let handle = CreateMutexW(std::ptr::null(), 0, wide_name.as_ptr());
            if handle.is_null() {
                return Err(GetLastError());
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                windows_sys::Win32::Foundation::CloseHandle(handle);
                return Ok(None);
            }
            Ok(Some(Self { handle }))
        }
    }
}

#[cfg(windows)]
impl Drop for SingleInstanceMutex {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_null() {
                windows_sys::Win32::Foundation::CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(not(windows))]
pub struct SingleInstanceMutex;

#[cfg(not(windows))]
impl SingleInstanceMutex {
    pub fn try_acquire(_name: &str) -> std::result::Result<Option<Self>, u32> {
        Ok(Some(Self))
    }
}

/// Displays a native Win32 error dialog box on Windows (or stderr if headless).
pub fn show_native_error_dialog(title: &str, message: &str) {
    if crate::launcher::is_headless() {
        eprintln!("[{}] {}", title, message);
        return;
    }

    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

        let wide_title: Vec<u16> = OsStr::new(title).encode_wide().chain(Some(0)).collect();
        let wide_msg: Vec<u16> = OsStr::new(message).encode_wide().chain(Some(0)).collect();

        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                wide_msg.as_ptr(),
                wide_title.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }

    #[cfg(not(windows))]
    {
        eprintln!("[{}] {}", title, message);
    }
}
