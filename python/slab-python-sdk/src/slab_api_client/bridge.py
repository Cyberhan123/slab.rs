from __future__ import annotations

from typing import Any, Mapping

import httpx

from .client import Client

PLUGIN_API_CLIENT_BASE_URL = "https://plugin.slab.local/"


class SlabApiTransport(httpx.BaseTransport):
    def __init__(self, api_bridge: Any, timeout_ms: int | None = None) -> None:
        self._api_bridge = api_bridge
        self._timeout_ms = timeout_ms

    def handle_request(self, request: httpx.Request) -> httpx.Response:
        body = request.content.decode("utf-8") if request.content else None
        response = self._api_bridge.request(
            request.method,
            request.url.raw_path.decode("ascii"),
            dict(request.headers),
            body,
            self._timeout_ms,
        )
        return _to_httpx_response(request, response)


class SlabApiAsyncTransport(httpx.AsyncBaseTransport):
    def __init__(self, api_bridge: Any, timeout_ms: int | None = None) -> None:
        self._api_bridge = api_bridge
        self._timeout_ms = timeout_ms

    async def handle_async_request(self, request: httpx.Request) -> httpx.Response:
        body = request.content.decode("utf-8") if request.content else None
        response = self._api_bridge.request(
            request.method,
            request.url.raw_path.decode("ascii"),
            dict(request.headers),
            body,
            self._timeout_ms,
        )
        return _to_httpx_response(request, response)


def create_client(
    api_bridge: Any | None = None,
    *,
    headers: Mapping[str, str] | None = None,
    raise_on_unexpected_status: bool = False,
    timeout_ms: int | None = None,
) -> Client:
    if api_bridge is None:
        import slab

        api_bridge = slab.api

    client = Client(
        base_url=PLUGIN_API_CLIENT_BASE_URL,
        headers=dict(headers or {}),
        raise_on_unexpected_status=raise_on_unexpected_status,
    )
    client.set_httpx_client(
        httpx.Client(
            base_url=PLUGIN_API_CLIENT_BASE_URL,
            headers=dict(headers or {}),
            transport=SlabApiTransport(api_bridge, timeout_ms),
        )
    )
    client.set_async_httpx_client(
        httpx.AsyncClient(
            base_url=PLUGIN_API_CLIENT_BASE_URL,
            headers=dict(headers or {}),
            transport=SlabApiAsyncTransport(api_bridge, timeout_ms),
        )
    )
    return client


def _to_httpx_response(request: httpx.Request, response: Mapping[str, Any]) -> httpx.Response:
    body = response.get("body") or ""
    if not isinstance(body, str):
        body = str(body)
    return httpx.Response(
        int(response["status"]),
        headers=response.get("headers") or {},
        content=body.encode("utf-8"),
        request=request,
    )
