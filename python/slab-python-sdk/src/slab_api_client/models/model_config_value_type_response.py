from enum import Enum


class ModelConfigValueTypeResponse(str, Enum):
    BOOLEAN = "boolean"
    INTEGER = "integer"
    JSON = "json"
    NUMBER = "number"
    PATH = "path"
    STRING = "string"

    def __str__(self) -> str:
        return str(self.value)
