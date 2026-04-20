use std::{sync::{Arc, atomic::{AtomicBool, Ordering}}};

use dpi::PhysicalSize;
use godot::prelude::*;
use servo::{EventLoopWaker, Servo, ServoBuilder};

use crate::headless_window::HeadlessWindow;

#[derive(GodotClass)]
#[class(base=Object)]
pub struct ServoManager {
    base: Base<Object>,
    servo: Servo,
    window: HeadlessWindow,
    needs_wake: Arc<AtomicBool>,
}

#[godot_api]
impl IObject for ServoManager {
    fn init(base: Base<Object>) -> Self {
        let needs_wake = Arc::new(AtomicBool::new(false));
        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(Proxy { needs_wake: Arc::clone(&needs_wake) }))
            .build();
        let window = HeadlessWindow::new(PhysicalSize::new(800, 600));
        Self {
            base,
            servo,
            window,
            needs_wake,
        }
    }
}

impl ServoManager {
    pub fn get_servo(&self) -> &Servo {
        &self.servo
    }

    pub fn get_window(&self) -> &HeadlessWindow {
        &self.window
    }

    pub fn wake_if_needed(&mut self) {
        if self.needs_wake.swap(false, Ordering::Relaxed) {
            self.servo.spin_event_loop();
        }
    }
}

struct Proxy {
    needs_wake: Arc<AtomicBool>,
}

impl EventLoopWaker for Proxy {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(Proxy { needs_wake: Arc::clone(&self.needs_wake) })
    }

    fn wake(&self) {
        // Safe to call from any thread — just flips an atomic flag
        self.needs_wake.store(true, Ordering::Relaxed);
    }
}
