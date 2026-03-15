import { useContext, useId, useLayoutEffect } from "react";
import { GlobalHeaderContext } from "@/layouts/global-header-provider";
import type { HeaderMeta, HeaderMetaOverride } from "@/layouts/header-meta";

export function useGlobalHeaderMeta(): HeaderMeta {
  const context = useContext(GlobalHeaderContext);

  if (!context) {
    throw new Error("useGlobalHeaderMeta must be used within GlobalHeaderProvider");
  }

  return context.meta;
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
