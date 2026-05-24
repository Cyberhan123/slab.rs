from enum import Enum


class SubtitleFormatRequest(str, Enum):
    SRT = "srt"

    def __str__(self) -> str:
        return str(self.value)
