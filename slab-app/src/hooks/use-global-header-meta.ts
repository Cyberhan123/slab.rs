import { useContext, useId, useLayoutEffect } from "react";
import { GlobalHeaderContext } from "@/layouts/global-header-provider";
import type { HeaderMeta, HeaderMetaOverride } from "@/layouts/header-meta";
import type { GlobalHeaderContextValue, HeaderModelPicker } from "@/layouts/global-header-provider";

export function useGlobalHeaderMeta(): HeaderMeta {
  const context = useContext(GlobalHeaderContext);

  if (!context) {
    throw new Error("useGlobalHeaderMeta must be used within GlobalHeaderProvider");
  }

  return context.meta;
}

export function useGlobalHeaderState(): Pick<GlobalHeaderContextValue, "meta" | "modelPicker"> {
  const context = useContext(GlobalHeaderContext);

  if (!context) {
    throw new Error("useGlobalHeaderState must be used within GlobalHeaderProvider");
  }

  return {
    meta: context.meta,
    modelPicker: context.modelPicker,
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

export function usePageHeaderModelPicker(
  modelPicker: HeaderModelPicker | null | undefined,
): void {
  const context = useContext(GlobalHeaderContext);
  const id = useId();

  if (!context) {
    throw new Error("usePageHeaderModelPicker must be used within GlobalHeaderProvider");
  }

  const { setModelPicker, clearModelPicker } = context;
  const isActive = modelPicker != null;
  const value = modelPicker?.value;
  const options = modelPicker?.options;
  const onValueChange = modelPicker?.onValueChange;
  const placeholder = modelPicker?.placeholder;
  const loading = modelPicker?.loading;
  const disabled = modelPicker?.disabled;
  const emptyLabel = modelPicker?.emptyLabel;

  useLayoutEffect(() => {
    if (!isActive || !modelPicker) {
      return undefined;
    }

    setModelPicker(id, modelPicker);

    return () => {
      clearModelPicker(id);
    };
  }, [
    clearModelPicker,
    disabled,
    emptyLabel,
    id,
    isActive,
    loading,
    modelPicker,
    onValueChange,
    options,
    placeholder,
    setModelPicker,
    value,
  ]);
}
