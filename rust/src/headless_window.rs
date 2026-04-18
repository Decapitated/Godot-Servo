use std::{cell::Cell, rc::Rc};

use dpi::PhysicalSize;
use servo::{AnimationState, RenderingContext, SoftwareRenderingContext};

pub(crate) struct HeadlessWindow {
    animation_state: Cell<AnimationState>,
    rendering_context: Rc<dyn RenderingContext>,
}

impl HeadlessWindow {
    pub fn new(size: PhysicalSize<u32>) -> Self {
        let rendering_context = SoftwareRenderingContext::new(
            size
        ).expect("Failed to create rendering context");

        Self {
            animation_state: Cell::new(AnimationState::NoAnimationsPresent),
            rendering_context: Rc::new(rendering_context),
        }
    }

    pub fn get_rendering_context(&self) -> Rc<dyn RenderingContext> {
        self.rendering_context.clone()
    }
}
