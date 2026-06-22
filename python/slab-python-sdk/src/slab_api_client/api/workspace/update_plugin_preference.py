from http import HTTPStatus
from typing import Any, cast
from urllib.parse import quote

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.workspace_plugin_preference_update import WorkspacePluginPreferenceUpdate
from ...models.workspace_state_response import WorkspaceStateResponse
from ...types import Response


def _get_kwargs(
    plugin_id: str,
    *,
    body: WorkspacePluginPreferenceUpdate,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}

    _kwargs: dict[str, Any] = {
        "method": "put",
        "url": "/v1/workspace/plugins/{plugin_id}/preference".format(
            plugin_id=quote(str(plugin_id), safe=""),
        ),
    }

    _kwargs["json"] = body.to_dict()

    headers["Content-Type"] = "application/json"

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Any | WorkspaceStateResponse | None:
    if response.status_code == 200:
        response_200 = WorkspaceStateResponse.from_dict(response.json())

        return response_200

    if response.status_code == 400:
        response_400 = cast(Any, None)
        return response_400

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Response[Any | WorkspaceStateResponse]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    plugin_id: str,
    *,
    client: AuthenticatedClient | Client,
    body: WorkspacePluginPreferenceUpdate,
) -> Response[Any | WorkspaceStateResponse]:
    """
    Args:
        plugin_id (str):
        body (WorkspacePluginPreferenceUpdate):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Any | WorkspaceStateResponse]
    """

    kwargs = _get_kwargs(
        plugin_id=plugin_id,
        body=body,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    plugin_id: str,
    *,
    client: AuthenticatedClient | Client,
    body: WorkspacePluginPreferenceUpdate,
) -> Any | WorkspaceStateResponse | None:
    """
    Args:
        plugin_id (str):
        body (WorkspacePluginPreferenceUpdate):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Any | WorkspaceStateResponse
    """

    return sync_detailed(
        plugin_id=plugin_id,
        client=client,
        body=body,
    ).parsed


async def asyncio_detailed(
    plugin_id: str,
    *,
    client: AuthenticatedClient | Client,
    body: WorkspacePluginPreferenceUpdate,
) -> Response[Any | WorkspaceStateResponse]:
    """
    Args:
        plugin_id (str):
        body (WorkspacePluginPreferenceUpdate):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Any | WorkspaceStateResponse]
    """

    kwargs = _get_kwargs(
        plugin_id=plugin_id,
        body=body,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    plugin_id: str,
    *,
    client: AuthenticatedClient | Client,
    body: WorkspacePluginPreferenceUpdate,
) -> Any | WorkspaceStateResponse | None:
    """
    Args:
        plugin_id (str):
        body (WorkspacePluginPreferenceUpdate):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Any | WorkspaceStateResponse
    """

    return (
        await asyncio_detailed(
            plugin_id=plugin_id,
            client=client,
            body=body,
        )
    ).parsed
