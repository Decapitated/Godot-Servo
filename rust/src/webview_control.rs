use std::{cell::RefCell, rc::Rc};

use dpi::PhysicalSize;
use euclid::Point2D;
use godot::{classes::{Control, Engine, FileAccess, IControl, InputEvent, InputEventKey, InputEventMouse, InputEventMouseButton, InputEventMouseMotion, control::{CursorShape, FocusMode}, file_access::ModeFlags}, global, prelude::*};
use http::{HeaderMap, HeaderValue, header};
use keyboard_types::{Code, Key, KeyState, Location, Modifiers};
use servo::{KeyboardEvent as ServoKeyboardEvent, MouseButtonEvent, MouseMoveEvent, NamedKey, WebResourceResponse, WebView, WebViewBuilder, WebViewDelegate, WebViewPoint, WheelDelta, WheelEvent, WheelMode};
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
        self.base_mut().set_focus_mode(FocusMode::ALL);

        self.signals().resized().connect_self(Self::on_resize);
        self.signals().mouse_entered().connect_self(Self::on_mouse_entered);
        self.signals().mouse_exited().connect_self(Self::on_mouse_exited);

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
            self.base_mut().accept_event();
        } else if let Ok(key_event) = event.try_cast::<InputEventKey>() {
            let state = if key_event.is_pressed() { KeyState::Down } else { KeyState::Up };

            // Use the unicode codepoint for printable characters so that layout-dependent
            // keys (accents, symbols, shifted digits, etc.) are handled automatically.
            // Fall back to the named-key mapping when no printable character is produced
            // or when Ctrl is held (where the unicode value would be a control byte).
            let unicode = key_event.get_unicode();
            let use_char = unicode > 0x1f   // skip ASCII control characters
                && unicode != 0x7f          // skip DEL
                && !key_event.is_ctrl_pressed();
            let key = if use_char {
                if let Some(c) = char::from_u32(unicode as u32) {
                    let mut s = String::new();
                    s.push(c);
                    Key::Character(s)
                } else {
                    godot_key_to_key(key_event.get_keycode())
                }
            } else {
                godot_key_to_key(key_event.get_keycode())
            };

            let code = godot_key_to_code(key_event.get_physical_keycode());

            let mut modifiers = Modifiers::empty();
            if key_event.is_ctrl_pressed()  { modifiers |= Modifiers::CONTROL; }
            if key_event.is_shift_pressed() { modifiers |= Modifiers::SHIFT; }
            if key_event.is_alt_pressed()   { modifiers |= Modifiers::ALT; }
            if key_event.is_meta_pressed()  { modifiers |= Modifiers::META; }

            let kb_event = keyboard_types::KeyboardEvent {
                state,
                key,
                code,
                location: Location::Standard,
                modifiers,
                repeat: key_event.is_echo(),
                is_composing: false,
            };
            webview_event = Some(servo::InputEvent::Keyboard(
                ServoKeyboardEvent::new(kb_event)
            ));
            self.base_mut().accept_event();
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

    fn on_mouse_entered(&mut self) {
        self.base_mut().grab_focus();
        // self.webview.focus();
    }

    fn on_mouse_exited(&mut self) {
        self.base_mut().release_focus();
        // self.webview.blur();
    }

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

    #[func]
    fn reload(&mut self) {
        self.webview.reload();
    }

    #[func]
    fn back(&mut self) {
        self.webview.go_back(1);
    }

    #[func]
    fn forward(&mut self) {
        self.webview.go_forward(1);
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
        // else {
        //     let mut request = load.request;
        //     request.headers.insert(
        //         header::USER_AGENT,
        //         HeaderValue::from_static(""));
        // }
    }
}

