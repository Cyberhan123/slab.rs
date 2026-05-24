from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.chat_content_part_type_0 import ChatContentPartType0
    from ..models.chat_content_part_type_1 import ChatContentPartType1
    from ..models.chat_content_part_type_2 import ChatContentPartType2
    from ..models.chat_content_part_type_3 import ChatContentPartType3
    from ..models.chat_content_part_type_4 import ChatContentPartType4
    from ..models.chat_content_part_type_5 import ChatContentPartType5
    from ..models.chat_content_part_type_6 import ChatContentPartType6
    from ..models.chat_tool_call import ChatToolCall


T = TypeVar("T", bound="ChatMessage")


@_attrs_define
class ChatMessage:
    """A single message in the conversation history.

    Attributes:
        role (str): The role of the message author.
        content (list[ChatContentPartType0 | ChatContentPartType1 | ChatContentPartType2 | ChatContentPartType3 |
            ChatContentPartType4 | ChatContentPartType5 | ChatContentPartType6] | None | str | Unset):
        name (None | str | Unset): Optional participant name for providers that support named turns.
        tool_call_id (None | str | Unset): Tool call id for tool result messages.
        tool_calls (list[ChatToolCall] | Unset): Assistant-emitted tool calls.
    """

    role: str
    content: (
        list[
            ChatContentPartType0
            | ChatContentPartType1
            | ChatContentPartType2
            | ChatContentPartType3
            | ChatContentPartType4
            | ChatContentPartType5
            | ChatContentPartType6
        ]
        | None
        | str
        | Unset
    ) = UNSET
    name: None | str | Unset = UNSET
    tool_call_id: None | str | Unset = UNSET
    tool_calls: list[ChatToolCall] | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.chat_content_part_type_0 import ChatContentPartType0
        from ..models.chat_content_part_type_1 import ChatContentPartType1
        from ..models.chat_content_part_type_2 import ChatContentPartType2
        from ..models.chat_content_part_type_3 import ChatContentPartType3
        from ..models.chat_content_part_type_4 import ChatContentPartType4
        from ..models.chat_content_part_type_5 import ChatContentPartType5

        role = self.role

        content: list[dict[str, Any]] | None | str | Unset
        if isinstance(self.content, Unset):
            content = UNSET
        elif isinstance(self.content, list):
            content = []
            for componentsschemas_chat_message_content_type_1_item_data in self.content:
                componentsschemas_chat_message_content_type_1_item: dict[str, Any]
                if isinstance(
                    componentsschemas_chat_message_content_type_1_item_data,
                    ChatContentPartType0,
                ):
                    componentsschemas_chat_message_content_type_1_item = componentsschemas_chat_message_content_type_1_item_data.to_dict()
                elif isinstance(
                    componentsschemas_chat_message_content_type_1_item_data,
                    ChatContentPartType1,
                ):
                    componentsschemas_chat_message_content_type_1_item = componentsschemas_chat_message_content_type_1_item_data.to_dict()
                elif isinstance(
                    componentsschemas_chat_message_content_type_1_item_data,
                    ChatContentPartType2,
                ):
                    componentsschemas_chat_message_content_type_1_item = componentsschemas_chat_message_content_type_1_item_data.to_dict()
                elif isinstance(
                    componentsschemas_chat_message_content_type_1_item_data,
                    ChatContentPartType3,
                ):
                    componentsschemas_chat_message_content_type_1_item = componentsschemas_chat_message_content_type_1_item_data.to_dict()
                elif isinstance(
                    componentsschemas_chat_message_content_type_1_item_data,
                    ChatContentPartType4,
                ):
                    componentsschemas_chat_message_content_type_1_item = componentsschemas_chat_message_content_type_1_item_data.to_dict()
                elif isinstance(
                    componentsschemas_chat_message_content_type_1_item_data,
                    ChatContentPartType5,
                ):
                    componentsschemas_chat_message_content_type_1_item = componentsschemas_chat_message_content_type_1_item_data.to_dict()
                else:
                    componentsschemas_chat_message_content_type_1_item = componentsschemas_chat_message_content_type_1_item_data.to_dict()

                content.append(componentsschemas_chat_message_content_type_1_item)

        else:
            content = self.content

        name: None | str | Unset
        if isinstance(self.name, Unset):
            name = UNSET
        else:
            name = self.name

        tool_call_id: None | str | Unset
        if isinstance(self.tool_call_id, Unset):
            tool_call_id = UNSET
        else:
            tool_call_id = self.tool_call_id

        tool_calls: list[dict[str, Any]] | Unset = UNSET
        if not isinstance(self.tool_calls, Unset):
            tool_calls = []
            for tool_calls_item_data in self.tool_calls:
                tool_calls_item = tool_calls_item_data.to_dict()
                tool_calls.append(tool_calls_item)

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "role": role,
            }
        )
        if content is not UNSET:
            field_dict["content"] = content
        if name is not UNSET:
            field_dict["name"] = name
        if tool_call_id is not UNSET:
            field_dict["tool_call_id"] = tool_call_id
        if tool_calls is not UNSET:
            field_dict["tool_calls"] = tool_calls

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.chat_content_part_type_0 import ChatContentPartType0
        from ..models.chat_content_part_type_1 import ChatContentPartType1
        from ..models.chat_content_part_type_2 import ChatContentPartType2
        from ..models.chat_content_part_type_3 import ChatContentPartType3
        from ..models.chat_content_part_type_4 import ChatContentPartType4
        from ..models.chat_content_part_type_5 import ChatContentPartType5
        from ..models.chat_content_part_type_6 import ChatContentPartType6
        from ..models.chat_tool_call import ChatToolCall

        d = dict(src_dict)
        role = d.pop("role")

        def _parse_content(
            data: object,
        ) -> (
            list[
                ChatContentPartType0
                | ChatContentPartType1
                | ChatContentPartType2
                | ChatContentPartType3
                | ChatContentPartType4
                | ChatContentPartType5
                | ChatContentPartType6
            ]
            | None
            | str
            | Unset
        ):
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, list):
                    raise TypeError()
                componentsschemas_chat_message_content_type_1 = []
                _componentsschemas_chat_message_content_type_1 = data
                for (
                    componentsschemas_chat_message_content_type_1_item_data
                ) in _componentsschemas_chat_message_content_type_1:

                    def _parse_componentsschemas_chat_message_content_type_1_item(
                        data: object,
                    ) -> (
                        ChatContentPartType0
                        | ChatContentPartType1
                        | ChatContentPartType2
                        | ChatContentPartType3
                        | ChatContentPartType4
                        | ChatContentPartType5
                        | ChatContentPartType6
                    ):
                        try:
                            if not isinstance(data, dict):
                                raise TypeError()
                            componentsschemas_chat_content_part_type_0 = (
                                ChatContentPartType0.from_dict(data)
                            )

                            return componentsschemas_chat_content_part_type_0
                        except (TypeError, ValueError, AttributeError, KeyError):
                            pass
                        try:
                            if not isinstance(data, dict):
                                raise TypeError()
                            componentsschemas_chat_content_part_type_1 = (
                                ChatContentPartType1.from_dict(data)
                            )

                            return componentsschemas_chat_content_part_type_1
                        except (TypeError, ValueError, AttributeError, KeyError):
                            pass
                        try:
                            if not isinstance(data, dict):
                                raise TypeError()
                            componentsschemas_chat_content_part_type_2 = (
                                ChatContentPartType2.from_dict(data)
                            )

                            return componentsschemas_chat_content_part_type_2
                        except (TypeError, ValueError, AttributeError, KeyError):
                            pass
                        try:
                            if not isinstance(data, dict):
                                raise TypeError()
                            componentsschemas_chat_content_part_type_3 = (
                                ChatContentPartType3.from_dict(data)
                            )

                            return componentsschemas_chat_content_part_type_3
                        except (TypeError, ValueError, AttributeError, KeyError):
                            pass
                        try:
                            if not isinstance(data, dict):
                                raise TypeError()
                            componentsschemas_chat_content_part_type_4 = (
                                ChatContentPartType4.from_dict(data)
                            )

                            return componentsschemas_chat_content_part_type_4
                        except (TypeError, ValueError, AttributeError, KeyError):
                            pass
                        try:
                            if not isinstance(data, dict):
                                raise TypeError()
                            componentsschemas_chat_content_part_type_5 = (
                                ChatContentPartType5.from_dict(data)
                            )

                            return componentsschemas_chat_content_part_type_5
                        except (TypeError, ValueError, AttributeError, KeyError):
                            pass
                        if not isinstance(data, dict):
                            raise TypeError()
                        componentsschemas_chat_content_part_type_6 = (
                            ChatContentPartType6.from_dict(data)
                        )

                        return componentsschemas_chat_content_part_type_6

                    componentsschemas_chat_message_content_type_1_item = (
                        _parse_componentsschemas_chat_message_content_type_1_item(
                            componentsschemas_chat_message_content_type_1_item_data
                        )
                    )

                    componentsschemas_chat_message_content_type_1.append(
                        componentsschemas_chat_message_content_type_1_item
                    )

                return componentsschemas_chat_message_content_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(
                list[
                    ChatContentPartType0
                    | ChatContentPartType1
                    | ChatContentPartType2
                    | ChatContentPartType3
                    | ChatContentPartType4
                    | ChatContentPartType5
                    | ChatContentPartType6
                ]
                | None
                | str
                | Unset,
                data,
            )

        content = _parse_content(d.pop("content", UNSET))

        def _parse_name(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        name = _parse_name(d.pop("name", UNSET))

        def _parse_tool_call_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        tool_call_id = _parse_tool_call_id(d.pop("tool_call_id", UNSET))

        _tool_calls = d.pop("tool_calls", UNSET)
        tool_calls: list[ChatToolCall] | Unset = UNSET
        if _tool_calls is not UNSET:
            tool_calls = []
            for tool_calls_item_data in _tool_calls:
                tool_calls_item = ChatToolCall.from_dict(tool_calls_item_data)

                tool_calls.append(tool_calls_item)

        chat_message = cls(
            role=role,
            content=content,
            name=name,
            tool_call_id=tool_call_id,
            tool_calls=tool_calls,
        )

        chat_message.additional_properties = d
        return chat_message

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
