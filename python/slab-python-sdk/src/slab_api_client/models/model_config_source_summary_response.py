from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.model_config_source_artifact_response import (
        ModelConfigSourceArtifactResponse,
    )


T = TypeVar("T", bound="ModelConfigSourceSummaryResponse")


@_attrs_define
class ModelConfigSourceSummaryResponse:
    """
    Attributes:
        artifacts (list[ModelConfigSourceArtifactResponse]):
        source_kind (str):
        filename (None | str | Unset):
        local_path (None | str | Unset):
        repo_id (None | str | Unset):
    """

    artifacts: list[ModelConfigSourceArtifactResponse]
    source_kind: str
    filename: None | str | Unset = UNSET
    local_path: None | str | Unset = UNSET
    repo_id: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        artifacts = []
        for artifacts_item_data in self.artifacts:
            artifacts_item = artifacts_item_data.to_dict()
            artifacts.append(artifacts_item)

        source_kind = self.source_kind

        filename: None | str | Unset
        if isinstance(self.filename, Unset):
            filename = UNSET
        else:
            filename = self.filename

        local_path: None | str | Unset
        if isinstance(self.local_path, Unset):
            local_path = UNSET
        else:
            local_path = self.local_path

        repo_id: None | str | Unset
        if isinstance(self.repo_id, Unset):
            repo_id = UNSET
        else:
            repo_id = self.repo_id

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "artifacts": artifacts,
                "source_kind": source_kind,
            }
        )
        if filename is not UNSET:
            field_dict["filename"] = filename
        if local_path is not UNSET:
            field_dict["local_path"] = local_path
        if repo_id is not UNSET:
            field_dict["repo_id"] = repo_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.model_config_source_artifact_response import (
            ModelConfigSourceArtifactResponse,
        )

        d = dict(src_dict)
        artifacts = []
        _artifacts = d.pop("artifacts")
        for artifacts_item_data in _artifacts:
            artifacts_item = ModelConfigSourceArtifactResponse.from_dict(
                artifacts_item_data
            )

            artifacts.append(artifacts_item)

        source_kind = d.pop("source_kind")

        def _parse_filename(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        filename = _parse_filename(d.pop("filename", UNSET))

        def _parse_local_path(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        local_path = _parse_local_path(d.pop("local_path", UNSET))

        def _parse_repo_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        repo_id = _parse_repo_id(d.pop("repo_id", UNSET))

        model_config_source_summary_response = cls(
            artifacts=artifacts,
            source_kind=source_kind,
            filename=filename,
            local_path=local_path,
            repo_id=repo_id,
        )

        model_config_source_summary_response.additional_properties = d
        return model_config_source_summary_response

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