/// Maps a Godot logical keycode to a `keyboard_types::Kmetaey`.
/// Printable characters are handled via `get_unicode()` at the call site;
/// this function covers only non-printable / named keys.
fn godot_key_to_key(keycode: global::Key) -> Key {
    match keycode {
        global::Key::ENTER | global::Key::KP_ENTER => Key::Named(NamedKey::Enter),
        global::Key::TAB => Key::Named(NamedKey::Tab),
        global::Key::BACKSPACE => Key::Named(NamedKey::Backspace),
        global::Key::ESCAPE => Key::Named(NamedKey::Escape),
        global::Key::DELETE => Key::Named(NamedKey::Delete),
        global::Key::INSERT => Key::Named(NamedKey::Insert),
        global::Key::HOME => Key::Named(NamedKey::Home),
        global::Key::END => Key::Named(NamedKey::End),
        global::Key::PAGEUP => Key::Named(NamedKey::PageUp),
        global::Key::PAGEDOWN => Key::Named(NamedKey::PageDown),
        global::Key::LEFT => Key::Named(NamedKey::ArrowLeft),
        global::Key::RIGHT => Key::Named(NamedKey::ArrowRight),
        global::Key::UP => Key::Named(NamedKey::ArrowUp),
        global::Key::DOWN => Key::Named(NamedKey::ArrowDown),
        global::Key::F1 => Key::Named(NamedKey::F1),
        global::Key::F2 => Key::Named(NamedKey::F2),
        global::Key::F3 => Key::Named(NamedKey::F3),
        global::Key::F4 => Key::Named(NamedKey::F4),
        global::Key::F5 => Key::Named(NamedKey::F5),
        global::Key::F6 => Key::Named(NamedKey::F6),
        global::Key::F7 => Key::Named(NamedKey::F7),
        global::Key::F8 => Key::Named(NamedKey::F8),
        global::Key::F9 => Key::Named(NamedKey::F9),
        global::Key::F10 => Key::Named(NamedKey::F10),
        global::Key::F11 => Key::Named(NamedKey::F11),
        global::Key::F12 => Key::Named(NamedKey::F12),
        global::Key::SHIFT => Key::Named(NamedKey::Shift),
        global::Key::CTRL => Key::Named(NamedKey::Control),
        global::Key::ALT => Key::Named(NamedKey::Alt),
        global::Key::META => Key::Named(NamedKey::Meta),
        global::Key::CAPSLOCK => Key::Named(NamedKey::CapsLock),
        global::Key::NUMLOCK => Key::Named(NamedKey::NumLock),
        global::Key::SCROLLLOCK => Key::Named(NamedKey::ScrollLock),
        global::Key::PAUSE => Key::Named(NamedKey::Pause),
        global::Key::PRINT => Key::Named(NamedKey::PrintScreen),
        global::Key::SPACE => Key::Character(" ".to_string()),
        _ => Key::Named(NamedKey::Unidentified),
    }
}

