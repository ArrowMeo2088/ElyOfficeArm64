//! Chromium's app protocol on GPUI's own `NSApplication` class.

use std::{cell::Cell, ffi::CStr};

use anyhow::{Result, bail};
use objc2::{
    ffi,
    runtime::{AnyClass, AnyObject, Bool, Imp, Sel},
    sel,
};

thread_local! {
    /// NSApp lives on the main thread; one flag serves it.
    static SENDING: Cell<bool> = const { Cell::new(false) };
}

extern "C-unwind" fn is_handling(_this: *mut AnyObject, _cmd: Sel) -> Bool {
    Bool::new(SENDING.get())
}

extern "C-unwind" fn set_handling(_this: *mut AnyObject, _cmd: Sel, on: Bool) {
    SENDING.set(on.as_bool());
}

extern "C-unwind" fn send_event(this: *mut AnyObject, _cmd: Sel, event: *mut AnyObject) {
    let was = SENDING.replace(true);
    let base = AnyClass::get(c"NSApplication").expect("AppKit has NSApplication");
    unsafe {
        let _: () = objc2::msg_send![super(&*this, base), sendEvent: event];
    }
    SENDING.set(was);
}

fn add(class: &AnyClass, name: Sel, imp: Imp, types: &CStr) -> Result<()> {
    let added = unsafe {
        ffi::class_addMethod(
            class as *const AnyClass as *mut AnyClass,
            name,
            imp,
            types.as_ptr(),
        )
    };
    if !added.as_bool() {
        bail!("{} already has -{name}", class.name().to_string_lossy());
    }
    Ok(())
}

/// Gives NSApp's class what CEF checks before it runs.
pub fn adopt_app_protocol() -> Result<()> {
    let Some(app) = (unsafe { objc2_app_kit_nsapp() }) else {
        bail!("no NSApp yet");
    };
    let class = unsafe { &*ffi::object_getClass(app.cast()) };
    if class.responds_to(sel!(isHandlingSendEvent)) {
        return Ok(());
    }
    unsafe {
        add(
            class,
            sel!(isHandlingSendEvent),
            std::mem::transmute(is_handling as *const ()),
            c"c@:",
        )?;
        add(
            class,
            sel!(setHandlingSendEvent:),
            std::mem::transmute(set_handling as *const ()),
            c"v@:c",
        )?;
        add(
            class,
            sel!(sendEvent:),
            std::mem::transmute(send_event as *const ()),
            c"v@:@",
        )?;
    }
    // Chromium checks the two `Cr` protocols; CEF's own is header-only.
    for (name, needed) in [
        (c"CrAppProtocol", true),
        (c"CrAppControlProtocol", true),
        (c"CefAppProtocol", false),
    ] {
        let protocol = unsafe { ffi::objc_getProtocol(name.as_ptr()) };
        match (protocol.is_null(), needed) {
            (true, true) => bail!("CEF did not register {}", name.to_string_lossy()),
            (true, false) => log::info!("web: {} is not registered", name.to_string_lossy()),
            (false, _) => unsafe {
                ffi::class_addProtocol(class as *const AnyClass as *mut AnyClass, protocol);
            },
        }
    }
    log::info!(
        "web: {} takes CefAppProtocol",
        class.name().to_string_lossy()
    );
    Ok(())
}

/// The shared application, if made.
unsafe fn objc2_app_kit_nsapp() -> Option<*mut AnyObject> {
    let class = AnyClass::get(c"NSApplication")?;
    let app: *mut AnyObject = unsafe { objc2::msg_send![class, sharedApplication] };
    (!app.is_null()).then_some(app)
}
