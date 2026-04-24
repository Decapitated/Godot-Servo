use std::{cell::RefCell, rc::Rc};

use dpi::PhysicalSize;
use euclid::Point2D;
use godot::{classes::{Control, Engine, FileAccess, IControl, InputEvent, InputEventMouse, InputEventMouseButton, InputEventMouseMotion, control::CursorShape, file_access::ModeFlags}, global, prelude::*};
use http::{HeaderMap, HeaderValue, header};
use servo::{MouseButtonEvent, MouseMoveEvent, WebResourceResponse, WebView, WebViewBuilder, WebViewDelegate, WebViewPoint, WheelDelta, WheelEvent, WheelMode};
use url::Url;

use crate::{godot_rendering_context::{GodotOffscreenRenderingContext, GodotRenderingContext}, mime::to_mime, servo_manager::ServoManager};

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
        let mut servo_manager = 
            Engine::singleton()
            .get_singleton("ServoManager")
            .expect("Failed to get singleton")
            .cast::<ServoManager>();

        let window_rendering_context = servo_manager.bind_mut().get_window_context();
        let rendering_context = Rc::new(RefCell::new(
            GodotOffscreenRenderingContext::new(window_rendering_context)));
        // let rendering_context = Rc::new(RefCell::new(
        //     GodotSoftwareRenderingContext::new(size)));
        
        let event_queue = Rc::new(RefCell::new(Vec::new()));
        let webview =
            WebViewBuilder::new(
                servo_manager.bind().get_servo(),
                rendering_context.borrow().get_rendering_context()
            )
            .delegate(Rc::new(Proxy {
                event_queue: event_queue.clone(),
            }))
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
        // self.signals().mouse_entered().connect_self(Self::on_mouse_entered);
        // self.signals().mouse_exited().connect_self(Self::on_mouse_exited);

        self.on_resize();
    }

    fn draw(&mut self) {
        let texture_option = self.rendering_context.borrow().get_texture();
        if let Some(texture) = texture_option {
            self.base_mut().draw_texture(&texture, Vector2::ZERO);
        }
    }

    fn gui_input(&mut self, event: Gd<InputEvent>) {
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

        self.process_events();
    }
}

#[godot_api]
impl WebViewControl {
    #[signal]
    fn url_changed(url: String);

    fn on_resize(&mut self) {
        self.rendering_context.borrow_mut().resized();
        let control_size = self.base().get_size();
        self.webview.resize(PhysicalSize {
            width: control_size.x as u32,
            height: control_size.y as u32
        });
        self.update_image();
    }

    // fn on_mouse_entered(&mut self) {
    //     self.webview.focus();
    // }

    // fn on_mouse_exited(&mut self) {
    //     self.webview.blur();
    // }

    fn update_image(&mut self) {
        self.webview.paint();
        self.rendering_context.borrow_mut().update();
        self.base_mut().queue_redraw();
    }

    fn process_events(&mut self) {
        let events: Vec<ProxyEvent> = self.event_queue.borrow_mut().drain(..).collect();
        for event in events {
            match event {
                ProxyEvent::UrlChanged(url) => {
                    self.signals().url_changed().emit(url.as_str().to_string());
                },
                ProxyEvent::NewFrameReady => {
                    self.update_image();
                },
                ProxyEvent::CursorChanged(cursor) => {
                    self.base_mut().set_default_cursor_shape(cursor);
                },
                ProxyEvent::LoadWebResource(load) => {
                    self.load_web_resource(load);
                }
            }
        }
    }

    fn load_web_resource(&self, load: servo::WebResourceLoad) {
        let url = load.request().url.clone();
        let path = url.as_str();
        if FileAccess::file_exists(path) {
            let file = FileAccess::open(path, ModeFlags::READ);
            if let Some(mut file) = file {
                let extension = GString::from(path).get_extension().to_string();
                let mut headers = HeaderMap::new();
                if let Some(mime) = to_mime(extension.as_str()) {
                    headers.insert(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static(mime),
                    );
                }

                let response = WebResourceResponse::new(url)
                    .status_code(http::StatusCode::OK)
                    .headers(headers);

                let mut intercept_load = load.intercept(response);

                let length = file.get_length() as i64;
                let content = file.get_buffer(length);

                intercept_load.send_body_data(content.to_vec());
                intercept_load.finish();
            } else {
                let response = WebResourceResponse::new(url)
                    .status_code(http::StatusCode::NOT_FOUND);
                let intercepted = load.intercept(response);
                intercepted.finish();
            }
        }
    }

