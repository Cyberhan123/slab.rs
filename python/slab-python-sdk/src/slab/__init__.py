from __future__ import annotations

from typing import Any


class _UnavailableApi:
    def request(
        self,
        method: str,
        path: str,
        headers: dict[str, str] | None = None,
        body: str | None = None,
        timeout_ms: int | None = None,
    ) -> Any:
        raise RuntimeError("slab.api is only available inside a Slab Python plugin runtime.")

    def client(self, **kwargs: Any) -> Any:
        raise RuntimeError("slab.api is only available inside a Slab Python plugin runtime.")


class _UnavailableUi:
    def emit(self, topic: str, data: Any | None = None) -> Any:
        raise RuntimeError("slab.ui is only available inside a Slab Python plugin runtime.")


plugin_id = ""
api = _UnavailableApi()
ui = _UnavailableUi()

__all__ = ["api", "plugin_id", "ui"]
