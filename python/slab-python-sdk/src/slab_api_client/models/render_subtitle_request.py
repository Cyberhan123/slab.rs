from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.subtitle_format_request import SubtitleFormatRequest
from ..models.subtitle_variant_request import SubtitleVariantRequest
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.subtitle_entry_request import SubtitleEntryRequest


T = TypeVar("T", bound="RenderSubtitleRequest")


@_attrs_define
class RenderSubtitleRequest:
    """
    Attributes:
        entries (list[SubtitleEntryRequest]):
        format_ (SubtitleFormatRequest):
        source_path (str): Absolute path to the source video/audio file used for default output naming.
        variant (SubtitleVariantRequest):
        output_path (None | str | Unset): Optional absolute output path. Defaults to `<source_stem>.<variant>.srt`.
        overwrite (bool | Unset): Whether an existing output file should be overwritten. Defaults to true.
    """

    entries: list[SubtitleEntryRequest]
    format_: SubtitleFormatRequest
    source_path: str
    variant: SubtitleVariantRequest
    output_path: None | str | Unset = UNSET
    overwrite: bool | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        entries = []
        for entries_item_data in self.entries:
            entries_item = entries_item_data.to_dict()
            entries.append(entries_item)

        format_ = self.format_.value

        source_path = self.source_path

        variant = self.variant.value

        output_path: None | str | Unset
        if isinstance(self.output_path, Unset):
            output_path = UNSET
        else:
            output_path = self.output_path

        overwrite = self.overwrite

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "entries": entries,
                "format": format_,
                "source_path": source_path,
                "variant": variant,
            }
        )
        if output_path is not UNSET:
            field_dict["output_path"] = output_path
        if overwrite is not UNSET:
            field_dict["overwrite"] = overwrite

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.subtitle_entry_request import SubtitleEntryRequest

        d = dict(src_dict)
        entries = []
        _entries = d.pop("entries")
        for entries_item_data in _entries:
            entries_item = SubtitleEntryRequest.from_dict(entries_item_data)

            entries.append(entries_item)

        format_ = SubtitleFormatRequest(d.pop("format"))

        source_path = d.pop("source_path")

        variant = SubtitleVariantRequest(d.pop("variant"))

        def _parse_output_path(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        output_path = _parse_output_path(d.pop("output_path", UNSET))

        overwrite = d.pop("overwrite", UNSET)

        render_subtitle_request = cls(
            entries=entries,
            format_=format_,
            source_path=source_path,
            variant=variant,
            output_path=output_path,
            overwrite=overwrite,
        )

        render_subtitle_request.additional_properties = d
        return render_subtitle_request

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
