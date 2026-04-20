use std::{cell::RefCell, rc::Rc};

use euclid::{Box2D, Point2D};
use godot::{classes::{Control, Engine, IControl, Image, ImageTexture, image::Format}, prelude::*};
use servo::{WebView, WebViewBuilder, WebViewDelegate};
use url::Url;

use crate::servo_manager::ServoManager;

enum ProxyEvent {
    UrlChanged(Url),
    NewFrameReady,
}

#[derive(GodotClass)]
#[class(base=Control, tool, rename=WebView)]
struct WebViewControl {
    base: Base<Control>,
    webview: WebView,
    event_queue: Rc<RefCell<Vec<ProxyEvent>>>,
    image_texture: Option<Gd<ImageTexture>>,
}

#[godot_api]
impl IControl for WebViewControl {
    fn init(base: Base<Control>) -> Self {
        let servo_manager = Engine::singleton()
            .get_singleton("ServoManager").expect("Failed to get singleton")
            .cast::<ServoManager>();
        let event_queue = Rc::new(RefCell::new(Vec::new()));
        let webview =
            WebViewBuilder::new(
                    servo_manager.bind().get_servo(),
                    servo_manager.bind().get_window().get_rendering_context()
                )
                .delegate(Rc::new(Proxy {
                    event_queue: event_queue.clone(),
                }))
                .url(Url::parse("https://google.com").expect("Failed to parse url"))
                .build();

        Self {
            base,
            webview,
            event_queue,
            image_texture: None
        }
    }

    fn draw(&mut self) {
        if let Some(image_texture) = self.image_texture.clone() {
            self.base_mut().draw_texture(&image_texture, Vector2::ZERO);
        }
    }

    fn process(&mut self, _delta: f64) {
        let mut servo_manager = Engine::singleton()
            .get_singleton("ServoManager")
            .expect("Failed to get singleton").cast::<ServoManager>();
        
        {
            servo_manager.bind_mut().wake_if_needed();
        }
            

        let events: Vec<ProxyEvent> = self.event_queue.borrow_mut().drain(..).collect();
        for event in events {
            match event {
                ProxyEvent::UrlChanged(url) => {
                    godot_print!("WebViewControl: URL changed to {}", url.as_str());

                },
                ProxyEvent::NewFrameReady => {
                    self.webview.paint();
                    let servo_manager = servo_manager.bind();
                    let window = servo_manager.get_window();
                    let window_size = window.get_rendering_context().size();
                    let image_option = window.get_rendering_context()
                        .read_to_image(Box2D::new(Point2D::origin(), Point2D::new(window_size.width as i32, window_size.height as i32)));
                    if let Some(image_buffer) = image_option {
                        let data = PackedByteArray::from(image_buffer.as_raw().as_slice());
                        let image = Image::create_from_data(
                            window_size.width as i32, window_size.height as i32,
                            false, Format::RGBA8, &data);
                        if let Some(mut image_texture) = self.image_texture.clone() {
                            image_texture.set_image(image.as_ref());
                        } else {
                            let image_texture = ImageTexture::create_from_image(image.as_ref());
                            self.image_texture = image_texture;
                        }
                        if image.is_some() {
                            self.base_mut().queue_redraw();
                        }
                    }
                }
            }
        }
    }
}

struct Proxy {
    event_queue: Rc<RefCell<Vec<ProxyEvent>>>
}

impl WebViewDelegate for Proxy {
    fn notify_url_changed(&self, _webview: WebView, url: Url) {
        self.event_queue.borrow_mut().push(ProxyEvent::UrlChanged(url));
    }

    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.event_queue.borrow_mut().push(ProxyEvent::NewFrameReady);
    }
}
