use std::rc::Rc;

use dpi::PhysicalSize;
use servo::{RenderingContext, SoftwareRenderingContext};

pub(crate) struct HeadlessWindow {
    rendering_context: Rc<dyn RenderingContext>
}

impl HeadlessWindow {
    pub fn new(size: PhysicalSize<u32>) -> Self {
        let rendering_context = SoftwareRenderingContext::new(
            size
        ).expect("Failed to create rendering context");

        Self {
            rendering_context: Rc::new(rendering_context)
        }
    }

    pub fn get_rendering_context(&self) -> Rc<dyn RenderingContext> {
        self.rendering_context.clone()
    }
}
