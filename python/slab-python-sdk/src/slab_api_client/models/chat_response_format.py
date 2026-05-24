from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.chat_response_format_type import ChatResponseFormatType
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.chat_response_json_schema import ChatResponseJsonSchema


T = TypeVar("T", bound="ChatResponseFormat")


@_attrs_define
class ChatResponseFormat:
    """
    Attributes:
        type_ (ChatResponseFormatType):
        json_schema (ChatResponseJsonSchema | None | Unset):
        schema (Any | Unset):
    """

    type_: ChatResponseFormatType
    json_schema: ChatResponseJsonSchema | None | Unset = UNSET
    schema: Any | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.chat_response_json_schema import ChatResponseJsonSchema

        type_ = self.type_.value

        json_schema: dict[str, Any] | None | Unset
        if isinstance(self.json_schema, Unset):
            json_schema = UNSET
        elif isinstance(self.json_schema, ChatResponseJsonSchema):
            json_schema = self.json_schema.to_dict()
        else:
            json_schema = self.json_schema

        schema = self.schema

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "type": type_,
            }
        )
        if json_schema is not UNSET:
            field_dict["json_schema"] = json_schema
        if schema is not UNSET:
            field_dict["schema"] = schema

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.chat_response_json_schema import ChatResponseJsonSchema

        d = dict(src_dict)
        type_ = ChatResponseFormatType(d.pop("type"))

        def _parse_json_schema(data: object) -> ChatResponseJsonSchema | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                json_schema_type_1 = ChatResponseJsonSchema.from_dict(data)

                return json_schema_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(ChatResponseJsonSchema | None | Unset, data)

        json_schema = _parse_json_schema(d.pop("json_schema", UNSET))

        schema = d.pop("schema", UNSET)

        chat_response_format = cls(
            type_=type_,
            json_schema=json_schema,
            schema=schema,
        )

        chat_response_format.additional_properties = d
        return chat_response_format

    @property
    def additional_keys(self) -> list[str]:
        return list(self.additional_properties.keys())

    def __getitem__(self, key: str) -> Any:
        return self.additional_properties[key]

    def __setitem__(self, key: str, value: Any) -> None:
        self.additional_properties[key] = value

    def __delitem__(self, key: str) -> None:
        del self.additional_properties[key]

    def __contains__(self, key: str) -> bool:
        return key in self.additional_properties
