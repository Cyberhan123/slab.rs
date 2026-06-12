from enum import Enum


class PluginAgentHookLifecycleEvent(str, Enum):
    ON_AGENT_END = "on_agent_end"
    ON_AGENT_START = "on_agent_start"
    ON_LLM_END = "on_llm_end"
    ON_LLM_START = "on_llm_start"
    ON_TOOL_END = "on_tool_end"
    ON_TOOL_START = "on_tool_start"

    def __str__(self) -> str:
        return str(self.value)
