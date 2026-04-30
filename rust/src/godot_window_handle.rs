use godot::{classes::{DisplayServer, display_server::HandleType}, prelude::*};
use raw_window_handle::{
    DisplayHandle, WindowHandle, HasDisplayHandle, HasWindowHandle,
    RawDisplayHandle, RawWindowHandle, HandleError,
};

pub struct GodotWindowHandle;

impl GodotWindowHandle {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use std::num::NonZeroIsize;
    use raw_window_handle::{Win32WindowHandle, WindowsDisplayHandle};

    impl HasWindowHandle for GodotWindowHandle {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let ds = DisplayServer::singleton();
            let hwnd = ds.window_get_native_handle(HandleType::WINDOW_HANDLE);
            let raw = RawWindowHandle::Win32(
                Win32WindowHandle::new(
                    NonZeroIsize::new(hwnd as isize).expect("HWND must be non-null")
                )
            );
            Ok(unsafe { WindowHandle::borrow_raw(raw) })
        }
    }

    impl HasDisplayHandle for GodotWindowHandle {
        fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
            // Windows doesn't need a display connection; the handle is empty
            let raw = RawDisplayHandle::Windows(WindowsDisplayHandle::new());
            Ok(unsafe { DisplayHandle::borrow_raw(raw) })
        }
    }
}

/* These have not been tested and vetted. Kept around for reference.
#[cfg(all(target_os = "linux", feature = "x11"))]
mod platform {
    use super::*;
    use std::ptr::NonNull;
    use raw_window_handle::{XlibWindowHandle, XlibDisplayHandle};

    impl HasWindowHandle for GodotWindowHandle {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let ds = DisplayServer::singleton();
            // WINDOW_HANDLE on X11 gives the XID (u64)
            let xid = ds.window_get_native_handle(HandleType::WINDOW_HANDLE, self.window_id);
            let raw = RawWindowHandle::Xlib(
                XlibWindowHandle::new(xid as u64)
            );
            Ok(unsafe { WindowHandle::borrow_raw(raw) })
        }
    }

    impl HasDisplayHandle for GodotWindowHandle {
        fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
            let ds = DisplayServer::singleton();
            // DISPLAY_HANDLE on X11 gives the *Display pointer
            let ptr = ds.window_get_native_handle(HandleType::DISPLAY_HANDLE, self.window_id);
            let mut handle = XlibDisplayHandle::new(
                NonNull::new(ptr as *mut c_void)
            );
            handle.screen = 0; // set screen number if needed
            let raw = RawDisplayHandle::Xlib(handle);
            Ok(unsafe { DisplayHandle::borrow_raw(raw) })
        }
    }
}

#[cfg(all(target_os = "linux", feature = "wayland"))]
mod platform {
    use super::*;
    use std::ptr::NonNull;
    use raw_window_handle::{WaylandWindowHandle, WaylandDisplayHandle};

    impl HasWindowHandle for GodotWindowHandle {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let ds = DisplayServer::singleton();
            let ptr = ds.window_get_native_handle(HandleType::WINDOW_HANDLE, self.window_id);
            let raw = RawWindowHandle::Wayland(
                WaylandWindowHandle::new(
                    NonNull::new(ptr as *mut c_void).expect("wl_surface must be non-null")
                )
            );
            Ok(unsafe { WindowHandle::borrow_raw(raw) })
        }
    }

    impl HasDisplayHandle for GodotWindowHandle {
        fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
            let ds = DisplayServer::singleton();
            let ptr = ds.window_get_native_handle(HandleType::DISPLAY_HANDLE, self.window_id);
            let raw = RawDisplayHandle::Wayland(
                WaylandDisplayHandle::new(
                    NonNull::new(ptr as *mut c_void).expect("wl_display must be non-null")
                )
            );
            Ok(unsafe { DisplayHandle::borrow_raw(raw) })
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::ptr::NonNull;
    use raw_window_handle::{AppKitWindowHandle, AppKitDisplayHandle};

    impl HasWindowHandle for GodotWindowHandle {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let ds = DisplayServer::singleton();
            // WINDOW_VIEW gives the NSView pointer on macOS
            let ptr = ds.window_get_native_handle(HandleType::WINDOW_VIEW, self.window_id);
            let raw = RawWindowHandle::AppKit(
                AppKitWindowHandle::new(
                    NonNull::new(ptr as *mut c_void).expect("NSView must be non-null")
                )
            );
            Ok(unsafe { WindowHandle::borrow_raw(raw) })
        }
    }

    impl HasDisplayHandle for GodotWindowHandle {
        fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
            // AppKit display handle is empty (no connection object)
            let raw = RawDisplayHandle::AppKit(AppKitDisplayHandle::new());
            Ok(unsafe { DisplayHandle::borrow_raw(raw) })
        }
    }
}
*/
