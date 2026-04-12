import {
  createContext,
  useCallback,
  useEffect,
  useMemo,
  useState,
  type PropsWithChildren,
} from "react";
import {
  DEFAULT_HEADER_META,
  type HeaderMeta,
  type HeaderMetaOverride,
} from "@/layouts/header-meta";
import type { HeaderControl } from "@/layouts/header-controls";

type HeaderMetaEntry = {
  id: string;
  meta: HeaderMetaOverride;
};

type HeaderControlEntry = {
  id: string;
  control: HeaderControl;
};

export type GlobalHeaderContextValue = {
  meta: HeaderMeta;
  control: HeaderControl | null;
  setMeta: (id: string, meta: HeaderMetaOverride) => void;
  clearMeta: (id: string) => void;
  setControl: (id: string, control: HeaderControl) => void;
  clearControl: (id: string) => void;
};

export const GlobalHeaderContext = createContext<GlobalHeaderContextValue | null>(null);

function mergeHeaderMeta(base: HeaderMeta, override: HeaderMetaOverride): HeaderMeta {
  return {
    title: override.title ?? base.title,
    subtitle: override.subtitle ?? base.subtitle,
    icon: override.icon ?? base.icon,
  };
}

type GlobalHeaderProviderProps = PropsWithChildren<{
  defaultMeta?: HeaderMeta;
}>;

export function GlobalHeaderProvider({
  children,
  defaultMeta = DEFAULT_HEADER_META,
}: GlobalHeaderProviderProps) {
  const [entries, setEntries] = useState<HeaderMetaEntry[]>([]);
  const [controlEntries, setControlEntries] = useState<HeaderControlEntry[]>([]);

  const setMeta = useCallback((id: string, meta: HeaderMetaOverride) => {
    setEntries((current) => {
      const index = current.findIndex((entry) => entry.id === id);

      if (index === -1) {
        return [...current, { id, meta }];
      }

      return current.map((entry, entryIndex) =>
        entryIndex === index ? { ...entry, meta } : entry,
      );
    });
  }, []);

  const clearMeta = useCallback((id: string) => {
    setEntries((current) => current.filter((entry) => entry.id !== id));
  }, []);

  const setControl = useCallback((id: string, control: HeaderControl) => {
    setControlEntries((current) => {
      const index = current.findIndex((entry) => entry.id === id);

      if (index === -1) {
        return [...current, { id, control }];
      }

      return current.map((entry, entryIndex) =>
        entryIndex === index ? { ...entry, control } : entry,
      );
    });
  }, []);

  const clearControl = useCallback((id: string) => {
    setControlEntries((current) => current.filter((entry) => entry.id !== id));
  }, []);

  const meta = useMemo(
    () => entries.reduce((current, entry) => mergeHeaderMeta(current, entry.meta), defaultMeta),
    [defaultMeta, entries],
  );
  const control = useMemo(() => {
    if (controlEntries.length === 0) {
      return null;
    }

    return controlEntries[controlEntries.length - 1]?.control ?? null;
  }, [controlEntries]);

  useEffect(() => {
    document.title = `${meta.title} | Slab`;
  }, [meta.title]);

  const value = useMemo(
    () => ({
      meta,
      control,
      setMeta,
      clearMeta,
      setControl,
      clearControl,
    }),
    [clearControl, clearMeta, control, meta, setControl, setMeta],
  );

  return <GlobalHeaderContext.Provider value={value}>{children}</GlobalHeaderContext.Provider>;
}
