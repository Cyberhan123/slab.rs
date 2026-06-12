from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.i18n_payload import I18NPayload


T = TypeVar("T", bound="TaskProgressResponse")


@_attrs_define
class TaskProgressResponse:
    """
    Attributes:
        current (int):
        i18n (I18NPayload | None | Unset):
        label (None | str | Unset):
        logs (list[str] | None | Unset):
        message (None | str | Unset):
        step (int | None | Unset):
        step_count (int | None | Unset):
        total (int | None | Unset):
        unit (None | str | Unset):
    """

    current: int
    i18n: I18NPayload | None | Unset = UNSET
    label: None | str | Unset = UNSET
    logs: list[str] | None | Unset = UNSET
    message: None | str | Unset = UNSET
    step: int | None | Unset = UNSET
    step_count: int | None | Unset = UNSET
    total: int | None | Unset = UNSET
    unit: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.i18n_payload import I18NPayload

        current = self.current

        i18n: dict[str, Any] | None | Unset
        if isinstance(self.i18n, Unset):
            i18n = UNSET
        elif isinstance(self.i18n, I18NPayload):
            i18n = self.i18n.to_dict()
        else:
            i18n = self.i18n

        label: None | str | Unset
        if isinstance(self.label, Unset):
            label = UNSET
        else:
            label = self.label

        logs: list[str] | None | Unset
        if isinstance(self.logs, Unset):
            logs = UNSET
        elif isinstance(self.logs, list):
            logs = self.logs

        else:
            logs = self.logs

        message: None | str | Unset
        if isinstance(self.message, Unset):
            message = UNSET
        else:
            message = self.message

        step: int | None | Unset
        if isinstance(self.step, Unset):
            step = UNSET
        else:
            step = self.step

        step_count: int | None | Unset
        if isinstance(self.step_count, Unset):
            step_count = UNSET
        else:
            step_count = self.step_count

        total: int | None | Unset
        if isinstance(self.total, Unset):
            total = UNSET
        else:
            total = self.total

        unit: None | str | Unset
        if isinstance(self.unit, Unset):
            unit = UNSET
        else:
            unit = self.unit

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "current": current,
            }
        )
        if i18n is not UNSET:
            field_dict["i18n"] = i18n
        if label is not UNSET:
            field_dict["label"] = label
        if logs is not UNSET:
            field_dict["logs"] = logs
        if message is not UNSET:
            field_dict["message"] = message
        if step is not UNSET:
            field_dict["step"] = step
        if step_count is not UNSET:
            field_dict["step_count"] = step_count
        if total is not UNSET:
            field_dict["total"] = total
        if unit is not UNSET:
            field_dict["unit"] = unit

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.i18n_payload import I18NPayload

        d = dict(src_dict)
        current = d.pop("current")

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

        def _parse_label(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        label = _parse_label(d.pop("label", UNSET))

        def _parse_logs(data: object) -> list[str] | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, list):
                    raise TypeError()
                logs_type_0 = cast(list[str], data)

                return logs_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(list[str] | None | Unset, data)

        logs = _parse_logs(d.pop("logs", UNSET))

        def _parse_message(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        message = _parse_message(d.pop("message", UNSET))

        def _parse_step(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        step = _parse_step(d.pop("step", UNSET))

        def _parse_step_count(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        step_count = _parse_step_count(d.pop("step_count", UNSET))

        def _parse_total(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        total = _parse_total(d.pop("total", UNSET))

        def _parse_unit(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        unit = _parse_unit(d.pop("unit", UNSET))

        task_progress_response = cls(
            current=current,
            i18n=i18n,
            label=label,
            logs=logs,
            message=message,
            step=step,
            step_count=step_count,
            total=total,
            unit=unit,
        )

        task_progress_response.additional_properties = d
        return task_progress_response

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
