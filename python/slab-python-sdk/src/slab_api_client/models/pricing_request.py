from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define

T = TypeVar("T", bound="PricingRequest")


@_attrs_define
class PricingRequest:
    """Pricing info for cost tracking.

    Attributes:
        input_ (float): Cost per 1K input tokens in USD.
        output (float): Cost per 1K output tokens in USD.
    """

    input_: float
    output: float

    def to_dict(self) -> dict[str, Any]:
        input_ = self.input_

        output = self.output

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "input": input_,
                "output": output,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        input_ = d.pop("input")

        output = d.pop("output")

        pricing_request = cls(
            input_=input_,
            output=output,
        )

        return pricing_request
