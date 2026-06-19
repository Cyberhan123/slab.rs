from enum import Enum


class SettingValueType(str, Enum):
    ARRAY = "array"
    BOOLEAN = "boolean"
    FLOAT = "float"
    INTEGER = "integer"
    OBJECT = "object"
    STRING = "string"
    TAGGED_UNION = "tagged_union"
    UNSIGNED = "unsigned"

    def __str__(self) -> str:
        return str(self.value)
