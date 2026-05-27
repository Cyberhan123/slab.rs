from enum import Enum


class AgentResponsesServerMessageType0Type(str, Enum):
    AGENT_ACK = "agent.ack"

    def __str__(self) -> str:
        return str(self.value)
