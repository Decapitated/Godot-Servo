extends Control

const DefaultBookmarks: Dictionary = {
    "DuckDuckGo": "https://duckduckgo.com",
    "Servo": "https://servo.org",
    "Servo Demos": "https://demo.servo.org",
    "Keyboard Tester": "https://keyboard-tester.com"
}

#region Main Bar
@onready var main_bar: HBoxContainer = %MainBar
@onready var back_button: Button = %BackButton
@onready var forward_button: Button = %ForwardButton
@onready var reload_button: Button = %ReloadButton
@onready var link_line_edit: LineEdit = %LinkLineEdit
#endregion
#region Bookmark Bar
@onready var bookmark_bar: HBoxContainer = %BookmarkBar
#endregion

@onready var webview: WebView = %WebView

func _ready() -> void:
    _setup_main_bar()
    _setup_bookmark_bar()

    webview.url_changed.connect(_on_url_changed)

#region Setup
func _setup_main_bar() -> void:
    back_button.pressed.connect(_on_back_button_pressed)
    forward_button.pressed.connect(_on_forward_button_pressed)
    reload_button.pressed.connect(_on_reload_button_pressed)
    link_line_edit.text_submitted.connect(_on_link_line_edit_text_submitted)

func _setup_bookmark_bar() -> void:
    for bookmark in DefaultBookmarks.keys():
        var button: Button = Button.new()
        button.text = bookmark
        button.pressed.connect(_on_bookmark_clicked.bind(DefaultBookmarks[bookmark]))
        bookmark_bar.add_child(button)
#endregion

func _on_back_button_pressed() -> void:
    webview.back()

func _on_forward_button_pressed() -> void:
    webview.forward()

func _on_reload_button_pressed() -> void:
    webview.reload()

func _on_link_line_edit_text_submitted(text: String) -> void:
    webview.load_url(text)

func _on_bookmark_clicked(url: String) -> void:
    webview.load_url(url)

func _on_url_changed(url: String) -> void:
    link_line_edit.text = url
