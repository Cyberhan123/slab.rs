import {
  createContext,
  useCallback,
  useEffect,
  useMemo,
  useState,
  type PropsWithChildren,
  type ReactNode,
} from 'react';

import {
  DEFAULT_HEADER_META,
  type HeaderHistoryConfig,
  type HeaderMeta,
  type HeaderSearchConfig,
  type HeaderSelectConfig,
} from '@slab/ui/layouts/header';

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

type HeaderHistoryEntry = {
  id: string;
  history: HeaderHistoryConfig;
};

type HeaderSlotEntry = {
  id: string;
  node: ReactNode;
};

export type HeaderContextValue = {
  meta: HeaderMeta;
  history: HeaderHistoryConfig | null;
  select: HeaderSelectConfig | null;
  search: HeaderSearchConfig | null;
  left: ReactNode | null;
  right: ReactNode | null;
  setMeta: (id: string, meta: HeaderMeta) => void;
  clearMeta: (id: string) => void;
  setHistory: (id: string, history: HeaderHistoryConfig) => void;
  clearHistory: (id: string) => void;
  setSelect: (id: string, select: HeaderSelectConfig) => void;
  clearSelect: (id: string) => void;
  setSearch: (id: string, search: HeaderSearchConfig) => void;
  clearSearch: (id: string) => void;
  setLeft: (id: string, node: ReactNode) => void;
  clearLeft: (id: string) => void;
  setRight: (id: string, node: ReactNode) => void;
  clearRight: (id: string) => void;
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

function areHeaderHistoryConfigsEqual(left: HeaderHistoryConfig, right: HeaderHistoryConfig) {
  return (
    left.onClick === right.onClick &&
    left.ariaLabel === right.ariaLabel &&
    left.title === right.title &&
    Boolean(left.disabled) === Boolean(right.disabled)
  );
}

export function HeaderProvider({
  children,
  defaultMeta = DEFAULT_HEADER_META,
}: HeaderProviderProps) {
  const [metaEntries, setMetaEntries] = useState<HeaderMetaEntry[]>([]);
  const [historyEntries, setHistoryEntries] = useState<HeaderHistoryEntry[]>([]);
  const [selectEntries, setSelectEntries] = useState<HeaderSelectEntry[]>([]);
  const [searchEntries, setSearchEntries] = useState<HeaderSearchEntry[]>([]);
  const [leftEntries, setLeftEntries] = useState<HeaderSlotEntry[]>([]);
  const [rightEntries, setRightEntries] = useState<HeaderSlotEntry[]>([]);

  const setMeta = useCallback((id: string, meta: HeaderMeta) => {
    setMetaEntries((current) =>
      upsertEntry(current, id, { id, meta }, (left, right) => areHeaderMetaEqual(left.meta, right.meta)),
    );
  }, []);

  const clearMeta = useCallback((id: string) => {
    setMetaEntries((current) => removeEntry(current, id));
  }, []);

  const setHistory = useCallback((id: string, history: HeaderHistoryConfig) => {
    setHistoryEntries((current) =>
      upsertEntry(current, id, { id, history }, (left, right) =>
        areHeaderHistoryConfigsEqual(left.history, right.history),
      ),
    );
  }, []);

  const clearHistory = useCallback((id: string) => {
    setHistoryEntries((current) => removeEntry(current, id));
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

  // Render-prop slots: ReactNode can't be deep-compared, so equality is
  // referential. Callers must keep the node referentially stable (e.g. via
  // `useMemo`) to avoid re-render churn — same contract as a `children` prop.
  const setLeft = useCallback((id: string, node: ReactNode) => {
    setLeftEntries((current) =>
      upsertEntry(current, id, { id, node }, (left, right) => left.node === right.node),
    );
  }, []);

  const clearLeft = useCallback((id: string) => {
    setLeftEntries((current) => removeEntry(current, id));
  }, []);

  const setRight = useCallback((id: string, node: ReactNode) => {
    setRightEntries((current) =>
      upsertEntry(current, id, { id, node }, (left, right) => left.node === right.node),
    );
  }, []);

  const clearRight = useCallback((id: string) => {
    setRightEntries((current) => removeEntry(current, id));
  }, []);

  const meta = metaEntries.at(-1)?.meta ?? defaultMeta;
  const history = historyEntries.at(-1)?.history ?? null;
  const select = selectEntries.at(-1)?.select ?? null;
  const search = searchEntries.at(-1)?.search ?? null;
  const left = leftEntries.at(-1)?.node ?? null;
  const right = rightEntries.at(-1)?.node ?? null;

  useEffect(() => {
    document.title = `${meta.title} | Slab`;
  }, [meta.title]);

  const value = useMemo(
    () => ({
      meta,
      history,
      select,
      search,
      left,
      right,
      setMeta,
      clearMeta,
      setHistory,
      clearHistory,
      setSelect,
      clearSelect,
      setSearch,
      clearSearch,
      setLeft,
      clearLeft,
      setRight,
      clearRight,
    }),
    [
      clearHistory,
      clearLeft,
      clearMeta,
      clearRight,
      clearSearch,
      clearSelect,
      history,
      left,
      meta,
      right,
      search,
      select,
      setHistory,
      setLeft,
      setMeta,
      setRight,
      setSearch,
      setSelect,
    ],
  );

  return <HeaderContext.Provider value={value}>{children}</HeaderContext.Provider>;
}
