from enum import Enum


class PluginLanguageServerTransportType1Type(str, Enum):
    WEBSOCKET = "webSocket"

    def __str__(self) -> str:
        return str(self.value)
