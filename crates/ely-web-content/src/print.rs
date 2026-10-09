//! A page as PDF, printed by Chromium itself (MARK-11).

use std::path::{Path, PathBuf};

use cef::*;

use crate::{surface::Shared, view::WebView};

wrap_pdf_print_callback! {
    struct Printed {
        surface: Shared,
    }

    impl PdfPrintCallback {
        fn on_pdf_print_finished(&self, path: Option<&CefString>, ok: ::std::os::raw::c_int) {
            let path = PathBuf::from(path.map(CefString::to_string).unwrap_or_default());
            log::info!("web: printed {} ok {}", path.display(), ok != 0);
            self.surface.lock().expect("the surface lock").printed.push((path, ok != 0));
        }
    }
}

impl WebView {
    /// Prints the page to `path`; `Event::Printed` tells how.
    pub fn print_pdf(&self, path: &Path) {
        let (path, surface) = (
            CefString::from(path.to_string_lossy().as_ref()),
            self.surface.clone(),
        );
        self.on_host(move |host| {
            let settings = PdfPrintSettings {
                print_background: 1,
                ..Default::default()
            };
            let mut done = Printed::new(surface);
            host.print_to_pdf(Some(&path), Some(&settings), Some(&mut done));
        });
    }
}
