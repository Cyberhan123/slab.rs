from enum import Enum


class SettingValueType(str, Enum):
    ARRAY = "array"
    BOOLEAN = "boolean"
    INTEGER = "integer"
    OBJECT = "object"
    STRING = "string"

    def __str__(self) -> str:
        return str(self.value)
