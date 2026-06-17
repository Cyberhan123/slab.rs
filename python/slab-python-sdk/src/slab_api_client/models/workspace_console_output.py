from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="WorkspaceConsoleOutput")


@_attrs_define
class WorkspaceConsoleOutput:
    """
    Attributes:
        command (str):
        stderr (str):
        stdout (str):
        timed_out (bool):
        exit_code (int | None | Unset):
    """

    command: str
    stderr: str
    stdout: str
    timed_out: bool
    exit_code: int | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        command = self.command

        stderr = self.stderr

        stdout = self.stdout

        timed_out = self.timed_out

        exit_code: int | None | Unset
        if isinstance(self.exit_code, Unset):
            exit_code = UNSET
        else:
            exit_code = self.exit_code

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "command": command,
                "stderr": stderr,
                "stdout": stdout,
                "timedOut": timed_out,
            }
        )
        if exit_code is not UNSET:
            field_dict["exitCode"] = exit_code

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        command = d.pop("command")

        stderr = d.pop("stderr")

        stdout = d.pop("stdout")

        timed_out = d.pop("timedOut")

        def _parse_exit_code(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        exit_code = _parse_exit_code(d.pop("exitCode", UNSET))

        workspace_console_output = cls(
            command=command,
            stderr=stderr,
            stdout=stdout,
            timed_out=timed_out,
            exit_code=exit_code,
        )

        workspace_console_output.additional_properties = d
        return workspace_console_output

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
