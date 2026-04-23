extends Control

@onready var back_button: Button = %BackButton
@onready var forward_button: Button = %ForwardButton
@onready var reload_button: Button = %ReloadButton
@onready var link_line_edit: LineEdit = %LinkLineEdit
@onready var webview: WebView = %WebView

func _ready() -> void:
    link_line_edit.text_submitted.connect(_on_link_line_edit_text_submitted)

func _on_link_line_edit_text_submitted(text: String) -> void:
    webview.load_url(text)
