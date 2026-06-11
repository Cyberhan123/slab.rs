from enum import Enum


class AgentToolChoiceInputType2Type(str, Enum):
    REQUIRED = "required"

    def __str__(self) -> str:
        return str(self.value)
