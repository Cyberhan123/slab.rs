from enum import Enum


class ImageMode(str, Enum):
    IMG2IMG = "img2img"
    TXT2IMG = "txt2img"

    def __str__(self) -> str:
        return str(self.value)
