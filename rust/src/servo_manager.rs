use std::{rc::Rc, sync::{Arc, atomic::{AtomicBool, Ordering}}};

use dpi::PhysicalSize;
use godot::{classes::Engine, prelude::*};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{AllowOrDenyRequest, EventLoopWaker, Opts, Preferences, Servo, ServoBuilder, ServoDelegate, WindowRenderingContext};

use crate::godot_window_handle::GodotWindowHandle;


#[derive(GodotClass)]
#[class(base=Object)]
pub struct ServoManager {
    base: Base<Object>,
    servo: Servo,
    needs_wake: Arc<AtomicBool>,
    window_rendering_context: Option<Rc<WindowRenderingContext>>,
}

#[godot_api]
impl IObject for ServoManager {
    fn init(base: Base<Object>) -> Self {
        let engine = Engine::singleton();
        let opts = Opts::default();

        let mut preferences = Preferences::default();
        preferences.user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:150.0) Gecko/20100101 Firefox/150.0".to_owned();
        preferences.dom_serviceworker_enabled = true; // Needed for devtools.
        if !engine.is_editor_hint() {
            preferences.devtools_server_enabled = true;
            preferences.devtools_server_listen_address = "127.0.0.1:6080".to_owned();
        }
        
        let needs_wake = Arc::new(AtomicBool::new(false));
        let servo = ServoBuilder::default()
            .opts(opts)
            .preferences(preferences)
            .event_loop_waker(Box::new(WakerProxy { needs_wake: Arc::clone(&needs_wake) }))
            .build();

        servo.set_delegate(Rc::new(DelegateProxy));

        Self {
            base,
            servo,
            needs_wake,
            window_rendering_context: None
        }
    }
}

impl ServoManager {
    pub fn get_servo(&self) -> &Servo {
        &self.servo
    }

    pub fn wake(&self) {
        self.servo.spin_event_loop();
    }

    pub fn wake_if_needed(&mut self) {
        if self.needs_wake.swap(false, Ordering::Relaxed) {
            self.wake();
        }
    }

    pub fn get_window_context(&mut self) -> Rc<WindowRenderingContext> {
        if let Some(window_rendering_context) = self.window_rendering_context.clone() {
            return window_rendering_context;
        } else {
            let size = PhysicalSize::new(800, 600);
            let window_rendering_context = Rc::new(Self::create_window_context(size));
            self.window_rendering_context = Some(window_rendering_context.clone());
            return window_rendering_context;
        }
    }

    fn create_window_context(size: PhysicalSize<u32>) -> WindowRenderingContext {
        let godot_window = GodotWindowHandle::new();

        let display_handle = godot_window.display_handle().expect("Failed to get display handle");
        let window_handle = godot_window.window_handle().expect("Failed to get window handle");

        WindowRenderingContext::new(display_handle, window_handle, size).expect("Failed to create window context")
    }
}

struct WakerProxy {
    needs_wake: Arc<AtomicBool>,
}

impl EventLoopWaker for WakerProxy {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(WakerProxy { needs_wake: Arc::clone(&self.needs_wake) })
    }

    fn wake(&self) {
        // Safe to call from any thread — just flips an atomic flag
        self.needs_wake.store(true, Ordering::Relaxed);
    }
}

struct DelegateProxy;

impl ServoDelegate for DelegateProxy {
    fn notify_devtools_server_started(&self, port: u16, _token: String) {
        godot_print!("Servo DevTools server started on port {}", port);
    }

    fn request_devtools_connection(&self, request: AllowOrDenyRequest) {
        request.allow();
    }
}
