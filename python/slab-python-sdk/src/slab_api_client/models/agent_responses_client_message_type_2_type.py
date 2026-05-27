from enum import Enum


class AgentResponsesClientMessageType2Type(str, Enum):
    AGENT_INPUT = "agent.input"

    def __str__(self) -> str:
        return str(self.value)
