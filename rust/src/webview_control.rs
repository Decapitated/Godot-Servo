use std::{cell::RefCell, rc::Rc};

use dpi::PhysicalSize;
use euclid::{Box2D, Point2D};
use godot::{classes::{Control, Engine, IControl, Image, ImageTexture, InputEvent, InputEventMouse, InputEventMouseButton, InputEventMouseMotion, image::Format}, global, prelude::*};
use servo::{MouseButtonEvent, MouseMoveEvent, RenderingContext, SoftwareRenderingContext, WebView, WebViewBuilder, WebViewDelegate, WebViewPoint, WheelDelta, WheelEvent, WheelMode};
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
    rendering_context: Rc<dyn RenderingContext>,
    webview: Rc<WebView>,
    event_queue: Rc<RefCell<Vec<ProxyEvent>>>,
    image_texture: Option<Gd<ImageTexture>>,
    image: Option<Gd<Image>>,
    buffer: PackedByteArray
}

#[godot_api]
impl IControl for WebViewControl {
    fn init(base: Base<Control>) -> Self {
        let servo_manager = 
            Engine::singleton()
            .get_singleton("ServoManager")
            .expect("Failed to get singleton")
            .cast::<ServoManager>();
        let rendering_context = 
            Rc::new(
                SoftwareRenderingContext::new(PhysicalSize::new(800, 600))
                .expect("Failed to create rendering context"));
        let event_queue = Rc::new(RefCell::new(Vec::new()));
        let webview =
            WebViewBuilder::new(
                servo_manager.bind().get_servo(),
                rendering_context.clone()
            )
            .delegate(Rc::new(Proxy {
                event_queue: event_queue.clone(),
            }))
            .url(Url::parse("https://demo.servo.org/").expect("Failed to parse url"))
            .build();

        Self {
            base,
            rendering_context,
            webview: Rc::new(webview),
            event_queue,
            image_texture: None,
            image: None,
            buffer: PackedByteArray::new()
        }
    }

    fn ready(&mut self) {
        self.signals().resized().connect_self(Self::on_resize);
        self.on_resize();
    }

    fn draw(&mut self) {
        if let Some(image_texture) = self.image_texture.clone() {
            self.base_mut().draw_texture(&image_texture, Vector2::ZERO);
        }
    }

    fn gui_input(&mut self, event: Gd<InputEvent>) {
        let event = self.base().make_input_local(&event);
        let mut webview_event: Option<servo::InputEvent> = None;
        if let Ok(mouse_event) = event.clone().try_cast::<InputEventMouse>() {
            let position = mouse_event.get_position();
            if let Ok(button_event) = mouse_event.clone().try_cast::<InputEventMouseButton>() {
                match button_event.get_button_index() {
                    global::MouseButton::WHEEL_UP |
                    global::MouseButton::WHEEL_DOWN |
                    global::MouseButton::WHEEL_LEFT |
                    global::MouseButton::WHEEL_RIGHT => {
                        let factor = button_event.get_factor() as f64 * 16.0;
                        webview_event = Some(servo::InputEvent::Wheel(WheelEvent {
                            delta: WheelDelta {
                                x: factor * match button_event.get_button_index() {
                                    global::MouseButton::WHEEL_LEFT => 1.0,
                                    global::MouseButton::WHEEL_RIGHT => -1.0,
                                    _ => 0.0
                                },
                                y: factor * match button_event.get_button_index() {
                                    global::MouseButton::WHEEL_UP => 1.0,
                                    global::MouseButton::WHEEL_DOWN => -1.0,
                                    _ => 0.0
                                },
                                z: 0.0,
                                mode: WheelMode::DeltaPixel
                            },
                            point: WebViewPoint::Device(
                                    Point2D::new(position.x, position.y))
                        }))
                    },
                    _ => {
                        webview_event = Some(servo::InputEvent::MouseButton(
                            MouseButtonEvent {
                                action: match button_event.is_pressed() {
                                    true => servo::MouseButtonAction::Down,
                                    false => servo::MouseButtonAction::Up
                                },
                                button: match button_event.get_button_index() {
                                    global::MouseButton::LEFT => servo::MouseButton::Left,
                                    global::MouseButton::MIDDLE => servo::MouseButton::Middle,
                                    global::MouseButton::RIGHT => servo::MouseButton::Right,
                                    global::MouseButton::XBUTTON1 => servo::MouseButton::Back,
                                    global::MouseButton::XBUTTON2 => servo::MouseButton::Forward,
                                    _ => servo::MouseButton::Other(0 as u16)
                                },
                                point: WebViewPoint::Device(
                                    Point2D::new(position.x, position.y))
                            }
                        ));
                    }
                }
            } else if let Ok(_) = mouse_event.try_cast::<InputEventMouseMotion>() {
                webview_event = Some(servo::InputEvent::MouseMove(MouseMoveEvent {
                    point:WebViewPoint::Device(Point2D::new(position.x,position.y)),
                    is_compatibility_event_for_touch: false
                }));
            }
        }

        if let Some(webview_event) = webview_event {
            self.webview.notify_input_event(webview_event);
        }

    }

    fn process(&mut self, _delta: f64) {
        let mut servo_manager = Engine::singleton()
            .get_singleton("ServoManager")
            .expect("Failed to get singleton").cast::<ServoManager>();
        
        if self.webview.as_ref().clone().animating() {
            servo_manager.bind_mut().wake();
        } else {
            servo_manager.bind_mut().wake_if_needed();
        }
            

        let events: Vec<ProxyEvent> = self.event_queue.borrow_mut().drain(..).collect();
        for event in events {
            match event {
                ProxyEvent::UrlChanged(url) => {
                    godot_print!("WebViewControl: URL changed to {}", url.as_str());

                },
                ProxyEvent::NewFrameReady => {
                    self.update_image();
                }
            }
        }
    }
}

#[godot_api]
impl WebViewControl {
    fn on_resize(&mut self) {
        self.image = None;
        self.image_texture = None;
        let control_size = self.base().get_size();
        self.webview.resize(PhysicalSize {
            width: control_size.x as u32,
            height: control_size.y as u32
        });
        self.update_image();
    }

    fn update_image(&mut self) {
        self.webview.paint();

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
            self.base_mut().queue_redraw();
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
