from enum import Enum


class AgentResponsesServerMessageType2Type(str, Enum):
    AGENT_ERROR = "agent.error"

    def __str__(self) -> str:
        return str(self.value)
