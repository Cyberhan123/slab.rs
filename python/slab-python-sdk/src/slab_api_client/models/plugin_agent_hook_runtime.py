from enum import Enum


class PluginAgentHookRuntime(str, Enum):
    JAVASCRIPT = "javascript"
    PYTHON = "python"

    def __str__(self) -> str:
        return str(self.value)
