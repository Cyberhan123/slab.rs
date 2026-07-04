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
  type HeaderMeta,
  type HeaderSearchConfig,
  type HeaderSelectConfig,
} from '@/layouts/header';

type HeaderMetaEntry = {
  id: string;
  meta: HeaderMeta;
};

type HeaderSelectEntry = {
  id: string;
  select: HeaderSelectConfig;
};

type HeaderSearchEntry = {
  id: string;
  search: HeaderSearchConfig;
};

export type HeaderContextValue = {
  meta: HeaderMeta;
  select: HeaderSelectConfig | null;
  search: HeaderSearchConfig | null;
  setMeta: (id: string, meta: HeaderMeta) => void;
  clearMeta: (id: string) => void;
  setSelect: (id: string, select: HeaderSelectConfig) => void;
  clearSelect: (id: string) => void;
  setSearch: (id: string, search: HeaderSearchConfig) => void;
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
  isSameEntry: (current: TEntry, next: TEntry) => boolean,
) {
  const index = entries.findIndex((entry) => entry.id === id);

  if (index === -1) {
    return [...entries, nextEntry];
  }

  if (isSameEntry(entries[index], nextEntry)) {
    return entries;
  }

  return entries.map((entry, entryIndex) => (entryIndex === index ? nextEntry : entry));
}

function removeEntry<TEntry extends { id: string }>(entries: TEntry[], id: string) {
  const nextEntries = entries.filter((entry) => entry.id !== id);
  return nextEntries.length === entries.length ? entries : nextEntries;
}

function areHeaderMetaEqual(left: HeaderMeta, right: HeaderMeta) {
  return (
    left.title === right.title &&
    left.subtitle === right.subtitle &&
    left.icon === right.icon &&
    (left.contextLabel ?? null) === (right.contextLabel ?? null)
  );
}

function areHeaderSelectOptionsEqual(
  left: HeaderSelectConfig['options'],
  right: HeaderSelectConfig['options'],
): boolean {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((leftOption, index) => {
    const rightOption = right[index];
    if (!rightOption) {
      return false;
    }

    const leftChildren = leftOption.children;
    const rightChildren = rightOption.children;
    const childrenEqual =
      !leftChildren && !rightChildren
        ? true
        : Boolean(
            leftChildren &&
              rightChildren &&
              leftChildren.groupLabel === rightChildren.groupLabel &&
              areHeaderSelectOptionsEqual(leftChildren.options, rightChildren.options),
          );

    return (
      leftOption.id === rightOption.id &&
      leftOption.label === rightOption.label &&
      Boolean(leftOption.disabled) === Boolean(rightOption.disabled) &&
      childrenEqual
    );
  });
}

function areHeaderSelectConfigsEqual(left: HeaderSelectConfig, right: HeaderSelectConfig) {
  return (
    left.value === right.value &&
    left.placeholder === right.placeholder &&
    left.onChange === right.onChange &&
    left.groupLabel === right.groupLabel &&
    Boolean(left.loading) === Boolean(right.loading) &&
    Boolean(left.disabled) === Boolean(right.disabled) &&
    left.emptyLabel === right.emptyLabel &&
    areHeaderSelectOptionsEqual(left.options, right.options)
  );
}

function areHeaderSearchConfigsEqual(left: HeaderSearchConfig, right: HeaderSearchConfig) {
  return (
    left.value === right.value &&
    left.placeholder === right.placeholder &&
    left.onChange === right.onChange &&
    left.ariaLabel === right.ariaLabel &&
    Boolean(left.disabled) === Boolean(right.disabled)
  );
}

export function HeaderProvider({
  children,
  defaultMeta = DEFAULT_HEADER_META,
}: HeaderProviderProps) {
  const [metaEntries, setMetaEntries] = useState<HeaderMetaEntry[]>([]);
  const [selectEntries, setSelectEntries] = useState<HeaderSelectEntry[]>([]);
  const [searchEntries, setSearchEntries] = useState<HeaderSearchEntry[]>([]);

  const setMeta = useCallback((id: string, meta: HeaderMeta) => {
    setMetaEntries((current) =>
      upsertEntry(current, id, { id, meta }, (left, right) => areHeaderMetaEqual(left.meta, right.meta)),
    );
  }, []);

  const clearMeta = useCallback((id: string) => {
    setMetaEntries((current) => removeEntry(current, id));
  }, []);

  const setSelect = useCallback((id: string, select: HeaderSelectConfig) => {
    setSelectEntries((current) =>
      upsertEntry(current, id, { id, select }, (left, right) =>
        areHeaderSelectConfigsEqual(left.select, right.select),
      ),
    );
  }, []);

  const clearSelect = useCallback((id: string) => {
    setSelectEntries((current) => removeEntry(current, id));
  }, []);

  const setSearch = useCallback((id: string, search: HeaderSearchConfig) => {
    setSearchEntries((current) =>
      upsertEntry(current, id, { id, search }, (left, right) =>
        areHeaderSearchConfigsEqual(left.search, right.search),
      ),
    );
  }, []);

  const clearSearch = useCallback((id: string) => {
    setSearchEntries((current) => removeEntry(current, id));
  }, []);

  const meta = metaEntries.at(-1)?.meta ?? defaultMeta;
  const select = selectEntries.at(-1)?.select ?? null;
  const search = searchEntries.at(-1)?.search ?? null;

  useEffect(() => {
    document.title = `${meta.title} | Slab`;
  }, [meta.title]);

  const value = useMemo(
    () => ({
      meta,
      select,
      search,
      setMeta,
      clearMeta,
      setSelect,
      clearSelect,
      setSearch,
      clearSearch,
    }),
    [clearMeta, clearSearch, clearSelect, meta, search, select, setMeta, setSearch, setSelect],
  );

  return <HeaderContext.Provider value={value}>{children}</HeaderContext.Provider>;
}