    #[func]
    fn load_url(&mut self, mut url: String) {
        let url_split = url.split_once("://");
        if url_split.is_none() {
            url = format!("https://{}", url);
        }
        let url = Url::parse(&url);
        if let Ok(url) = url {
            self.webview.load(url);
        } else if let Err(err) = url {
            godot_error!("Failed to parse url: {}", err);
        }
    }
}

enum ProxyEvent {
    UrlChanged(Url),
    NewFrameReady,
    CursorChanged(CursorShape),
    LoadWebResource(servo::WebResourceLoad)
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

    fn notify_cursor_changed(&self, _webview: WebView, cursor: servo::Cursor) {
        let cursor_shape: CursorShape = match cursor {
            // servo::Cursor::None => todo!(),
            // servo::Cursor::Default => todo!(),
            servo::Cursor::Pointer => CursorShape::POINTING_HAND,
            // servo::Cursor::ContextMenu => todo!(),
            servo::Cursor::Help => CursorShape::HELP,
            servo::Cursor::Progress => CursorShape::BUSY,
            servo::Cursor::Wait => CursorShape::WAIT,
            // servo::Cursor::Cell => todo!(),
            servo::Cursor::Crosshair => CursorShape::CROSS,
            servo::Cursor::Text => CursorShape::IBEAM,
            servo::Cursor::VerticalText => CursorShape::IBEAM,
            // servo::Cursor::Alias => todo!(),
            // servo::Cursor::Copy => todo!(),
            servo::Cursor::Move => CursorShape::MOVE,
            servo::Cursor::NoDrop => CursorShape::FORBIDDEN,
            servo::Cursor::NotAllowed => CursorShape::FORBIDDEN,
            // servo::Cursor::Grab => todo!(),
            // servo::Cursor::Grabbing => todo!(),
            servo::Cursor::EResize => CursorShape::HSIZE,
            servo::Cursor::NResize => CursorShape::VSIZE,
            servo::Cursor::NeResize => CursorShape::BDIAGSIZE,
            servo::Cursor::NwResize => CursorShape::FDIAGSIZE,
            servo::Cursor::SResize => CursorShape::VSIZE,
            servo::Cursor::SeResize => CursorShape::FDIAGSIZE,
            servo::Cursor::SwResize => CursorShape::BDIAGSIZE,
            servo::Cursor::WResize => CursorShape::HSIZE,
            servo::Cursor::EwResize => CursorShape::HSIZE,
            servo::Cursor::NsResize => CursorShape::BDIAGSIZE,
            servo::Cursor::NeswResize => CursorShape::BDIAGSIZE,
            servo::Cursor::NwseResize => CursorShape::FDIAGSIZE,
            servo::Cursor::ColResize => CursorShape::HSPLIT,
            servo::Cursor::RowResize => CursorShape::VSPLIT,
            servo::Cursor::AllScroll => CursorShape::DRAG,
            // servo::Cursor::ZoomIn => todo!(),
            // servo::Cursor::ZoomOut => todo!(),
            _ => CursorShape::ARROW
        };
        self.event_queue.borrow_mut().push(ProxyEvent::CursorChanged(cursor_shape));
    }

    // fn request_navigation(&self, _webview: WebView, _navigation_request: servo::NavigationRequest) {
        
    // }

    fn load_web_resource(&self, _webview: WebView, load: servo::WebResourceLoad) {
        if load.request().url.to_string().starts_with("res://") {
            self.event_queue.borrow_mut().push(ProxyEvent::LoadWebResource(load));
        }
    }
}
