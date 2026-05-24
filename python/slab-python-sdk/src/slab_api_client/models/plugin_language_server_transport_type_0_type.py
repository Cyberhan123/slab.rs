from enum import Enum


class PluginLanguageServerTransportType0Type(str, Enum):
    STDIO = "stdio"

    def __str__(self) -> str:
        return str(self.value)