/// Maps a Godot physical keycode to a `keyboard_types::Code`.
/// Physical keycodes represent the key's position on the keyboard regardless of layout.
fn godot_key_to_code(physical: global::Key) -> Code {
    match physical {
        global::Key::A => Code::KeyA,
        global::Key::B => Code::KeyB,
        global::Key::C => Code::KeyC,
        global::Key::D => Code::KeyD,
        global::Key::E => Code::KeyE,
        global::Key::F => Code::KeyF,
        global::Key::G => Code::KeyG,
        global::Key::H => Code::KeyH,
        global::Key::I => Code::KeyI,
        global::Key::J => Code::KeyJ,
        global::Key::K => Code::KeyK,
        global::Key::L => Code::KeyL,
        global::Key::M => Code::KeyM,
        global::Key::N => Code::KeyN,
        global::Key::O => Code::KeyO,
        global::Key::P => Code::KeyP,
        global::Key::Q => Code::KeyQ,
        global::Key::R => Code::KeyR,
        global::Key::S => Code::KeyS,
        global::Key::T => Code::KeyT,
        global::Key::U => Code::KeyU,
        global::Key::V => Code::KeyV,
        global::Key::W => Code::KeyW,
        global::Key::X => Code::KeyX,
        global::Key::Y => Code::KeyY,
        global::Key::Z => Code::KeyZ,
        global::Key::KEY_0 => Code::Digit0,
        global::Key::KEY_1 => Code::Digit1,
        global::Key::KEY_2 => Code::Digit2,
        global::Key::KEY_3 => Code::Digit3,
        global::Key::KEY_4 => Code::Digit4,
        global::Key::KEY_5 => Code::Digit5,
        global::Key::KEY_6 => Code::Digit6,
        global::Key::KEY_7 => Code::Digit7,
        global::Key::KEY_8 => Code::Digit8,
        global::Key::KEY_9 => Code::Digit9,
        global::Key::SPACE => Code::Space,
        global::Key::ENTER => Code::Enter,
        global::Key::KP_ENTER => Code::NumpadEnter,
        global::Key::TAB => Code::Tab,
        global::Key::BACKSPACE => Code::Backspace,
        global::Key::ESCAPE => Code::Escape,
        global::Key::DELETE => Code::Delete,
        global::Key::INSERT => Code::Insert,
        global::Key::HOME => Code::Home,
        global::Key::END => Code::End,
        global::Key::PAGEUP => Code::PageUp,
        global::Key::PAGEDOWN => Code::PageDown,
        global::Key::LEFT => Code::ArrowLeft,
        global::Key::RIGHT => Code::ArrowRight,
        global::Key::UP => Code::ArrowUp,
        global::Key::DOWN => Code::ArrowDown,
        global::Key::F1 => Code::F1,
        global::Key::F2 => Code::F2,
        global::Key::F3 => Code::F3,
        global::Key::F4 => Code::F4,
        global::Key::F5 => Code::F5,
        global::Key::F6 => Code::F6,
        global::Key::F7 => Code::F7,
        global::Key::F8 => Code::F8,
        global::Key::F9 => Code::F9,
        global::Key::F10 => Code::F10,
        global::Key::F11 => Code::F11,
        global::Key::F12 => Code::F12,
        global::Key::SHIFT => Code::ShiftLeft,
        global::Key::CTRL => Code::ControlLeft,
        global::Key::ALT => Code::AltLeft,
        global::Key::META => Code::MetaLeft,
        global::Key::CAPSLOCK => Code::CapsLock,
        global::Key::NUMLOCK => Code::NumLock,
        global::Key::SCROLLLOCK => Code::ScrollLock,
        global::Key::MINUS => Code::Minus,
        global::Key::EQUAL => Code::Equal,
        global::Key::BRACKETLEFT => Code::BracketLeft,
        global::Key::BRACKETRIGHT => Code::BracketRight,
        global::Key::SEMICOLON => Code::Semicolon,
        global::Key::APOSTROPHE => Code::Quote,
        global::Key::COMMA => Code::Comma,
        global::Key::PERIOD => Code::Period,
        global::Key::SLASH => Code::Slash,
        global::Key::BACKSLASH => Code::Backslash,
        global::Key::QUOTELEFT => Code::Backquote,
        global::Key::KP_0 => Code::Numpad0,
        global::Key::KP_1 => Code::Numpad1,
        global::Key::KP_2 => Code::Numpad2,
        global::Key::KP_3 => Code::Numpad3,
        global::Key::KP_4 => Code::Numpad4,
        global::Key::KP_5 => Code::Numpad5,
        global::Key::KP_6 => Code::Numpad6,
        global::Key::KP_7 => Code::Numpad7,
        global::Key::KP_8 => Code::Numpad8,
        global::Key::KP_9 => Code::Numpad9,
        global::Key::KP_ADD => Code::NumpadAdd,
        global::Key::KP_SUBTRACT => Code::NumpadSubtract,
        global::Key::KP_MULTIPLY => Code::NumpadMultiply,
        global::Key::KP_DIVIDE => Code::NumpadDivide,
        global::Key::KP_PERIOD => Code::NumpadDecimal,
        _ => Code::Unidentified,
    }
}
