from enum import Enum


class SubtitleVariantRequest(str, Enum):
    SOURCE = "source"
    TRANSLATED = "translated"

    def __str__(self) -> str:
        return str(self.value)
