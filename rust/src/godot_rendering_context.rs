use std::rc::Rc;

use godot::{classes::{Image, ImageTexture, Texture2D}, prelude::*};
use servo::{RenderingContext, SoftwareRenderingContext};

trait GodotRenderingContext {
    fn get_rendering_context(&self) -> Rc<dyn RenderingContext>;
    fn get_texture(&self) -> Option<Gd<Texture2D>>;
    fn update(&mut self);
    fn resized(&mut self);
}

struct GodotSoftwareRenderingContext {
    rendering_context: Rc<SoftwareRenderingContext>,
    image_texture: Option<Gd<ImageTexture>>,
    image: Option<Gd<Image>>,
    buffer: PackedByteArray
}

impl GodotRenderingContext for GodotSoftwareRenderingContext {
    fn get_rendering_context(&self) -> Rc<dyn RenderingContext> {
        self.rendering_context.clone()
    }

    fn get_texture(&self) -> Option<Gd<Texture2D>> {
        if let Some(image_texture) = self.image_texture.clone() {
            return Some(image_texture.upcast::<Texture2D>());
        }
        None
    }

    fn update(&mut self) {
        self.webview.paint();
    }

    fn resized(&mut self) {

    }
}
