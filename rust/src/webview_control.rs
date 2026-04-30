use std::{cell::RefCell, rc::Rc};

use dpi::PhysicalSize;
use euclid::Point2D;
use godot::{classes::{Control, Engine, FileAccess, IControl, InputEvent, InputEventKey, InputEventMouse, InputEventMouseButton, InputEventMouseMotion, Os, control::{CursorShape, FocusMode}, file_access::ModeFlags}, global::{self, KeyLocation, KeyModifierMask}, obj::EngineEnum, prelude::*};
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
            let os = Os::singleton();
            // Key
            let keycode = key_event.get_keycode();
            let is_unicode = os.is_keycode_unicode(keycode.ord() as u32);
            let key = if key_event.is_pressed() && is_unicode {
                let character = GString::chr(key_event.get_unicode().into());
                Some(Key::Character(character.to_string()))
            } else if !is_unicode {
                Some(godot_key_to_key(keycode))  
            } else {
                None
            };
            if  let Some(key) = key {
                // State
                let state = match key_event.is_pressed() {
                    true => KeyState::Down,
                    false => KeyState::Up
                };
                // Code
                let code = godot_key_to_code(key_event.get_physical_keycode(), key_event.get_location());
                // Modifiers
                let modifiers = key_event.get_modifiers_mask().ord() as i32;
                let mut servo_modifiers: u32 = 0;
                for modifier in KeyModifierMask::all_constants() {
                    let modifier = modifier.value();
                    let modifier_ord = modifier.ord() as i32;
                    if (modifiers & modifier_ord) == modifier_ord {
                        servo_modifiers |= godot_modifier_to_modifier(modifier).bits();
                    }
                }
                let servo_modifiers = Modifiers::from_bits_retain(servo_modifiers);
                let kb_event = keyboard_types::KeyboardEvent {
                    state,
                    key,
                    code,
                    location: Location::Standard,
                    modifiers: servo_modifiers,
                    repeat: key_event.is_echo(),
                    is_composing: false,
                };
                webview_event = Some(servo::InputEvent::Keyboard(
                    ServoKeyboardEvent::new(kb_event)
                ));
                self.base_mut().accept_event();
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
        // Confirm / whitespace
        global::Key::ENTER | global::Key::KP_ENTER => Key::Named(NamedKey::Enter),
        global::Key::TAB | global::Key::BACKTAB => Key::Named(NamedKey::Tab),
        global::Key::SPACE => Key::Character(" ".to_string()),

        // Editing
        global::Key::BACKSPACE  => Key::Named(NamedKey::Backspace),
        global::Key::DELETE     => Key::Named(NamedKey::Delete),
        global::Key::INSERT     => Key::Named(NamedKey::Insert),
        global::Key::CLEAR      => Key::Named(NamedKey::Clear),
        global::Key::PAUSE      => Key::Named(NamedKey::Pause),

        // Navigation
        global::Key::HOME     => Key::Named(NamedKey::Home),
        global::Key::END      => Key::Named(NamedKey::End),
        global::Key::PAGEUP   => Key::Named(NamedKey::PageUp),
        global::Key::PAGEDOWN => Key::Named(NamedKey::PageDown),
        global::Key::LEFT     => Key::Named(NamedKey::ArrowLeft),
        global::Key::RIGHT    => Key::Named(NamedKey::ArrowRight),
        global::Key::UP       => Key::Named(NamedKey::ArrowUp),
        global::Key::DOWN     => Key::Named(NamedKey::ArrowDown),

        // Modifiers
        global::Key::SHIFT      => Key::Named(NamedKey::Shift),
        global::Key::CTRL       => Key::Named(NamedKey::Control),
        global::Key::ALT        => Key::Named(NamedKey::Alt),
        global::Key::META       => Key::Named(NamedKey::Meta),
        #[allow(deprecated)]
        global::Key::HYPER      => Key::Named(NamedKey::Hyper),
        global::Key::CAPSLOCK   => Key::Named(NamedKey::CapsLock),
        global::Key::NUMLOCK    => Key::Named(NamedKey::NumLock),
        global::Key::SCROLLLOCK => Key::Named(NamedKey::ScrollLock),

        // System
        global::Key::ESCAPE        => Key::Named(NamedKey::Escape),
        global::Key::PRINT         => Key::Named(NamedKey::PrintScreen),
        global::Key::SYSREQ        => Key::Named(NamedKey::PrintScreen),
        global::Key::MENU          => Key::Named(NamedKey::ContextMenu),
        global::Key::HELP          => Key::Named(NamedKey::Help),
        global::Key::STANDBY       => Key::Named(NamedKey::Standby),

        // Function keys
        global::Key::F1  => Key::Named(NamedKey::F1),
        global::Key::F2  => Key::Named(NamedKey::F2),
        global::Key::F3  => Key::Named(NamedKey::F3),
        global::Key::F4  => Key::Named(NamedKey::F4),
        global::Key::F5  => Key::Named(NamedKey::F5),
        global::Key::F6  => Key::Named(NamedKey::F6),
        global::Key::F7  => Key::Named(NamedKey::F7),
        global::Key::F8  => Key::Named(NamedKey::F8),
        global::Key::F9  => Key::Named(NamedKey::F9),
        global::Key::F10 => Key::Named(NamedKey::F10),
        global::Key::F11 => Key::Named(NamedKey::F11),
        global::Key::F12 => Key::Named(NamedKey::F12),
        global::Key::F13 => Key::Named(NamedKey::F13),
        global::Key::F14 => Key::Named(NamedKey::F14),
        global::Key::F15 => Key::Named(NamedKey::F15),
        global::Key::F16 => Key::Named(NamedKey::F16),
        global::Key::F17 => Key::Named(NamedKey::F17),
        global::Key::F18 => Key::Named(NamedKey::F18),
        global::Key::F19 => Key::Named(NamedKey::F19),
        global::Key::F20 => Key::Named(NamedKey::F20),
        global::Key::F21 => Key::Named(NamedKey::F21),
        global::Key::F22 => Key::Named(NamedKey::F22),
        global::Key::F23 => Key::Named(NamedKey::F23),
        global::Key::F24 => Key::Named(NamedKey::F24),
        global::Key::F25 => Key::Named(NamedKey::F25),
        global::Key::F26 => Key::Named(NamedKey::F26),
        global::Key::F27 => Key::Named(NamedKey::F27),
        global::Key::F28 => Key::Named(NamedKey::F28),
        global::Key::F29 => Key::Named(NamedKey::F29),
        global::Key::F30 => Key::Named(NamedKey::F30),
        global::Key::F31 => Key::Named(NamedKey::F31),
        global::Key::F32 => Key::Named(NamedKey::F32),
        global::Key::F33 => Key::Named(NamedKey::F33),
        global::Key::F34 => Key::Named(NamedKey::F34),
        global::Key::F35 => Key::Named(NamedKey::F35),

        // Numpad operators — KP digits are handled upstream by is_keycode_unicode
        global::Key::KP_MULTIPLY => Key::Character("*".to_string()),
        global::Key::KP_DIVIDE   => Key::Character("/".to_string()),
        global::Key::KP_SUBTRACT => Key::Character("-".to_string()),
        global::Key::KP_ADD      => Key::Character("+".to_string()),
        global::Key::KP_PERIOD   => Key::Character(".".to_string()),

        // Media playback
        global::Key::MEDIAPLAY     => Key::Named(NamedKey::MediaPlay),
        global::Key::MEDIASTOP     => Key::Named(NamedKey::MediaStop),
        global::Key::MEDIAPREVIOUS => Key::Named(NamedKey::MediaTrackPrevious),
        global::Key::MEDIANEXT     => Key::Named(NamedKey::MediaTrackNext),
        global::Key::MEDIARECORD   => Key::Named(NamedKey::MediaRecord),

        // Volume
        global::Key::VOLUMEUP   => Key::Named(NamedKey::AudioVolumeUp),
        global::Key::VOLUMEDOWN => Key::Named(NamedKey::AudioVolumeDown),
        global::Key::VOLUMEMUTE => Key::Named(NamedKey::AudioVolumeMute),

        // Browser
        global::Key::BACK      => Key::Named(NamedKey::BrowserBack),
        global::Key::FORWARD   => Key::Named(NamedKey::BrowserForward),
        global::Key::STOP      => Key::Named(NamedKey::BrowserStop),
        global::Key::REFRESH   => Key::Named(NamedKey::BrowserRefresh),
        global::Key::HOMEPAGE  => Key::Named(NamedKey::BrowserHome),
        global::Key::FAVORITES => Key::Named(NamedKey::BrowserFavorites),
        global::Key::SEARCH    => Key::Named(NamedKey::BrowserSearch),
        global::Key::OPENURL   => Key::Named(NamedKey::LaunchWebBrowser),

        // Launch
        global::Key::LAUNCHMAIL  => Key::Named(NamedKey::LaunchMail),
        global::Key::LAUNCHMEDIA => Key::Named(NamedKey::LaunchMediaPlayer),
        global::Key::LAUNCH0     => Key::Named(NamedKey::LaunchApplication1),
        global::Key::LAUNCH1     => Key::Named(NamedKey::LaunchApplication1),
        global::Key::LAUNCH2     => Key::Named(NamedKey::LaunchApplication2),
        global::Key::LAUNCH3     => Key::Named(NamedKey::LaunchApplication2),
        // LAUNCH4-9 and LAUNCHA-F have no servo equivalents

        // IME / JIS
        global::Key::JIS_EISU => Key::Named(NamedKey::Eisu),
        global::Key::JIS_KANA => Key::Named(NamedKey::KanaMode),

        _ => Key::Named(NamedKey::Unidentified),
    }
}

