from enum import Enum


class ModelCapability(str, Enum):
    AUDIO_TRANSCRIPTION = "audio_transcription"
    AUDIO_VAD = "audio_vad"
    CHAT_GENERATION = "chat_generation"
    IMAGE_EMBEDDING = "image_embedding"
    IMAGE_GENERATION = "image_generation"
    TEXT_GENERATION = "text_generation"
    VIDEO_GENERATION = "video_generation"

    def __str__(self) -> str:
        return str(self.value)
