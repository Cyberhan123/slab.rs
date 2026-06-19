from enum import Enum


class SettingOverrideSourceType1Type(str, Enum):
    PARENT = "parent"

    def __str__(self) -> str:
        return str(self.value)
