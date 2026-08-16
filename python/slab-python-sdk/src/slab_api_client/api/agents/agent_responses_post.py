from http import HTTPStatus
from typing import Any, cast

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.open_ai_create_request import OpenAICreateRequest
from ...models.open_ai_error_response import OpenAiErrorResponse
from ...types import Response


def _get_kwargs(
    *,
    body: OpenAICreateRequest,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}

    _kwargs: dict[str, Any] = {
        "method": "post",
        "url": "/v1/agents/responses",
    }

    _kwargs["json"] = body.to_dict()

    headers["Content-Type"] = "application/json"

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Any | OpenAiErrorResponse | None:
    if response.status_code == 200:
        response_200 = cast(Any, None)
        return response_200

    if response.status_code == 400:
        response_400 = OpenAiErrorResponse.from_dict(response.json())

        return response_400

    if response.status_code == 404:
        response_404 = OpenAiErrorResponse.from_dict(response.json())

        return response_404

    if response.status_code == 429:
        response_429 = OpenAiErrorResponse.from_dict(response.json())

        return response_429

    if response.status_code == 500:
        response_500 = OpenAiErrorResponse.from_dict(response.json())

        return response_500

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
    body: OpenAICreateRequest,
) -> Response[Any | OpenAiErrorResponse]:
    """
    Args:
        body (OpenAICreateRequest): `POST /v1/agents/responses` body as sent by the official
            `openai` SDK
            (`ResponseCreateParamsBase`). Slab translates `input` + a subset of config;
            unknown fields are ignored (no `deny_unknown_fields`) so future SDK fields
            don't break the server. `input` is held as a `serde_json::Value` (a string
            or an array of input items) so the type is `ToSchema`-derivable.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Any | OpenAiErrorResponse]
    """

    kwargs = _get_kwargs(
        body=body,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    *,
    client: AuthenticatedClient | Client,
    body: OpenAICreateRequest,
) -> Any | OpenAiErrorResponse | None:
    """
    Args:
        body (OpenAICreateRequest): `POST /v1/agents/responses` body as sent by the official
            `openai` SDK
            (`ResponseCreateParamsBase`). Slab translates `input` + a subset of config;
            unknown fields are ignored (no `deny_unknown_fields`) so future SDK fields
            don't break the server. `input` is held as a `serde_json::Value` (a string
            or an array of input items) so the type is `ToSchema`-derivable.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Any | OpenAiErrorResponse
    """

    return sync_detailed(
        client=client,
        body=body,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient | Client,
    body: OpenAICreateRequest,
) -> Response[Any | OpenAiErrorResponse]:
    """
    Args:
        body (OpenAICreateRequest): `POST /v1/agents/responses` body as sent by the official
            `openai` SDK
            (`ResponseCreateParamsBase`). Slab translates `input` + a subset of config;
            unknown fields are ignored (no `deny_unknown_fields`) so future SDK fields
            don't break the server. `input` is held as a `serde_json::Value` (a string
            or an array of input items) so the type is `ToSchema`-derivable.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Any | OpenAiErrorResponse]
    """

    kwargs = _get_kwargs(
        body=body,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient | Client,
    body: OpenAICreateRequest,
) -> Any | OpenAiErrorResponse | None:
    """
    Args:
        body (OpenAICreateRequest): `POST /v1/agents/responses` body as sent by the official
            `openai` SDK
            (`ResponseCreateParamsBase`). Slab translates `input` + a subset of config;
            unknown fields are ignored (no `deny_unknown_fields`) so future SDK fields
            don't break the server. `input` is held as a `serde_json::Value` (a string
            or an array of input items) so the type is `ToSchema`-derivable.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Any | OpenAiErrorResponse
    """

    return (
        await asyncio_detailed(
            client=client,
            body=body,
        )
    ).parsed
