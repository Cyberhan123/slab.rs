from enum import Enum


class UpdateSettingOperation(str, Enum):
    SET = "set"
    UNSET = "unset"

    def __str__(self) -> str:
        return str(self.value)
