import { useContext, useId, useLayoutEffect } from "react";
import { GlobalHeaderContext } from "@/layouts/global-header-provider";
import type { HeaderMeta, HeaderMetaOverride } from "@/layouts/header-meta";
import type { GlobalHeaderContextValue } from "@/layouts/global-header-provider";
import type { HeaderControl } from "@/layouts/header-controls";

export function useGlobalHeaderMeta(): HeaderMeta {
  const context = useContext(GlobalHeaderContext);

  if (!context) {
    throw new Error("useGlobalHeaderMeta must be used within GlobalHeaderProvider");
  }

  return context.meta;
}

export function useGlobalHeaderState(): Pick<GlobalHeaderContextValue, "meta" | "control"> {
  const context = useContext(GlobalHeaderContext);

  if (!context) {
    throw new Error("useGlobalHeaderState must be used within GlobalHeaderProvider");
  }

  return {
    meta: context.meta,
    control: context.control,
  };
}

export function usePageHeader(meta: HeaderMetaOverride | null | undefined): void {
  const context = useContext(GlobalHeaderContext);
  const id = useId();

  if (!context) {
    throw new Error("usePageHeader must be used within GlobalHeaderProvider");
  }

  const { setMeta, clearMeta } = context;
  const isActive = meta != null;
  const title = meta?.title;
  const subtitle = meta?.subtitle;
  const icon = meta?.icon;

  useLayoutEffect(() => {
    if (!isActive) {
      return undefined;
    }

    setMeta(id, { title, subtitle, icon });

    return () => {
      clearMeta(id);
    };
  }, [clearMeta, icon, id, isActive, setMeta, subtitle, title]);
}

export function usePageHeaderControl(
  control: HeaderControl | null | undefined,
): void {
  const context = useContext(GlobalHeaderContext);
  const id = useId();

  if (!context) {
    throw new Error("usePageHeaderControl must be used within GlobalHeaderProvider");
  }

  const { setControl, clearControl } = context;
  const isActive = control != null;
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
    if (!isActive || !control) {
      return undefined;
    }

    setControl(id, control);

    return () => {
      clearControl(id);
    };
  }, [
    clearControl,
    disabled,
    emptyLabel,
    groupLabel,
    id,
    isActive,
    loading,
    onValueChange,
    options,
    placeholder,
    setControl,
    type,
    value,
  ]);
}
