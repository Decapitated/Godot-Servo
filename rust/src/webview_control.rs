use std::{cell::RefCell, rc::Rc};

use dpi::PhysicalSize;
use euclid::Point2D;
use godot::{classes::{Control, Engine, IControl, InputEvent, InputEventMouse, InputEventMouseButton, InputEventMouseMotion}, global, prelude::*};
use servo::{MouseButtonEvent, MouseMoveEvent, WebView, WebViewBuilder, WebViewDelegate, WebViewPoint, WheelDelta, WheelEvent, WheelMode};
use url::Url;

use crate::{godot_rendering_context::{GodotOffscreenRenderingContext, GodotRenderingContext}, servo_manager::ServoManager};

enum ProxyEvent {
    UrlChanged(Url),
    NewFrameReady,
}

#[derive(GodotClass)]
#[class(base=Control, tool, rename=WebView)]
struct WebViewControl {
    base: Base<Control>,
    rendering_context: Rc<RefCell<dyn GodotRenderingContext>>,
    webview: Rc<WebView>,
    event_queue: Rc<RefCell<Vec<ProxyEvent>>>
}

#[godot_api]
impl IControl for WebViewControl {
    fn init(base: Base<Control>) -> Self {
        let servo_manager = 
            Engine::singleton()
            .get_singleton("ServoManager")
            .expect("Failed to get singleton")
            .cast::<ServoManager>();

        let size = PhysicalSize::new(800, 600);
        let rendering_context = Rc::new(RefCell::new(
            GodotOffscreenRenderingContext::new(size)));
        
        let event_queue = Rc::new(RefCell::new(Vec::new()));
        let webview =
            WebViewBuilder::new(
                servo_manager.bind().get_servo(),
                rendering_context.borrow().get_rendering_context()
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
            event_queue
        }
    }

    fn ready(&mut self) {
        self.signals().resized().connect_self(Self::on_resize);
        self.on_resize();
    }

    fn draw(&mut self) {
        let texture_option = self.rendering_context.borrow().get_texture();
        if let Some(texture) = texture_option {
            self.base_mut().draw_texture(&texture, Vector2::ZERO);
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
        self.rendering_context.borrow_mut().resized();
        let control_size = self.base().get_size();
        self.webview.resize(PhysicalSize {
            width: control_size.x as u32,
            height: control_size.y as u32
        });
        self.update_image();
    }

    fn update_image(&mut self) {
        self.webview.paint();
        self.rendering_context.borrow_mut().update();
        self.base_mut().queue_redraw();
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
