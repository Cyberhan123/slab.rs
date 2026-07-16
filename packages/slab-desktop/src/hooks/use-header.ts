import { useContext, useId, useLayoutEffect, type ReactNode } from 'react';

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
  /** Custom content rendered at the end of the header's left block. */
  left?: ReactNode;
  /** Custom content rendered right of the history control (before window controls). */
  right?: ReactNode;
};

function useRequiredHeaderContext(hookName: string): HeaderContextValue {
  const context = useContext(HeaderContext);

  if (!context) {
    throw new Error(`${hookName} must be used within HeaderProvider`);
  }

  return context;
}

export function useHeader(): Pick<
  HeaderContextValue,
  'history' | 'meta' | 'select' | 'search' | 'left' | 'right'
>;
export function useHeader(registration: UseHeaderRegistration | null | undefined): Pick<
  HeaderContextValue,
  'history' | 'meta' | 'select' | 'search' | 'left' | 'right'
>;
export function useHeader(registration?: UseHeaderRegistration | null) {
  const context = useRequiredHeaderContext('useHeader');
  const id = useId();
  const history = registration?.history ?? null;
  const meta = registration?.meta ?? null;
  const select = registration?.select ?? null;
  const search = registration?.search ?? null;
  const left = registration?.left ?? null;
  const right = registration?.right ?? null;
  const {
    clearHistory,
    clearLeft,
    clearMeta,
    clearRight,
    clearSearch,
    clearSelect,
    setHistory,
    setLeft,
    setMeta,
    setRight,
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

    if (left !== null) {
      setLeft(id, left);
    } else {
      clearLeft(id);
    }

    if (right !== null) {
      setRight(id, right);
    } else {
      clearRight(id);
    }
    return undefined;
  }, [
    clearHistory,
    clearLeft,
    clearMeta,
    clearRight,
    clearSearch,
    clearSelect,
    history,
    id,
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
  ]);

  useLayoutEffect(
    () => () => {
      clearMeta(id);
      clearHistory(id);
      clearSelect(id);
      clearSearch(id);
      clearLeft(id);
      clearRight(id);
    },
    [clearHistory, clearLeft, clearMeta, clearRight, clearSearch, clearSelect, id],
  );

  return {
    history: context.history,
    meta: context.meta,
    select: context.select,
    search: context.search,
    left: context.left,
    right: context.right,
  };
}
