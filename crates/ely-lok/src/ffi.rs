use std::{
    ffi::{CString, c_char, c_void},
    io,
    path::Path,
};

/// The kit vtable: nSize, then 26 slots in header order.
#[repr(C)]
struct KitClass {
    n_size: usize,
    entries: [*mut c_void; 26],
}

/// The document vtable: nSize, then 81 slots in header order.
#[repr(C)]
struct DocClass {
    n_size: usize,
    entries: [*mut c_void; 81],
}

#[repr(C)]
struct Kit {
    class: *const KitClass,
}

#[repr(C)]
struct Doc {
    class: *const DocClass,
}

type Hook2 = unsafe extern "C" fn(*const i8, *const i8) -> *mut Kit;

/// A loaded LibreOfficeKit instance.
pub struct Lok {
    kit: *mut Kit,
    _lib: Library,
}

/// A LOK event callback (LibreOfficeKitCallback).
pub type Callback = unsafe extern "C" fn(i32, *const c_char, *mut c_void);

/// Documents move between the registry and handlers on one thread.
unsafe impl Send for Document {}

/// A loaded document inside one Lok.
pub struct Document {
    doc: *mut Doc,
    /// The kernel view the load made.
    view: i32,
}

/// Calls vtable slot `index` as `F`.
macro_rules! call {
    ($class:expr, $index:expr, $type:ty, $($arg:expr),*) => {{
        unsafe {
            let entry = (*$class).entries[$index];
            let f: $type = std::mem::transmute(entry);
            f($($arg),*)
        }
    }};
}

mod document;
mod slideshow;
pub use document::{Raw, close_window};

impl Lok {
    /// LOK_FEATURE_PART_IN_INVALIDATION_CALLBACK.
    pub const PART_IN_INVALIDATION: u64 = 1 << 2;
    /// LOK_FEATURE_NO_TILED_ANNOTATIONS: comments come as callbacks.
    pub const NO_TILED_ANNOTATIONS: u64 = 1 << 3;
    /// LOK_FEATURE_DOCUMENT_PASSWORD: the client answers.
    pub const DOCUMENT_PASSWORD: u64 = 1 << 0;

    /// dlopens the LOK library and hooks with the profile URL.
    pub fn hook(install: &Path, profile_url: &str) -> io::Result<Self> {
        let install = std::fs::canonicalize(install)?;
        let install = install.as_path();
        let install_str = install.to_string_lossy().into_owned();
        let install_c = cstring(&install_str)?;
        let profile_c = cstring(profile_url)?;
        let lib = Library::open(install)?;
        let hook: Hook2 = unsafe {
            lib.symbol(b"libreofficekit_hook_2\0")
                .or_else(|_| lib.symbol(b"libreofficekit_hook\0"))
                .map_err(|error| io::Error::new(io::ErrorKind::NotFound, error))?
        };
        let kit = unsafe { hook(install_c.as_ptr(), profile_c.as_ptr()) };
        if !kit.is_null() {
            type F = unsafe extern "C" fn(*mut Kit, u64);
            let features =
                Self::PART_IN_INVALIDATION | Self::NO_TILED_ANNOTATIONS | Self::DOCUMENT_PASSWORD;
            call!((*kit).class, 7, F, kit, features);
        }
        if kit.is_null() {
            return Err(io::Error::other(format!(
                "lok hook failed for {install_str}"
            )));
        }
        log::info!("lok: hooked {}", install_str);
        Ok(Self { kit, _lib: lib })
    }