/// Maps a Godot physical keycode to a `keyboard_types::Code`.
/// Physical keycodes represent the key's position on the keyboard regardless of layout.
fn godot_key_to_code(physical: global::Key, location: KeyLocation) -> Code {
    match (physical, location) {
        (global::Key::A, _) => Code::KeyA,
        (global::Key::B, _) => Code::KeyB,
        (global::Key::C, _) => Code::KeyC,
        (global::Key::D, _) => Code::KeyD,
        (global::Key::E, _) => Code::KeyE,
        (global::Key::F, _) => Code::KeyF,
        (global::Key::G, _) => Code::KeyG,
        (global::Key::H, _) => Code::KeyH,
        (global::Key::I, _) => Code::KeyI,
        (global::Key::J, _) => Code::KeyJ,
        (global::Key::K, _) => Code::KeyK,
        (global::Key::L, _) => Code::KeyL,
        (global::Key::M, _) => Code::KeyM,
        (global::Key::N, _) => Code::KeyN,
        (global::Key::O, _) => Code::KeyO,
        (global::Key::P, _) => Code::KeyP,
        (global::Key::Q, _) => Code::KeyQ,
        (global::Key::R, _) => Code::KeyR,
        (global::Key::S, _) => Code::KeyS,
        (global::Key::T, _) => Code::KeyT,
        (global::Key::U, _) => Code::KeyU,
        (global::Key::V, _) => Code::KeyV,
        (global::Key::W, _) => Code::KeyW,
        (global::Key::X, _) => Code::KeyX,
        (global::Key::Y, _) => Code::KeyY,
        (global::Key::Z, _) => Code::KeyZ,
        (global::Key::KEY_0, _) => Code::Digit0,
        (global::Key::KEY_1, _) => Code::Digit1,
        (global::Key::KEY_2, _) => Code::Digit2,
        (global::Key::KEY_3, _) => Code::Digit3,
        (global::Key::KEY_4, _) => Code::Digit4,
        (global::Key::KEY_5, _) => Code::Digit5,
        (global::Key::KEY_6, _) => Code::Digit6,
        (global::Key::KEY_7, _) => Code::Digit7,
        (global::Key::KEY_8, _) => Code::Digit8,
        (global::Key::KEY_9, _) => Code::Digit9,
        (global::Key::SPACE, _) => Code::Space,
        (global::Key::ENTER, _) => Code::Enter,
        (global::Key::KP_ENTER, _) => Code::NumpadEnter,
        (global::Key::TAB, _) => Code::Tab,
        (global::Key::BACKSPACE, _) => Code::Backspace,
        (global::Key::ESCAPE, _) => Code::Escape,
        (global::Key::DELETE, _) => Code::Delete,
        (global::Key::INSERT, _) => Code::Insert,
        (global::Key::HOME, _) => Code::Home,
        (global::Key::END, _) => Code::End,
        (global::Key::PAGEUP, _) => Code::PageUp,
        (global::Key::PAGEDOWN, _) => Code::PageDown,
        (global::Key::LEFT, _) => Code::ArrowLeft,
        (global::Key::RIGHT, _) => Code::ArrowRight,
        (global::Key::UP, _) => Code::ArrowUp,
        (global::Key::DOWN, _) => Code::ArrowDown,
        (global::Key::F1, _) => Code::F1,
        (global::Key::F2, _) => Code::F2,
        (global::Key::F3, _) => Code::F3,
        (global::Key::F4, _) => Code::F4,
        (global::Key::F5, _) => Code::F5,
        (global::Key::F6, _) => Code::F6,
        (global::Key::F7, _) => Code::F7,
        (global::Key::F8, _) => Code::F8,
        (global::Key::F9, _) => Code::F9,
        (global::Key::F10, _) => Code::F10,
        (global::Key::F11, _) => Code::F11,
        (global::Key::F12, _) => Code::F12,
        (global::Key::SHIFT, KeyLocation::LEFT) => Code::ShiftLeft,
        (global::Key::SHIFT, KeyLocation::RIGHT) => Code::ShiftRight,
        (global::Key::CTRL, KeyLocation::LEFT) => Code::ControlLeft,
        (global::Key::CTRL, KeyLocation::RIGHT) => Code::ControlRight,
        (global::Key::ALT, KeyLocation::LEFT) => Code::AltLeft,
        (global::Key::ALT, KeyLocation::RIGHT) => Code::AltRight,
        (global::Key::META, KeyLocation::LEFT) => Code::MetaLeft,
        (global::Key::META, KeyLocation::RIGHT) => Code::MetaRight,
        (global::Key::CAPSLOCK, _) => Code::CapsLock,
        (global::Key::NUMLOCK, _) => Code::NumLock,
        (global::Key::SCROLLLOCK, _) => Code::ScrollLock,
        (global::Key::MINUS, _) => Code::Minus,
        (global::Key::EQUAL, _) => Code::Equal,
        (global::Key::BRACKETLEFT, _) => Code::BracketLeft,
        (global::Key::BRACKETRIGHT, _) => Code::BracketRight,
        (global::Key::SEMICOLON, _) => Code::Semicolon,
        (global::Key::APOSTROPHE, _) => Code::Quote,
        (global::Key::COMMA, _) => Code::Comma,
        (global::Key::PERIOD, _) => Code::Period,
        (global::Key::SLASH, _) => Code::Slash,
        (global::Key::BACKSLASH, _) => Code::Backslash,
        (global::Key::QUOTELEFT, _) => Code::Backquote,
        (global::Key::KP_0, _) => Code::Numpad0,
        (global::Key::KP_1, _) => Code::Numpad1,
        (global::Key::KP_2, _) => Code::Numpad2,
        (global::Key::KP_3, _) => Code::Numpad3,
        (global::Key::KP_4, _) => Code::Numpad4,
        (global::Key::KP_5, _) => Code::Numpad5,
        (global::Key::KP_6, _) => Code::Numpad6,
        (global::Key::KP_7, _) => Code::Numpad7,
        (global::Key::KP_8, _) => Code::Numpad8,
        (global::Key::KP_9, _) => Code::Numpad9,
        (global::Key::KP_ADD, _) => Code::NumpadAdd,
        (global::Key::KP_SUBTRACT, _) => Code::NumpadSubtract,
        (global::Key::KP_MULTIPLY, _) => Code::NumpadMultiply,
        (global::Key::KP_DIVIDE, _) => Code::NumpadDivide,
        (global::Key::KP_PERIOD, _) => Code::NumpadDecimal,
        _ => Code::Unidentified,
    }
}

fn godot_modifier_to_modifier(modifier: KeyModifierMask) -> Modifiers {
    match modifier {
        KeyModifierMask::ALT => Modifiers::ALT,
        KeyModifierMask::CTRL => Modifiers::CONTROL,
        KeyModifierMask::META => Modifiers::META,
        KeyModifierMask::SHIFT => Modifiers::SHIFT,
        _ => Modifiers::empty(),
    }
}
