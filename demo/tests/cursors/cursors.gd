extends Control

const ICON_SVG: Texture2D = preload("res://icon.svg")

@onready var cursors_container: Container = %Cursors

func _ready() -> void:
    var enum_values: PackedStringArray = ClassDB.class_get_enum_constants("Control", "CursorShape", true)
    for shape_name in enum_values:
        var value: int = ClassDB.class_get_integer_constant("Control", shape_name)
        var tester: TextureRect = _create_cursor_tester(value)
        cursors_container.add_child(tester)
        
        var label: Label = Label.new()
        label.text = shape_name
        tester.add_child(label)

func _create_cursor_tester(shape: CursorShape) -> TextureRect:
    var cursor_tester: TextureRect = TextureRect.new()
    cursor_tester.texture = ICON_SVG
    cursor_tester.mouse_default_cursor_shape = shape
    return cursor_tester
