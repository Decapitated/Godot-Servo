use std::rc::Rc;

use godot::{classes::{Control, Engine, IControl}, prelude::*};
use servo::{WebView, WebViewBuilder, WebViewDelegate};
use url::Url;

use crate::servo_manager::ServoManager;

#[derive(GodotClass)]
#[class(base=Control, init, tool, rename=WebView)]
struct WebViewControl {
    base: Base<Control>,
    webview: Option<WebView>
}

#[godot_api]
impl IControl for WebViewControl {
    // fn init(base: Base<Control>) -> Self {
    //     Self {
    //         base,
    //         webview: None
    //     }      
    // }

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
                    control: self.to_gd()
                }))
                .url(Url::parse("https://google.com").expect("Failed to parse url"))
                .build();
        self.webview = Some(webview);
        godot_print!("Ready");
    }
}

impl WebViewControl {
    pub fn update(&mut self) {
        godot_print!("Updating");
    }
}

struct Proxy {
    control: Gd<WebViewControl>
}

impl WebViewDelegate for Proxy {
    fn notify_new_frame_ready(&self, _webview: WebView) {
        godot_print!("Frame ready");
        self.control.clone().bind_mut().update();
    }
}
