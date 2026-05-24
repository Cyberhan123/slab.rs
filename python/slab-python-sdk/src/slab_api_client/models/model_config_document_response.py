from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.model_config_section_response import ModelConfigSectionResponse
    from ..models.model_config_selection_response import ModelConfigSelectionResponse
    from ..models.model_config_source_summary_response import (
        ModelConfigSourceSummaryResponse,
    )
    from ..models.unified_model_response import UnifiedModelResponse


T = TypeVar("T", bound="ModelConfigDocumentResponse")


@_attrs_define
class ModelConfigDocumentResponse:
    """
    Attributes:
        model_summary (UnifiedModelResponse): Unified model response returned by `/v1/models`.
        resolved_inference_spec (Any):
        resolved_load_spec (Any):
        sections (list[ModelConfigSectionResponse]):
        selection (ModelConfigSelectionResponse):
        source_summary (ModelConfigSourceSummaryResponse):
        warnings (list[str]):
    """

    model_summary: UnifiedModelResponse
    resolved_inference_spec: Any
    resolved_load_spec: Any
    sections: list[ModelConfigSectionResponse]
    selection: ModelConfigSelectionResponse
    source_summary: ModelConfigSourceSummaryResponse
    warnings: list[str]
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        model_summary = self.model_summary.to_dict()

        resolved_inference_spec = self.resolved_inference_spec

        resolved_load_spec = self.resolved_load_spec

        sections = []
        for sections_item_data in self.sections:
            sections_item = sections_item_data.to_dict()
            sections.append(sections_item)

        selection = self.selection.to_dict()

        source_summary = self.source_summary.to_dict()

        warnings = self.warnings

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "model_summary": model_summary,
                "resolved_inference_spec": resolved_inference_spec,
                "resolved_load_spec": resolved_load_spec,
                "sections": sections,
                "selection": selection,
                "source_summary": source_summary,
                "warnings": warnings,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.model_config_section_response import ModelConfigSectionResponse
        from ..models.model_config_selection_response import (
            ModelConfigSelectionResponse,
        )
        from ..models.model_config_source_summary_response import (
            ModelConfigSourceSummaryResponse,
        )
        from ..models.unified_model_response import UnifiedModelResponse

        d = dict(src_dict)
        model_summary = UnifiedModelResponse.from_dict(d.pop("model_summary"))

        resolved_inference_spec = d.pop("resolved_inference_spec")

        resolved_load_spec = d.pop("resolved_load_spec")

        sections = []
        _sections = d.pop("sections")
        for sections_item_data in _sections:
            sections_item = ModelConfigSectionResponse.from_dict(sections_item_data)

            sections.append(sections_item)

        selection = ModelConfigSelectionResponse.from_dict(d.pop("selection"))

        source_summary = ModelConfigSourceSummaryResponse.from_dict(
            d.pop("source_summary")
        )

        warnings = cast(list[str], d.pop("warnings"))

        model_config_document_response = cls(
            model_summary=model_summary,
            resolved_inference_spec=resolved_inference_spec,
            resolved_load_spec=resolved_load_spec,
            sections=sections,
            selection=selection,
            source_summary=source_summary,
            warnings=warnings,
        )

        model_config_document_response.additional_properties = d
        return model_config_document_response

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
