import { useContext, useId, useLayoutEffect } from 'react';

import type {
  HeaderMeta,
  HeaderHistoryConfig,
  HeaderSearchConfig,
  HeaderSelectConfig,
} from '@/layouts/header';
import { HeaderContext, type HeaderContextValue } from '@/layouts/header-provider';

export type UseHeaderRegistration = {
  history?: HeaderHistoryConfig | null;
  meta?: HeaderMeta | null;
  select?: HeaderSelectConfig | null;
  search?: HeaderSearchConfig | null;
};

function useRequiredHeaderContext(hookName: string): HeaderContextValue {
  const context = useContext(HeaderContext);

  if (!context) {
    throw new Error(`${hookName} must be used within HeaderProvider`);
  }

  return context;
}

export function useHeader(): Pick<HeaderContextValue, 'history' | 'meta' | 'select' | 'search'>;
export function useHeader(registration: UseHeaderRegistration | null | undefined): Pick<
  HeaderContextValue,
  'history' | 'meta' | 'select' | 'search'
>;
export function useHeader(registration?: UseHeaderRegistration | null) {
  const context = useRequiredHeaderContext('useHeader');
  const id = useId();
  const history = registration?.history ?? null;
  const meta = registration?.meta ?? null;
  const select = registration?.select ?? null;
  const search = registration?.search ?? null;
  const {
    clearHistory,
    clearMeta,
    clearSearch,
    clearSelect,
    setHistory,
    setMeta,
    setSearch,
    setSelect,
  } = context;

  useLayoutEffect(() => {
    if (registration === undefined) {
      return undefined;
    }

    if (meta) {
      setMeta(id, meta);
    } else {
      clearMeta(id);
    }

    if (history) {
      setHistory(id, history);
    } else {
      clearHistory(id);
    }

    if (select) {
      setSelect(id, select);
    } else {
      clearSelect(id);
    }

    if (search) {
      setSearch(id, search);
    } else {
      clearSearch(id);
    }
    return undefined;
  }, [
    clearHistory,
    clearMeta,
    clearSearch,
    clearSelect,
    history,
    id,
    meta,
    search,
    select,
    setHistory,
    setMeta,
    setSearch,
    setSelect,
  ]);

  useLayoutEffect(
    () => () => {
      clearMeta(id);
      clearHistory(id);
      clearSelect(id);
      clearSearch(id);
    },
    [clearHistory, clearMeta, clearSearch, clearSelect, id],
  );

  return {
    history: context.history,
    meta: context.meta,
    select: context.select,
    search: context.search,
  };
}
