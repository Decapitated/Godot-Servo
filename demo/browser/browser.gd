extends Control

@onready var back_button: Button = %BackButton
@onready var forward_button: Button = %ForwardButton
@onready var reload_button: Button = %ReloadButton
@onready var link_line_edit: LineEdit = %LinkLineEdit
@onready var webview: WebView = %WebView

func _ready() -> void:
    link_line_edit.text_submitted.connect(_on_link_line_edit_text_submitted)

    back_button.pressed.connect(_on_back_button_pressed)
    forward_button.pressed.connect(_on_forward_button_pressed)
    reload_button.pressed.connect(_on_reload_button_pressed)

    webview.url_changed.connect(_on_url_changed)

func _on_link_line_edit_text_submitted(text: String) -> void:
    webview.load_url(text)

func _on_url_changed(url: String) -> void:
    link_line_edit.text = url

func _on_back_button_pressed() -> void:
    webview.back()

func _on_forward_button_pressed() -> void:
    webview.forward()

func _on_reload_button_pressed() -> void:
    webview.reload()
