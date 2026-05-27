from enum import Enum


class AgentResponsesClientMessageType0Type(str, Enum):
    AGENT_SESSION_RESTORE = "agent.session.restore"

    def __str__(self) -> str:
        return str(self.value)
