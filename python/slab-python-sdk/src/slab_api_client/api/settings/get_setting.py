from http import HTTPStatus
from typing import Any, cast
from urllib.parse import quote

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.setting_property_view import SettingPropertyView
from ...types import Response


def _get_kwargs(
    pmid: str,
) -> dict[str, Any]:

    _kwargs: dict[str, Any] = {
        "method": "get",
        "url": "/v1/settings/{pmid}".format(
            pmid=quote(str(pmid), safe=""),
        ),
    }

    return _kwargs


def _parse_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Any | SettingPropertyView | None:
    if response.status_code == 200:
        response_200 = SettingPropertyView.from_dict(response.json())

        return response_200

    if response.status_code == 401:
        response_401 = cast(Any, None)
        return response_401

    if response.status_code == 404:
        response_404 = cast(Any, None)
        return response_404

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Response[Any | SettingPropertyView]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    pmid: str,
    *,
    client: AuthenticatedClient | Client,
) -> Response[Any | SettingPropertyView]:
    """
    Args:
        pmid (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Any | SettingPropertyView]
    """

    kwargs = _get_kwargs(
        pmid=pmid,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    pmid: str,
    *,
    client: AuthenticatedClient | Client,
) -> Any | SettingPropertyView | None:
    """
    Args:
        pmid (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Any | SettingPropertyView
    """

    return sync_detailed(
        pmid=pmid,
        client=client,
    ).parsed


async def asyncio_detailed(
    pmid: str,
    *,
    client: AuthenticatedClient | Client,
) -> Response[Any | SettingPropertyView]:
    """
    Args:
        pmid (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Any | SettingPropertyView]
    """

    kwargs = _get_kwargs(
        pmid=pmid,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    pmid: str,
    *,
    client: AuthenticatedClient | Client,
) -> Any | SettingPropertyView | None:
    """
    Args:
        pmid (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Any | SettingPropertyView
    """

    return (
        await asyncio_detailed(
            pmid=pmid,
            client=client,
        )
    ).parsed
