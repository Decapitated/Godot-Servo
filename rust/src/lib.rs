use godot::{classes::Engine, prelude::*};

mod mime;
mod godot_rendering_context;
mod godot_window_handle;
mod servo_manager;
mod webview_control;

use servo_manager::ServoManager;

struct GodotServo;

#[gdextension]
unsafe impl ExtensionLibrary for GodotServo {
    fn on_stage_init(stage: InitStage) {
        if stage == InitStage::Scene {
            let manager = ServoManager::new_alloc();
            Engine::singleton().register_singleton(
                "ServoManager", &manager);
        }
    }

    fn on_stage_deinit(stage: InitStage) {
        if stage == InitStage::Scene {
            let mut engine = Engine::singleton();
            let singleton_name: StringName = "ServoManager".into();

            let singleton = engine
                .get_singleton(&singleton_name)
                .expect("Failed to get singleton");

            engine.unregister_singleton(&singleton_name);
            singleton.free();
        }
    }
}
