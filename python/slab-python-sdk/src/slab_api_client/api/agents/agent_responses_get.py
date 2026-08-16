from http import HTTPStatus
from typing import Any, cast

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.open_ai_error_response import OpenAiErrorResponse
from ...types import UNSET, Response, Unset


def _get_kwargs(
    *,
    transport: str | Unset = UNSET,
    thread_id: str | Unset = UNSET,
) -> dict[str, Any]:

    params: dict[str, Any] = {}

    params["transport"] = transport

    params["thread_id"] = thread_id

    params = {k: v for k, v in params.items() if v is not UNSET and v is not None}

    _kwargs: dict[str, Any] = {
        "method": "get",
        "url": "/v1/agents/responses",
        "params": params,
    }

    return _kwargs


def _parse_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Any | OpenAiErrorResponse | None:
    if response.status_code == 101:
        response_101 = cast(Any, None)
        return response_101

    if response.status_code == 200:
        response_200 = cast(Any, None)
        return response_200

    if response.status_code == 400:
        response_400 = OpenAiErrorResponse.from_dict(response.json())

        return response_400

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Response[Any | OpenAiErrorResponse]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient | Client,
    transport: str | Unset = UNSET,
    thread_id: str | Unset = UNSET,
) -> Response[Any | OpenAiErrorResponse]:
    """
    Args:
        transport (str | Unset):
        thread_id (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Any | OpenAiErrorResponse]
    """

    kwargs = _get_kwargs(
        transport=transport,
        thread_id=thread_id,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    *,
    client: AuthenticatedClient | Client,
    transport: str | Unset = UNSET,
    thread_id: str | Unset = UNSET,
) -> Any | OpenAiErrorResponse | None:
    """
    Args:
        transport (str | Unset):
        thread_id (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Any | OpenAiErrorResponse
    """

    return sync_detailed(
        client=client,
        transport=transport,
        thread_id=thread_id,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient | Client,
    transport: str | Unset = UNSET,
    thread_id: str | Unset = UNSET,
) -> Response[Any | OpenAiErrorResponse]:
    """
    Args:
        transport (str | Unset):
        thread_id (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Any | OpenAiErrorResponse]
    """

    kwargs = _get_kwargs(
        transport=transport,
        thread_id=thread_id,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient | Client,
    transport: str | Unset = UNSET,
    thread_id: str | Unset = UNSET,
) -> Any | OpenAiErrorResponse | None:
    """
    Args:
        transport (str | Unset):
        thread_id (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Any | OpenAiErrorResponse
    """

    return (
        await asyncio_detailed(
            client=client,
            transport=transport,
            thread_id=thread_id,
        )
    ).parsed
