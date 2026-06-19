from enum import Enum


class SettingChangeEffect(str, Enum):
    LIVE = "live"
    NEEDS_MODEL_RELOAD = "needs_model_reload"
    NEEDS_RESTART = "needs_restart"
    NONE = "none"

    def __str__(self) -> str:
        return str(self.value)
