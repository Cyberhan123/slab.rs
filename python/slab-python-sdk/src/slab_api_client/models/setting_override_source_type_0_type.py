from enum import Enum


class SettingOverrideSourceType0Type(str, Enum):
    ENV = "env"

    def __str__(self) -> str:
        return str(self.value)
