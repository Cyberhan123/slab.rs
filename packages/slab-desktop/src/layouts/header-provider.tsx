import {
  createContext,
  useCallback,
  useEffect,
  useMemo,
  useState,
  type PropsWithChildren,
} from 'react';

import {
  DEFAULT_HEADER_META,
  type HeaderControl,
  type HeaderMeta,
  type HeaderSearchControl,
} from '@/layouts/header';

type HeaderControlEntry = {
  id: string;
  control: HeaderControl;
};

type HeaderSearchEntry = {
  id: string;
  search: HeaderSearchControl;
};

export type HeaderContextValue = {
  meta: HeaderMeta;
  control: HeaderControl | null;
  search: HeaderSearchControl | null;
  setControl: (id: string, control: HeaderControl) => void;
  clearControl: (id: string) => void;
  setSearch: (id: string, search: HeaderSearchControl) => void;
  clearSearch: (id: string) => void;
};

export const HeaderContext = createContext<HeaderContextValue | null>(null);

type HeaderProviderProps = PropsWithChildren<{
  defaultMeta?: HeaderMeta;
}>;

function upsertEntry<TEntry extends { id: string }>(
  entries: TEntry[],
  id: string,
  nextEntry: TEntry,
) {
  const index = entries.findIndex((entry) => entry.id === id);

  if (index === -1) {
    return [...entries, nextEntry];
  }

  return entries.map((entry, entryIndex) => (entryIndex === index ? nextEntry : entry));
}

export function HeaderProvider({
  children,
  defaultMeta = DEFAULT_HEADER_META,
}: HeaderProviderProps) {
  const [controlEntries, setControlEntries] = useState<HeaderControlEntry[]>([]);
  const [searchEntries, setSearchEntries] = useState<HeaderSearchEntry[]>([]);

  const setControl = useCallback((id: string, control: HeaderControl) => {
    setControlEntries((current) => upsertEntry(current, id, { id, control }));
  }, []);

  const clearControl = useCallback((id: string) => {
    setControlEntries((current) => current.filter((entry) => entry.id !== id));
  }, []);

  const setSearch = useCallback((id: string, search: HeaderSearchControl) => {
    setSearchEntries((current) => upsertEntry(current, id, { id, search }));
  }, []);

  const clearSearch = useCallback((id: string) => {
    setSearchEntries((current) => current.filter((entry) => entry.id !== id));
  }, []);

  const control = controlEntries.at(-1)?.control ?? null;
  const search = searchEntries.at(-1)?.search ?? null;

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

  return <HeaderContext.Provider value={value}>{children}</HeaderContext.Provider>;
}
