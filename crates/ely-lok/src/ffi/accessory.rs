//! The worker stays out of the Dock (macOS).

use std::ffi::{CString, c_char, c_void};
use std::io;
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend();
}

/// The kernel's Aqua plugin; it defines `VCL_NSApplication`.
const PLUGIN: &str = "libvclplug_osxlo.dylib";
/// NSApplicationActivationPolicyAccessory: no Dock icon, no menu bar.
const ACCESSORY: isize = 1;

/// Makes the kernel's application an accessory before the kit starts.
pub fn accessory(install: &Path) -> io::Result<()> {
    // Loaded for good; the kernel later opens the same image.
    let path = install.join(PLUGIN);
    let c = CString::new(path.as_os_str().as_bytes()).map_err(|_| io::Error::other("the path holds a NUL"))?;
    if unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_LAZY | libc::RTLD_GLOBAL) }.is_null() {
        return Err(io::Error::other(format!("dlopen {}: {}", path.display(), super::dl_error())));
    }
    // The kernel's own class, so it reuses this instance.
    let class = unsafe { objc_getClass(c"VCL_NSApplication".as_ptr()) };
    if class.is_null() {
        return Err(io::Error::other(format!("{PLUGIN} has no VCL_NSApplication class")));
    }
    type Shared = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
    type Policy = unsafe extern "C" fn(*mut c_void, *mut c_void, isize) -> i8;
    let send = objc_msgSend as unsafe extern "C" fn();
    let shared: Shared = unsafe { std::mem::transmute(send) };
    let policy: Policy = unsafe { std::mem::transmute(send) };
    let app = unsafe { shared(class, sel_registerName(c"sharedApplication".as_ptr())) };
    if app.is_null() {
        return Err(io::Error::other("the kernel's application did not start"));
    }
    let set = unsafe { policy(app, sel_registerName(c"setActivationPolicy:".as_ptr()), ACCESSORY) };
    if set == 0 {
        return Err(io::Error::other("macOS refused the accessory policy"));
    }
    log::info!("lok: the worker is an accessory app");
    Ok(())
}
