import { useContext, useId, useLayoutEffect } from "react";

import {
  GlobalHeaderContext,
  type GlobalHeaderContextValue,
} from "@/layouts/global-header-provider";
import type { HeaderControl, HeaderSearchControl } from "@/layouts/header-controls";

export function useHeader(): Pick<GlobalHeaderContextValue, "meta" | "control" | "search"> {
  const context = useContext(GlobalHeaderContext);

  if (!context) {
    throw new Error("useHeader must be used within GlobalHeaderProvider");
  }

  return {
    meta: context.meta,
    control: context.control,
    search: context.search,
  };
}

export function useHeaderControl(control: HeaderControl | null | undefined): void {
  const context = useContext(GlobalHeaderContext);
  const id = useId();

  if (!context) {
    throw new Error("useHeaderControl must be used within GlobalHeaderProvider");
  }

  const { setControl, clearControl } = context;
  const type = control?.type;
  const value = control?.type === "select" ? control.value : undefined;
  const options = control?.type === "select" ? control.options : undefined;
  const onValueChange = control?.type === "select" ? control.onValueChange : undefined;
  const groupLabel = control?.type === "select" ? control.groupLabel : undefined;
  const placeholder = control?.type === "select" ? control.placeholder : undefined;
  const loading = control?.type === "select" ? control.loading : undefined;
  const disabled = control?.type === "select" ? control.disabled : undefined;
  const emptyLabel = control?.type === "select" ? control.emptyLabel : undefined;

  useLayoutEffect(() => {
    if (!control) {
      return undefined;
    }

    setControl(id, control);

    return () => {
      clearControl(id);
    };
  }, [
    clearControl,
    control,
    disabled,
    emptyLabel,
    groupLabel,
    id,
    loading,
    onValueChange,
    options,
    placeholder,
    setControl,
    type,
    value,
  ]);
}

export function useHeaderSearch(search: HeaderSearchControl | null | undefined): void {
  const context = useContext(GlobalHeaderContext);
  const id = useId();

  if (!context) {
    throw new Error("useHeaderSearch must be used within GlobalHeaderProvider");
  }

  const { setSearch, clearSearch } = context;
  const isActive = search != null;
  const value = search?.type === "search" ? search.value : "";
  const onValueChange = search?.type === "search" ? search.onValueChange : undefined;
  const placeholder = search?.type === "search" ? search.placeholder : undefined;
  const ariaLabel = search?.type === "search" ? search.ariaLabel : undefined;
  const disabled = search?.type === "search" ? search.disabled : undefined;

  useLayoutEffect(() => {
    if (!isActive || !onValueChange) {
      return undefined;
    }

    setSearch(id, {
      type: "search",
      value,
      onValueChange,
      placeholder,
      ariaLabel,
      disabled,
    });

    return () => {
      clearSearch(id);
    };
  }, [
    ariaLabel,
    clearSearch,
    disabled,
    id,
    isActive,
    onValueChange,
    placeholder,
    setSearch,
    value,
  ]);
}
