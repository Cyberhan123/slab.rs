from http import HTTPStatus
from typing import Any

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.completion_request import CompletionRequest
from ...models.completion_response import CompletionResponse
from ...models.open_ai_error_response import OpenAiErrorResponse
from ...types import Response


def _get_kwargs(
    *,
    body: CompletionRequest,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}

    _kwargs: dict[str, Any] = {
        "method": "post",
        "url": "/v1/completions",
    }

    _kwargs["json"] = body.to_dict()

    headers["Content-Type"] = "application/json"

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> CompletionResponse | OpenAiErrorResponse | None:
    if response.status_code == 200:
        response_200 = CompletionResponse.from_dict(response.json())

        return response_200

    if response.status_code == 400:
        response_400 = OpenAiErrorResponse.from_dict(response.json())

        return response_400

    if response.status_code == 500:
        response_500 = OpenAiErrorResponse.from_dict(response.json())

        return response_500

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Response[CompletionResponse | OpenAiErrorResponse]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient | Client,
    body: CompletionRequest,
) -> Response[CompletionResponse | OpenAiErrorResponse]:
    """
    Args:
        body (CompletionRequest): Request body for `POST /v1/completions`.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[CompletionResponse | OpenAiErrorResponse]
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
    body: CompletionRequest,
) -> CompletionResponse | OpenAiErrorResponse | None:
    """
    Args:
        body (CompletionRequest): Request body for `POST /v1/completions`.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        CompletionResponse | OpenAiErrorResponse
    """

    return sync_detailed(
        client=client,
        body=body,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient | Client,
    body: CompletionRequest,
) -> Response[CompletionResponse | OpenAiErrorResponse]:
    """
    Args:
        body (CompletionRequest): Request body for `POST /v1/completions`.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[CompletionResponse | OpenAiErrorResponse]
    """

    kwargs = _get_kwargs(
        body=body,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient | Client,
    body: CompletionRequest,
) -> CompletionResponse | OpenAiErrorResponse | None:
    """
    Args:
        body (CompletionRequest): Request body for `POST /v1/completions`.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        CompletionResponse | OpenAiErrorResponse
    """

    return (
        await asyncio_detailed(
            client=client,
            body=body,
        )
    ).parsed
