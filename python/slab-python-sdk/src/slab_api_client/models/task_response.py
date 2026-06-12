from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.task_status import TaskStatus
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.i18n_payload import I18NPayload
    from ..models.task_progress_response import TaskProgressResponse


T = TypeVar("T", bound="TaskResponse")


@_attrs_define
class TaskResponse:
    """
    Attributes:
        created_at (str):
        id (str):
        status (TaskStatus):
        task_type (str):
        updated_at (str):
        error_msg (None | str | Unset):
        i18n (I18NPayload | None | Unset):
        progress (None | TaskProgressResponse | Unset):
    """

    created_at: str
    id: str
    status: TaskStatus
    task_type: str
    updated_at: str
    error_msg: None | str | Unset = UNSET
    i18n: I18NPayload | None | Unset = UNSET
    progress: None | TaskProgressResponse | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.i18n_payload import I18NPayload
        from ..models.task_progress_response import TaskProgressResponse

        created_at = self.created_at

        id = self.id

        status = self.status.value

        task_type = self.task_type

        updated_at = self.updated_at

        error_msg: None | str | Unset
        if isinstance(self.error_msg, Unset):
            error_msg = UNSET
        else:
            error_msg = self.error_msg

        i18n: dict[str, Any] | None | Unset
        if isinstance(self.i18n, Unset):
            i18n = UNSET
        elif isinstance(self.i18n, I18NPayload):
            i18n = self.i18n.to_dict()
        else:
            i18n = self.i18n

        progress: dict[str, Any] | None | Unset
        if isinstance(self.progress, Unset):
            progress = UNSET
        elif isinstance(self.progress, TaskProgressResponse):
            progress = self.progress.to_dict()
        else:
            progress = self.progress

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "created_at": created_at,
                "id": id,
                "status": status,
                "task_type": task_type,
                "updated_at": updated_at,
            }
        )
        if error_msg is not UNSET:
            field_dict["error_msg"] = error_msg
        if i18n is not UNSET:
            field_dict["i18n"] = i18n
        if progress is not UNSET:
            field_dict["progress"] = progress

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.i18n_payload import I18NPayload
        from ..models.task_progress_response import TaskProgressResponse

        d = dict(src_dict)
        created_at = d.pop("created_at")

        id = d.pop("id")

        status = TaskStatus(d.pop("status"))

        task_type = d.pop("task_type")

        updated_at = d.pop("updated_at")

        def _parse_error_msg(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        error_msg = _parse_error_msg(d.pop("error_msg", UNSET))

        def _parse_i18n(data: object) -> I18NPayload | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                i18n_type_1 = I18NPayload.from_dict(data)

                return i18n_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(I18NPayload | None | Unset, data)

        i18n = _parse_i18n(d.pop("i18n", UNSET))

        def _parse_progress(data: object) -> None | TaskProgressResponse | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                progress_type_1 = TaskProgressResponse.from_dict(data)

                return progress_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | TaskProgressResponse | Unset, data)

        progress = _parse_progress(d.pop("progress", UNSET))

        task_response = cls(
            created_at=created_at,
            id=id,
            status=status,
            task_type=task_type,
            updated_at=updated_at,
            error_msg=error_msg,
            i18n=i18n,
            progress=progress,
        )

        task_response.additional_properties = d
        return task_response

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
