use dpi::PhysicalSize;
use godot::prelude::*;
use servo::{Servo, ServoBuilder};

use crate::headless_window::HeadlessWindow;

#[derive(GodotClass)]
#[class(base=Object)]
pub struct ServoManager {
    base: Base<Object>,
    servo: Servo,
    window: HeadlessWindow
}

#[godot_api]
impl IObject for ServoManager {
    fn init(base: Base<Object>) -> Self {
        Self {
            base,
            servo: ServoBuilder::default().build(),
            window: HeadlessWindow::new(PhysicalSize::new(800, 600))
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
}