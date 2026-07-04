import { useContext, useId, useLayoutEffect } from 'react';

import type {
  HeaderMeta,
  HeaderSearchConfig,
  HeaderSelectConfig,
} from '@/layouts/header';
import { HeaderContext, type HeaderContextValue } from '@/layouts/header-provider';

export type UseHeaderRegistration = {
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

export function useHeader(): Pick<HeaderContextValue, 'meta' | 'select' | 'search'>;
export function useHeader(registration: UseHeaderRegistration | null | undefined): Pick<
  HeaderContextValue,
  'meta' | 'select' | 'search'
>;
export function useHeader(registration?: UseHeaderRegistration | null) {
  const context = useRequiredHeaderContext('useHeader');
  const id = useId();
  const meta = registration?.meta ?? null;
  const select = registration?.select ?? null;
  const search = registration?.search ?? null;
  const {
    clearMeta,
    clearSearch,
    clearSelect,
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
    clearMeta,
    clearSearch,
    clearSelect,
    id,
    meta,
    registration,
    search,
    select,
    setMeta,
    setSearch,
    setSelect,
  ]);

  useLayoutEffect(
    () => () => {
      clearMeta(id);
      clearSelect(id);
      clearSearch(id);
    },
    [clearMeta, clearSearch, clearSelect, id],
  );

  return {
    meta: context.meta,
    select: context.select,
    search: context.search,
  };
}
