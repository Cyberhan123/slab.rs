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

export type HeaderModelPickerOption = {
  id: string;
  label: string;
  disabled?: boolean;
};

export type HeaderModelPicker = {
  value: string;
  options: HeaderModelPickerOption[];
  onValueChange: (value: string) => void;
  groupLabel?: string;
  placeholder?: string;
  loading?: boolean;
  disabled?: boolean;
  emptyLabel?: string;
};

type HeaderModelPickerEntry = {
  id: string;
  picker: HeaderModelPicker;
};

export type GlobalHeaderContextValue = {
  meta: HeaderMeta;
  modelPicker: HeaderModelPicker | null;
  setMeta: (id: string, meta: HeaderMetaOverride) => void;
  clearMeta: (id: string) => void;
  setModelPicker: (id: string, picker: HeaderModelPicker) => void;
  clearModelPicker: (id: string) => void;
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
  const [modelPickerEntries, setModelPickerEntries] = useState<HeaderModelPickerEntry[]>([]);

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

  const setModelPicker = useCallback((id: string, picker: HeaderModelPicker) => {
    setModelPickerEntries((current) => {
      const index = current.findIndex((entry) => entry.id === id);

      if (index === -1) {
        return [...current, { id, picker }];
      }

      return current.map((entry, entryIndex) =>
        entryIndex === index ? { ...entry, picker } : entry,
      );
    });
  }, []);

  const clearModelPicker = useCallback((id: string) => {
    setModelPickerEntries((current) => current.filter((entry) => entry.id !== id));
  }, []);

  const meta = useMemo(
    () => entries.reduce((current, entry) => mergeHeaderMeta(current, entry.meta), defaultMeta),
    [defaultMeta, entries],
  );
  const modelPicker = useMemo(() => {
    if (modelPickerEntries.length === 0) {
      return null;
    }

    return modelPickerEntries[modelPickerEntries.length - 1]?.picker ?? null;
  }, [modelPickerEntries]);

  useEffect(() => {
    document.title = `${meta.title} | Slab`;
  }, [meta.title]);

  const value = useMemo(
    () => ({
      meta,
      modelPicker,
      setMeta,
      clearMeta,
      setModelPicker,
      clearModelPicker,
    }),
    [clearMeta, clearModelPicker, meta, modelPicker, setMeta, setModelPicker],
  );

  return <GlobalHeaderContext.Provider value={value}>{children}</GlobalHeaderContext.Provider>;
}
