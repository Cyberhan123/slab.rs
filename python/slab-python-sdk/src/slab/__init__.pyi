from typing import Any, Protocol

from slab_api_client.client import Client


class ApiBridge(Protocol):
    def request(
        self,
        method: str,
        path: str,
        headers: dict[str, str] | None = None,
        body: str | None = None,
        timeout_ms: int | None = None,
    ) -> dict[str, Any]: ...

    def client(
        self,
        *,
        raise_on_unexpected_status: bool = False,
        timeout_ms: int | None = None,
    ) -> Client: ...


class UiBridge(Protocol):
    def emit(self, topic: str, data: Any | None = None) -> Any: ...


plugin_id: str
api: ApiBridge
ui: UiBridge
