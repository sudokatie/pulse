//! CLAP host implementation

use std::ffi::{c_char, c_void, CStr};
use std::ptr;

/// CLAP version
pub const CLAP_VERSION_MAJOR: u32 = 1;
pub const CLAP_VERSION_MINOR: u32 = 2;
pub const CLAP_VERSION_REVISION: u32 = 0;

/// CLAP host struct matching the C ABI
#[repr(C)]
pub struct ClapHost {
    pub clap_version: ClapVersion,
    pub host_data: *mut c_void,
    pub name: *const c_char,
    pub vendor: *const c_char,
    pub url: *const c_char,
    pub version: *const c_char,
    pub get_extension: Option<unsafe extern "C" fn(host: *const ClapHost, extension_id: *const c_char) -> *const c_void>,
    pub request_restart: Option<unsafe extern "C" fn(host: *const ClapHost)>,
    pub request_process: Option<unsafe extern "C" fn(host: *const ClapHost)>,
    pub request_callback: Option<unsafe extern "C" fn(host: *const ClapHost)>,
}

/// CLAP version struct
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ClapVersion {
    pub major: u32,
    pub minor: u32,
    pub revision: u32,
}

impl Default for ClapVersion {
    fn default() -> Self {
        Self {
            major: CLAP_VERSION_MAJOR,
            minor: CLAP_VERSION_MINOR,
            revision: CLAP_VERSION_REVISION,
        }
    }
}

/// Host data stored with the host
pub struct HostData {
    pub name: std::ffi::CString,
    pub vendor: std::ffi::CString,
    pub url: std::ffi::CString,
    pub version: std::ffi::CString,
    pub restart_requested: std::sync::atomic::AtomicBool,
    pub process_requested: std::sync::atomic::AtomicBool,
    pub callback_requested: std::sync::atomic::AtomicBool,
}

impl HostData {
    pub fn new() -> Box<Self> {
        Box::new(Self {
            name: std::ffi::CString::new("Pulse").unwrap(),
            vendor: std::ffi::CString::new("Pulse Audio").unwrap(),
            url: std::ffi::CString::new("https://github.com/pulse").unwrap(),
            version: std::ffi::CString::new("1.0.0").unwrap(),
            restart_requested: std::sync::atomic::AtomicBool::new(false),
            process_requested: std::sync::atomic::AtomicBool::new(false),
            callback_requested: std::sync::atomic::AtomicBool::new(false),
        })
    }
}

impl ClapHost {
    /// Create a new CLAP host
    pub fn new(host_data: Box<HostData>) -> Box<Self> {
        let host_data_ptr = Box::into_raw(host_data);
        
        // Get string pointers before creating the struct
        let (name, vendor, url, version) = unsafe {
            let data = &*host_data_ptr;
            (
                data.name.as_ptr(),
                data.vendor.as_ptr(),
                data.url.as_ptr(),
                data.version.as_ptr(),
            )
        };
        
        Box::new(Self {
            clap_version: ClapVersion::default(),
            host_data: host_data_ptr as *mut c_void,
            name,
            vendor,
            url,
            version,
            get_extension: Some(host_get_extension),
            request_restart: Some(host_request_restart),
            request_process: Some(host_request_process),
            request_callback: Some(host_request_callback),
        })
    }
}

impl Drop for ClapHost {
    fn drop(&mut self) {
        if !self.host_data.is_null() {
            unsafe {
                let _ = Box::from_raw(self.host_data as *mut HostData);
            }
        }
    }
}

unsafe extern "C" fn host_get_extension(_host: *const ClapHost, _extension_id: *const c_char) -> *const c_void {
    // We don't support any extensions yet
    ptr::null()
}

unsafe extern "C" fn host_request_restart(host: *const ClapHost) {
    if let Some(data) = ((*host).host_data as *mut HostData).as_mut() {
        data.restart_requested.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

unsafe extern "C" fn host_request_process(host: *const ClapHost) {
    if let Some(data) = ((*host).host_data as *mut HostData).as_mut() {
        data.process_requested.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

unsafe extern "C" fn host_request_callback(host: *const ClapHost) {
    if let Some(data) = ((*host).host_data as *mut HostData).as_mut() {
        data.callback_requested.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clap_host_new() {
        let host_data = HostData::new();
        let host = ClapHost::new(host_data);
        
        assert_eq!(host.clap_version.major, CLAP_VERSION_MAJOR);
        assert!(!host.name.is_null());
    }

    #[test]
    fn test_clap_version_default() {
        let version = ClapVersion::default();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
    }
}
