use std::{cell::RefCell, rc::Rc};

use godot::{classes::{Control, Engine, IControl}, prelude::*};
use servo::{WebView, WebViewBuilder, WebViewDelegate};
use url::Url;

use crate::servo_manager::ServoManager;

enum ProxyEvent {
    UrlChanged(Url),
    NewFrameReady,
}

#[derive(GodotClass)]
#[class(base=Control, init, tool, rename=WebView)]
struct WebViewControl {
    base: Base<Control>,
    webview: Option<WebView>,
    event_queue: Rc<RefCell<Vec<ProxyEvent>>>,
}

#[godot_api]
impl IControl for WebViewControl {
    fn ready(&mut self) {
        let servo_manager = Engine::singleton()
            .get_singleton("ServoManager").expect("Failed to get singleton")
            .cast::<ServoManager>();
        let webview =
            WebViewBuilder::new(
                    servo_manager.bind().get_servo(),
                    servo_manager.bind().get_window().get_rendering_context()
                )
                .delegate(Rc::new(Proxy {
                    event_queue: self.event_queue.clone(),
                }))
                .url(Url::parse("https://google.com").expect("Failed to parse url"))
                .build();
        self.webview = Some(webview);
        godot_print!("Ready");
    }

    fn process(&mut self, _delta: f64) {
        Engine::singleton()
            .get_singleton("ServoManager")
            .expect("Failed to get singleton").cast::<ServoManager>()
            .bind_mut()
            .wake_if_needed();

        let events: Vec<ProxyEvent> = self.event_queue.borrow_mut().drain(..).collect();
        for event in events {
            match event {
                ProxyEvent::UrlChanged(url) => {
                    godot_print!("WebViewControl: URL changed to {url}");

                },
                ProxyEvent::NewFrameReady => {
                    godot_print!("WebViewControl: New frame ready");
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
        godot_print!("Proxy: URL changed");
        self.event_queue.borrow_mut().push(ProxyEvent::UrlChanged(url));
    }

    fn notify_new_frame_ready(&self, _webview: WebView) {
        godot_print!("Proxy: New frame ready");
        self.event_queue.borrow_mut().push(ProxyEvent::NewFrameReady);
    }
}
