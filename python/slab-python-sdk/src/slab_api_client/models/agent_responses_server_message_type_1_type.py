from enum import Enum


class AgentResponsesServerMessageType1Type(str, Enum):
    AGENT_SESSION_RESTORED = "agent.session.restored"

    def __str__(self) -> str:
        return str(self.value)