    /// The BuildId string of the hooked library.
    pub fn build_id(&self) -> String {
        let class = unsafe { (*self.kit).class };
        type F = unsafe extern "C" fn(*mut Kit) -> *mut i8;
        let raw = call!(class, 9, F, self.kit);
        unsafe { std::ffi::CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned()
    }

    /// Loads a document; failures surface (LOK-09).
    pub fn load(&self, url: &str, options: &str) -> io::Result<Document> {
        let class = unsafe { (*self.kit).class };
        let url_c = cstring(url)?;
        let options_c = cstring(options)?;
        type F = unsafe extern "C" fn(*mut Kit, *const i8, *const i8) -> *mut Doc;
        let doc = call!(class, 3, F, self.kit, url_c.as_ptr(), options_c.as_ptr());
        if doc.is_null() {
            return Err(io::Error::other(self.error()));
        }
        type View = unsafe extern "C" fn(*mut Doc) -> i32;
        let doc_class = unsafe { (*doc).class };
        let view = call!(doc_class, 28, View, doc);
        log::info!("lok: loaded into view {view}");
        Ok(Document { doc, view })
    }

    /// The kit's current error text.
    fn error(&self) -> String {
        let class = unsafe { (*self.kit).class };
        type Get = unsafe extern "C" fn(*mut Kit) -> *mut i8;
        type Free = unsafe extern "C" fn(*mut i8);
        let raw = call!(class, 2, Get, self.kit);
        if raw.is_null() {
            return "unknown LOK error".into();
        }
        let text = unsafe { std::ffi::CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned();
        call!(class, 4, Free, raw);
        text
    }

    /// Registers the kit-wide callback.
    pub fn register_callback(&self, callback: Callback) {
        let class = unsafe { (*self.kit).class };
        type F = unsafe extern "C" fn(*mut Kit, Callback, *mut c_void);
        call!(class, 5, F, self.kit, callback, std::ptr::null_mut());
    }

    /// Answers a password question; None refuses.
    pub fn set_password(&self, url: &str, password: Option<&str>) {
        let class = unsafe { (*self.kit).class };
        let url_c = cstring(url).expect("a url without NUL");
        let password_c = password.map(|password| cstring(password).expect("no NUL"));
        let password_ptr = password_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
        type F = unsafe extern "C" fn(*mut Kit, *const c_char, *const c_char);
        call!(class, 8, F, self.kit, url_c.as_ptr(), password_ptr);
    }

    /// Sets a kit option such as `author`.
    pub fn set_option(&self, option: &str, value: &str) -> io::Result<()> {
        let class = unsafe { (*self.kit).class };
        let (option, value) = (cstring(option)?, cstring(value)?);
        type F = unsafe extern "C" fn(*mut Kit, *const c_char, *const c_char);
        call!(class, 14, F, self.kit, option.as_ptr(), value.as_ptr());
        Ok(())
    }

    /// Waits for the kit's threads to finish (LOK-10).
    pub fn join_threads(&self) -> bool {
        let class = unsafe { (*self.kit).class };
        type F = unsafe extern "C" fn(*mut Kit) -> i32;
        call!(class, 20, F, self.kit) != 0
    }

    /// Runs a macro by URL, for the debug probe.
    pub fn run_macro(&self, url: &str) -> i32 {
        let class = unsafe { (*self.kit).class };
        let url_c = cstring(url).expect("a url");
        type F = unsafe extern "C" fn(*mut Kit, *const i8) -> i32;
        call!(class, 10, F, self.kit, url_c.as_ptr())
    }

    #[cfg(target_os = "macos")]
    /// Answers the kernel's "input waiting?" without a run-loop peek.
    pub fn register_any_input(&self, waiting: extern "C" fn(*mut c_void, i32) -> bool) {
        static DATA: u8 = 0;
        let class = unsafe { (*self.kit).class };
        type F =
            unsafe extern "C" fn(*mut Kit, extern "C" fn(*mut c_void, i32) -> bool, *mut c_void);
        let data = std::ptr::addr_of!(DATA).cast_mut().cast();
        call!(class, 24, F, self.kit, waiting, data);
    }

    #[cfg(target_os = "macos")]
    /// Runs LOK's loop until kit exit (R-01).
    pub fn run_loop(&self) {
        type PollCb = Option<extern "C" fn(*mut c_void, i32) -> i32>;
        type WakeCb = Option<extern "C" fn(*mut c_void)>;
        extern "C" fn poll(_: *mut c_void, _: i32) -> i32 {
            0
        }
        extern "C" fn wake(_: *mut c_void) {}
        let class = unsafe { (*self.kit).class };
        type F = unsafe extern "C" fn(*mut Kit, PollCb, WakeCb, *mut c_void);
        call!(
            class,
            12,
            F,
            self.kit,
            Some(poll),
            Some(wake),
            std::ptr::null_mut()
        );
    }
}

impl Drop for Lok {
    fn drop(&mut self) {
        if self.kit.is_null() {
            return;
        }
        let class = unsafe { (*self.kit).class };
        type F = unsafe extern "C" fn(*mut Kit);
        call!(class, 0, F, self.kit);
        log::info!("lok: destroyed");
    }
}

/// A Rust string as a NUL-terminated C string.
fn cstring(text: &str) -> io::Result<CString> {
    CString::new(text)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the string holds a NUL"))
}

#[cfg(unix)]
struct Library {
    handle: *mut c_void,
}

#[cfg(unix)]
impl Library {
    /// dlopens the LOK library, merged candidates in order.
    fn open(install: &Path) -> io::Result<Self> {
        use std::os::unix::ffi::OsStrExt;
        let mut last = "no candidate".to_string();
        for name in [
            "libsofficeapp.dylib",
            "libmergedlo.dylib",
            "libsofficeapp.so",
            "libmergedlo.so",
        ] {
            let path = install.join(name);
            if !path.try_exists()? {
                continue;
            }
            let c = CString::new(path.as_os_str().as_bytes())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the path holds a NUL"))?;
            let handle = unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_LAZY | libc::RTLD_GLOBAL) };
            if handle.is_null() {
                last = dl_error();
                log::warn!("lok: dlopen {} failed: {last}", path.display());
                continue;
            }
            return Ok(Self { handle });
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "no loadable LOK library under {}: {last}",
                install.display()
            ),
        ))
    }

    /// dlsyms `name` from the library.
    unsafe fn symbol<F: Copy>(&self, name: &[u8]) -> Result<F, String> {
        let name = name.strip_suffix(b"\0").unwrap_or(name);
        let c = CString::new(name).map_err(|_| "bad symbol".to_string())?;
        unsafe { libc::dlerror() };
        let ptr = unsafe { libc::dlsym(self.handle, c.as_ptr()) };
        if ptr.is_null() {
            return Err(dl_error());
        }
        Ok(unsafe { std::mem::transmute_copy(&ptr) })
    }
}

