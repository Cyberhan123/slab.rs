from http import HTTPStatus
from typing import Any, cast

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.agent_responses_client_message_type_0 import (
    AgentResponsesClientMessageType0,
)
from ...models.agent_responses_client_message_type_1 import (
    AgentResponsesClientMessageType1,
)
from ...models.agent_responses_client_message_type_2 import (
    AgentResponsesClientMessageType2,
)
from ...models.agent_responses_client_message_type_3 import (
    AgentResponsesClientMessageType3,
)
from ...models.agent_responses_client_message_type_4 import (
    AgentResponsesClientMessageType4,
)
from ...models.agent_responses_client_message_type_5 import (
    AgentResponsesClientMessageType5,
)
from ...models.agent_responses_server_message_type_0 import (
    AgentResponsesServerMessageType0,
)
from ...models.agent_responses_server_message_type_1 import (
    AgentResponsesServerMessageType1,
)
from ...models.agent_responses_server_message_type_2 import (
    AgentResponsesServerMessageType2,
)
from ...types import Response


def _get_kwargs(
    *,
    body: AgentResponsesClientMessageType0
    | AgentResponsesClientMessageType1
    | AgentResponsesClientMessageType2
    | AgentResponsesClientMessageType3
    | AgentResponsesClientMessageType4
    | AgentResponsesClientMessageType5,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}

    _kwargs: dict[str, Any] = {
        "method": "post",
        "url": "/v1/agents/responses",
    }

    if isinstance(body, AgentResponsesClientMessageType0):
        _kwargs["json"] = body.to_dict()
    elif isinstance(body, AgentResponsesClientMessageType1):
        _kwargs["json"] = body.to_dict()
    elif isinstance(body, AgentResponsesClientMessageType2):
        _kwargs["json"] = body.to_dict()
    elif isinstance(body, AgentResponsesClientMessageType3):
        _kwargs["json"] = body.to_dict()
    elif isinstance(body, AgentResponsesClientMessageType4):
        _kwargs["json"] = body.to_dict()
    else:
        _kwargs["json"] = body.to_dict()

    headers["Content-Type"] = "application/json"

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> (
    AgentResponsesServerMessageType0
    | AgentResponsesServerMessageType1
    | AgentResponsesServerMessageType2
    | Any
    | None
):
    if response.status_code == 200:

        def _parse_response_200(
            data: object,
        ) -> (
            AgentResponsesServerMessageType0
            | AgentResponsesServerMessageType1
            | AgentResponsesServerMessageType2
        ):
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                componentsschemas_agent_responses_server_message_type_0 = (
                    AgentResponsesServerMessageType0.from_dict(data)
                )

                return componentsschemas_agent_responses_server_message_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                componentsschemas_agent_responses_server_message_type_1 = (
                    AgentResponsesServerMessageType1.from_dict(data)
                )

                return componentsschemas_agent_responses_server_message_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            if not isinstance(data, dict):
                raise TypeError()
            componentsschemas_agent_responses_server_message_type_2 = (
                AgentResponsesServerMessageType2.from_dict(data)
            )

            return componentsschemas_agent_responses_server_message_type_2

        response_200 = _parse_response_200(response.json())

        return response_200

    if response.status_code == 400:
        response_400 = cast(Any, None)
        return response_400

    if response.status_code == 404:
        response_404 = cast(Any, None)
        return response_404

    if response.status_code == 429:
        response_429 = cast(Any, None)
        return response_429

    if response.status_code == 500:
        response_500 = cast(Any, None)
        return response_500

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Response[
    AgentResponsesServerMessageType0
    | AgentResponsesServerMessageType1
    | AgentResponsesServerMessageType2
    | Any
]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient | Client,
    body: AgentResponsesClientMessageType0
    | AgentResponsesClientMessageType1
    | AgentResponsesClientMessageType2
    | AgentResponsesClientMessageType3
    | AgentResponsesClientMessageType4
    | AgentResponsesClientMessageType5,
) -> Response[
    AgentResponsesServerMessageType0
    | AgentResponsesServerMessageType1
    | AgentResponsesServerMessageType2
    | Any
]:
    """
    Args:
        body (AgentResponsesClientMessageType0 | AgentResponsesClientMessageType1 |
            AgentResponsesClientMessageType2 | AgentResponsesClientMessageType3 |
            AgentResponsesClientMessageType4 | AgentResponsesClientMessageType5): Client message
            accepted by `GET` WebSocket and `POST /v1/agents/responses`.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[AgentResponsesServerMessageType0 | AgentResponsesServerMessageType1 | AgentResponsesServerMessageType2 | Any]
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
    body: AgentResponsesClientMessageType0
    | AgentResponsesClientMessageType1
    | AgentResponsesClientMessageType2
    | AgentResponsesClientMessageType3
    | AgentResponsesClientMessageType4
    | AgentResponsesClientMessageType5,
) -> (
    AgentResponsesServerMessageType0
    | AgentResponsesServerMessageType1
    | AgentResponsesServerMessageType2
    | Any
    | None
):
    """
    Args:
        body (AgentResponsesClientMessageType0 | AgentResponsesClientMessageType1 |
            AgentResponsesClientMessageType2 | AgentResponsesClientMessageType3 |
            AgentResponsesClientMessageType4 | AgentResponsesClientMessageType5): Client message
            accepted by `GET` WebSocket and `POST /v1/agents/responses`.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        AgentResponsesServerMessageType0 | AgentResponsesServerMessageType1 | AgentResponsesServerMessageType2 | Any
    """

    return sync_detailed(
        client=client,
        body=body,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient | Client,
    body: AgentResponsesClientMessageType0
    | AgentResponsesClientMessageType1
    | AgentResponsesClientMessageType2
    | AgentResponsesClientMessageType3
    | AgentResponsesClientMessageType4
    | AgentResponsesClientMessageType5,
) -> Response[
    AgentResponsesServerMessageType0
    | AgentResponsesServerMessageType1
    | AgentResponsesServerMessageType2
    | Any
]:
    """
    Args:
        body (AgentResponsesClientMessageType0 | AgentResponsesClientMessageType1 |
            AgentResponsesClientMessageType2 | AgentResponsesClientMessageType3 |
            AgentResponsesClientMessageType4 | AgentResponsesClientMessageType5): Client message
            accepted by `GET` WebSocket and `POST /v1/agents/responses`.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[AgentResponsesServerMessageType0 | AgentResponsesServerMessageType1 | AgentResponsesServerMessageType2 | Any]
    """

    kwargs = _get_kwargs(
        body=body,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient | Client,
    body: AgentResponsesClientMessageType0
    | AgentResponsesClientMessageType1
    | AgentResponsesClientMessageType2
    | AgentResponsesClientMessageType3
    | AgentResponsesClientMessageType4
    | AgentResponsesClientMessageType5,
) -> (
    AgentResponsesServerMessageType0
    | AgentResponsesServerMessageType1
    | AgentResponsesServerMessageType2
    | Any
    | None
):
    """
    Args:
        body (AgentResponsesClientMessageType0 | AgentResponsesClientMessageType1 |
            AgentResponsesClientMessageType2 | AgentResponsesClientMessageType3 |
            AgentResponsesClientMessageType4 | AgentResponsesClientMessageType5): Client message
            accepted by `GET` WebSocket and `POST /v1/agents/responses`.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        AgentResponsesServerMessageType0 | AgentResponsesServerMessageType1 | AgentResponsesServerMessageType2 | Any
    """

    return (
        await asyncio_detailed(
            client=client,
            body=body,
        )
    ).parsed
