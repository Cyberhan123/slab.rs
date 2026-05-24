from enum import Enum


class ModelConfigFieldScopeResponse(str, Enum):
    ADVANCED = "advanced"
    INFERENCE = "inference"
    LOAD = "load"
    SOURCE = "source"
    SUMMARY = "summary"

    def __str__(self) -> str:
        return str(self.value)
