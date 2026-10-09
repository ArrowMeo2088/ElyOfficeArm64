//! The fence: where pages go, what they load.

use cef::{wrapper::message_router::MessageRouterBrowserSideHandlerCallbacks, *};

use crate::{handlers::router, surface::Shared};

wrap_request_handler! {
    pub(crate) struct Requests {
        surface: Shared,
    }

    impl RequestHandler {
        fn on_before_browse(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            user_gesture: ::std::os::raw::c_int,
            _is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let url = request.map(|r| CefString::from(&r.url()).to_string()).unwrap_or_default();
            let mut surface = self.surface.lock().expect("the surface lock");
            // Content stays bundled; a clicked link leaves for the system.
            if !surface.navigable(&url) {
                if user_gesture == 1 {
                    surface.leaving.push(url);
                }
                return 1;
            }
            drop(surface);
            router().on_before_browse(browser.map(|b| b.clone()), frame.map(|f| f.clone()));
            0
        }

        fn resource_request_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            _is_navigation: ::std::os::raw::c_int,
            _is_download: ::std::os::raw::c_int,
            _request_initiator: Option<&CefString>,
            _disable_default_handling: Option<&mut ::std::os::raw::c_int>,
        ) -> Option<ResourceRequestHandler> {
            Some(Resources::new(self.surface.clone()))
        }

        fn on_render_process_terminated(
            &self,
            browser: Option<&mut Browser>,
            status: TerminationStatus,
            error_code: ::std::os::raw::c_int,
            _error_string: Option<&CefString>,
        ) {
            router().on_render_process_terminated(browser.map(|b| b.clone()));
            self.surface.lock().expect("the surface lock").failed =
                Some(format!("the page stopped: {status:?} ({error_code})"));
        }
    }
}

wrap_resource_request_handler! {
    struct Resources {
        surface: Shared,
    }

    impl ResourceRequestHandler {
        // Every request, scripts and images too, meets the fence.
        fn on_before_resource_load(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> ReturnValue {
            let Some(request) = request else {
                return ReturnValue::CANCEL;
            };
            let url = CefString::from(&request.url()).to_string();
            let image = request.resource_type() == ResourceType::IMAGE;
            if self.surface.lock().expect("the surface lock").loadable(&url, image) {
                return ReturnValue::CONTINUE;
            }
            log::warn!("web: a request outside the allowed folders was refused");
            ReturnValue::CANCEL
        }
    }
}
