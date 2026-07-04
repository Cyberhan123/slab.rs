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
  type HeaderControl,
  type HeaderMeta,
  type HeaderSearchControl,
} from "@/layouts/header-controls";

type HeaderControlEntry = {
  id: string;
  control: HeaderControl;
};

type HeaderSearchEntry = {
  id: string;
  search: HeaderSearchControl;
};

export type GlobalHeaderContextValue = {
  meta: HeaderMeta;
  control: HeaderControl | null;
  search: HeaderSearchControl | null;
  setControl: (id: string, control: HeaderControl) => void;
  clearControl: (id: string) => void;
  setSearch: (id: string, search: HeaderSearchControl) => void;
  clearSearch: (id: string) => void;
};

export const GlobalHeaderContext = createContext<GlobalHeaderContextValue | null>(null);

type GlobalHeaderProviderProps = PropsWithChildren<{
  defaultMeta?: HeaderMeta;
}>;

export function GlobalHeaderProvider({
  children,
  defaultMeta = DEFAULT_HEADER_META,
}: GlobalHeaderProviderProps) {
  const [controlEntries, setControlEntries] = useState<HeaderControlEntry[]>([]);
  const [searchEntries, setSearchEntries] = useState<HeaderSearchEntry[]>([]);

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

  const setSearch = useCallback((id: string, search: HeaderSearchControl) => {
    setSearchEntries((current) => {
      const index = current.findIndex((entry) => entry.id === id);

      if (index === -1) {
        return [...current, { id, search }];
      }

      return current.map((entry, entryIndex) =>
        entryIndex === index ? { ...entry, search } : entry,
      );
    });
  }, []);

  const clearSearch = useCallback((id: string) => {
    setSearchEntries((current) => current.filter((entry) => entry.id !== id));
  }, []);

  const control = useMemo(() => {
    if (controlEntries.length === 0) {
      return null;
    }

    return controlEntries[controlEntries.length - 1]?.control ?? null;
  }, [controlEntries]);
  const search = useMemo(() => {
    if (searchEntries.length === 0) {
      return null;
    }

    return searchEntries[searchEntries.length - 1]?.search ?? null;
  }, [searchEntries]);

  useEffect(() => {
    document.title = `${defaultMeta.title} | Slab`;
  }, [defaultMeta.title]);

  const value = useMemo(
    () => ({
      meta: defaultMeta,
      control,
      search,
      setControl,
      clearControl,
      setSearch,
      clearSearch,
    }),
    [clearControl, clearSearch, control, defaultMeta, search, setControl, setSearch],
  );

  return <GlobalHeaderContext.Provider value={value}>{children}</GlobalHeaderContext.Provider>;
}
