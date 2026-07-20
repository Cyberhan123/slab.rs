from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="QuantizeModelRequest")


@_attrs_define
class QuantizeModelRequest:
    """Request body for `POST /v1/models/quantize`.

    Attributes:
        ftype (int): Target quantization format as a raw `llama_ftype` int (e.g. 15 = Q4_K_M, 36 = TQ1_0).
        input_path (str): Path to the source GGUF model file.
        output_path (str): Path to write the quantized GGUF model to.
        allow_requantize (bool | None | Unset): Allow re-quantizing already-quantized tensors.
        dry_run (bool | None | Unset): Do not write a file — only report what would happen.
        keep_split (bool | None | Unset): Keep the model split layout.
        nthread (int | None | Unset): Number of threads (0 = let llama.cpp decide).
        only_copy (bool | None | Unset): Only copy tensors instead of quantizing.
        pure (bool | None | Unset): Disable mix-and-match of quantization types when not specified.
        quantize_output_tensor (bool | None | Unset): Quantize the `output` tensor (default true).
    """

    ftype: int
    input_path: str
    output_path: str
    allow_requantize: bool | None | Unset = UNSET
    dry_run: bool | None | Unset = UNSET
    keep_split: bool | None | Unset = UNSET
    nthread: int | None | Unset = UNSET
    only_copy: bool | None | Unset = UNSET
    pure: bool | None | Unset = UNSET
    quantize_output_tensor: bool | None | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        ftype = self.ftype

        input_path = self.input_path

        output_path = self.output_path

        allow_requantize: bool | None | Unset
        if isinstance(self.allow_requantize, Unset):
            allow_requantize = UNSET
        else:
            allow_requantize = self.allow_requantize

        dry_run: bool | None | Unset
        if isinstance(self.dry_run, Unset):
            dry_run = UNSET
        else:
            dry_run = self.dry_run

        keep_split: bool | None | Unset
        if isinstance(self.keep_split, Unset):
            keep_split = UNSET
        else:
            keep_split = self.keep_split

        nthread: int | None | Unset
        if isinstance(self.nthread, Unset):
            nthread = UNSET
        else:
            nthread = self.nthread

        only_copy: bool | None | Unset
        if isinstance(self.only_copy, Unset):
            only_copy = UNSET
        else:
            only_copy = self.only_copy

        pure: bool | None | Unset
        if isinstance(self.pure, Unset):
            pure = UNSET
        else:
            pure = self.pure

        quantize_output_tensor: bool | None | Unset
        if isinstance(self.quantize_output_tensor, Unset):
            quantize_output_tensor = UNSET
        else:
            quantize_output_tensor = self.quantize_output_tensor

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "ftype": ftype,
                "input_path": input_path,
                "output_path": output_path,
            }
        )
        if allow_requantize is not UNSET:
            field_dict["allow_requantize"] = allow_requantize
        if dry_run is not UNSET:
            field_dict["dry_run"] = dry_run
        if keep_split is not UNSET:
            field_dict["keep_split"] = keep_split
        if nthread is not UNSET:
            field_dict["nthread"] = nthread
        if only_copy is not UNSET:
            field_dict["only_copy"] = only_copy
        if pure is not UNSET:
            field_dict["pure"] = pure
        if quantize_output_tensor is not UNSET:
            field_dict["quantize_output_tensor"] = quantize_output_tensor

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        ftype = d.pop("ftype")

        input_path = d.pop("input_path")

        output_path = d.pop("output_path")

        def _parse_allow_requantize(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        allow_requantize = _parse_allow_requantize(d.pop("allow_requantize", UNSET))

        def _parse_dry_run(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        dry_run = _parse_dry_run(d.pop("dry_run", UNSET))

        def _parse_keep_split(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        keep_split = _parse_keep_split(d.pop("keep_split", UNSET))

        def _parse_nthread(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        nthread = _parse_nthread(d.pop("nthread", UNSET))

        def _parse_only_copy(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        only_copy = _parse_only_copy(d.pop("only_copy", UNSET))

        def _parse_pure(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        pure = _parse_pure(d.pop("pure", UNSET))

        def _parse_quantize_output_tensor(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        quantize_output_tensor = _parse_quantize_output_tensor(
            d.pop("quantize_output_tensor", UNSET)
        )

        quantize_model_request = cls(
            ftype=ftype,
            input_path=input_path,
            output_path=output_path,
            allow_requantize=allow_requantize,
            dry_run=dry_run,
            keep_split=keep_split,
            nthread=nthread,
            only_copy=only_copy,
            pure=pure,
            quantize_output_tensor=quantize_output_tensor,
        )

        return quantize_model_request