#[cfg(unix)]
fn dl_error() -> String {
    let raw = unsafe { libc::dlerror() };
    if raw.is_null() {
        return "dlopen failed".into();
    }
    unsafe { std::ffi::CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(windows)]
struct Library {
    handle: isize,
}

#[cfg(windows)]
impl Library {
    /// Loads `<install>/sofficeapp.dll`, merged fallback.
    fn open(install: &Path) -> io::Result<Self> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::System::LibraryLoader::{
            LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LoadLibraryExW,
        };
        for name in ["sofficeapp.dll", "mergedlo.dll"] {
            let path = install.join(name);
            if !path.try_exists()? {
                continue;
            }
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let handle = unsafe {
                LoadLibraryExW(
                    wide.as_ptr(),
                    std::ptr::null_mut(),
                    LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                )
            };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            return Ok(Self {
                handle: handle as isize,
            });
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no LOK library under {}", install.display()),
        ))
    }

    /// Gets `name` from the library.
    unsafe fn symbol<F: Copy>(&self, name: &[u8]) -> Result<F, String> {
        use windows_sys::Win32::System::LibraryLoader::GetProcAddress;
        let name = name.strip_suffix(b"\0").unwrap_or(name);
        let c = CString::new(name).map_err(|_| "bad symbol".to_string())?;
        let ptr = unsafe { GetProcAddress(self.handle as _, c.as_ptr() as _) };
        let Some(ptr) = ptr else {
            return Err(format!("no symbol {}", String::from_utf8_lossy(name)));
        };
        Ok(unsafe { std::mem::transmute_copy(&ptr) })
    }
}
