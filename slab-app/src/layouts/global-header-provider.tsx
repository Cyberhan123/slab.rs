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

type HeaderMetaEntry = {
  id: string;
  meta: HeaderMetaOverride;
};

export type GlobalHeaderContextValue = {
  meta: HeaderMeta;
  setMeta: (id: string, meta: HeaderMetaOverride) => void;
  clearMeta: (id: string) => void;
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

  const meta = useMemo(
    () => entries.reduce((current, entry) => mergeHeaderMeta(current, entry.meta), defaultMeta),
    [defaultMeta, entries],
  );

  useEffect(() => {
    document.title = `${meta.title} | Slab`;
  }, [meta.title]);

  const value = useMemo(
    () => ({
      meta,
      setMeta,
      clearMeta,
    }),
    [clearMeta, meta, setMeta],
  );

  return <GlobalHeaderContext.Provider value={value}>{children}</GlobalHeaderContext.Provider>;
}
