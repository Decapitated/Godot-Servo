use std::rc::Rc;

use dpi::PhysicalSize;
use euclid::{Box2D, Point2D};
use godot::{classes::{Image, ImageTexture, Texture2D, image::Format}, prelude::*};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{OffscreenRenderingContext, RenderingContext, SoftwareRenderingContext, WindowRenderingContext};

use crate::godot_window_handle::GodotWindowHandle;

pub trait GodotRenderingContext {
    fn get_rendering_context(&self) -> Rc<dyn RenderingContext>;
    fn get_texture(&self) -> Option<Gd<Texture2D>>;
    fn update(&mut self);
    fn resized(&mut self);
}

pub struct GodotSoftwareRenderingContext {
    rendering_context: Rc<SoftwareRenderingContext>,
    image_texture: Option<Gd<ImageTexture>>,
    image: Option<Gd<Image>>,
    buffer: PackedByteArray
}

impl GodotSoftwareRenderingContext {
    pub fn new(size: PhysicalSize<u32>) -> Self {
        let rendering_context = 
            Rc::new(
                SoftwareRenderingContext::new(size)
                .expect("Failed to create rendering context"));
        Self {
            rendering_context,
            image_texture: None,
            image: None,
            buffer: PackedByteArray::new()
        }
    }
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
        let window_size = self.rendering_context.size();
        let width = window_size.width as i32;
        let height = window_size.height as i32;

        let image_option = self.rendering_context.read_to_image(
            Box2D::new(Point2D::origin(), Point2D::new(width, height)));
        if let Some(image_buffer) = image_option {
            let raw = image_buffer.as_raw();
            if self.buffer.len() != raw.len() {
                self.buffer.resize(raw.len());
            }
            self.buffer.as_mut_slice().copy_from_slice(raw.as_slice());
            
            if self.image_texture.is_none() {
                let image = Image::create_from_data(
                    width, height,
                    false, Format::RGBA8, &self.buffer);
                let image_texture = ImageTexture::create_from_image(image.as_ref());
                self.image = image;
                self.image_texture = image_texture;
            } else {
                if let Some(mut image) = self.image.clone() {
                    image.set_data(width, height, false, Format::RGBA8, &self.buffer);
                }
                if let (Some(mut image_texture), Some(image)) =
                       (self.image_texture.clone(), self.image.clone()) {
                    image_texture.update(&image);
                }
            }
        }
    }

    fn resized(&mut self) {
        self.image = None;
        self.image_texture = None;
    }
}

pub struct GodotOffscreenRenderingContext {
    _window_rendering_context: Rc<WindowRenderingContext>,
    rendering_context: Rc<OffscreenRenderingContext>,
    image_texture: Option<Gd<ImageTexture>>,
    image: Option<Gd<Image>>,
    buffer: PackedByteArray
}

impl GodotOffscreenRenderingContext {
    pub fn new(size: PhysicalSize<u32>) -> Self {
        let _window_rendering_context =
            Rc::new(Self::get_window_context(size));
        let rendering_context =
            Rc::new(_window_rendering_context.offscreen_context(size));
        Self {
            _window_rendering_context,
            rendering_context,
            image_texture: None,
            image: None,
            buffer: PackedByteArray::new()
        }
    }

    fn get_window_context(size: PhysicalSize<u32>) -> WindowRenderingContext {
        let godot_window = GodotWindowHandle::new();

        let display_handle = godot_window.display_handle().expect("Failed to get display handle");
        let window_handle = godot_window.window_handle().expect("Failed to get window handle");

        WindowRenderingContext::new(display_handle, window_handle, size).expect("Failed to create window context")
    }
}

impl GodotRenderingContext for GodotOffscreenRenderingContext {
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
        if let Err(e) = self.rendering_context.make_current() {
            godot_error!("WebViewControl: Failed to make GL context current: {:?}", e);
            return;
        }

        self.rendering_context.present();

        let window_size = self.rendering_context.size();
        let width = window_size.width as i32;
        let height = window_size.height as i32;

        let image_option = self.rendering_context.read_to_image(
            Box2D::new(Point2D::origin(), Point2D::new(width, height)));
        if let Some(image_buffer) = image_option {
            let raw = image_buffer.as_raw();
            if self.buffer.len() != raw.len() {
                self.buffer.resize(raw.len());
            }
            self.buffer.as_mut_slice().copy_from_slice(raw.as_slice());
            
            if self.image_texture.is_none() {
                let image = Image::create_from_data(
                    width, height,
                    false, Format::RGBA8, &self.buffer);
                let image_texture = ImageTexture::create_from_image(image.as_ref());
                self.image = image;
                self.image_texture = image_texture;
            } else {
                if let Some(mut image) = self.image.clone() {
                    image.set_data(width, height, false, Format::RGBA8, &self.buffer);
                }
                if let (Some(mut image_texture), Some(image)) =
                       (self.image_texture.clone(), self.image.clone()) {
                    image_texture.update(&image);
                }
            }
        }
    }

    fn resized(&mut self) {
        self.image = None;
        self.image_texture = None;
    }
}
