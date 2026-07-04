import { useCallback, useContext, useEffect, useId, useLayoutEffect } from 'react';

import type { HeaderControl, HeaderSearchControl } from '@/layouts/header';
import { HeaderContext, type HeaderContextValue } from '@/layouts/header-provider';
import { useHeaderUiStore } from '@/store/useHeaderUiStore';

type PersistedHeaderSelectOption = {
  id: string;
  disabled?: boolean;
};

type UsePersistedHeaderSelectOptions<TOption extends PersistedHeaderSelectOption> = {
  isLoading?: boolean;
  key: string;
  options: TOption[];
  getDefaultValue?: (options: TOption[]) => string | undefined;
};

function useRequiredHeaderContext(hookName: string): HeaderContextValue {
  const context = useContext(HeaderContext);

  if (!context) {
    throw new Error(`${hookName} must be used within HeaderProvider`);
  }

  return context;
}

export function useHeader(): Pick<HeaderContextValue, 'meta' | 'control' | 'search'> {
  const context = useRequiredHeaderContext('useHeader');

  return {
    meta: context.meta,
    control: context.control,
    search: context.search,
  };
}

export function useHeaderControl(control: HeaderControl | null | undefined): void {
  const { setControl, clearControl } = useRequiredHeaderContext('useHeaderControl');
  const id = useId();

  useLayoutEffect(() => {
    if (!control) {
      return undefined;
    }

    setControl(id, control);

    return () => {
      clearControl(id);
    };
  }, [clearControl, control, id, setControl]);
}

export function useHeaderSearch(search: HeaderSearchControl | null | undefined): void {
  const { setSearch, clearSearch } = useRequiredHeaderContext('useHeaderSearch');
  const id = useId();

  useLayoutEffect(() => {
    if (!search) {
      return undefined;
    }

    setSearch(id, search);

    return () => {
      clearSearch(id);
    };
  }, [clearSearch, id, search, setSearch]);
}

export function usePersistedHeaderSelect<TOption extends PersistedHeaderSelectOption>({
  isLoading = false,
  key,
  options,
  getDefaultValue,
}: UsePersistedHeaderSelectOptions<TOption>) {
  const hasHydrated = useHeaderUiStore((state) => state.hasHydrated);
  const value = useHeaderUiStore((state) => state.selections[key] ?? '');
  const setSelection = useHeaderUiStore((state) => state.setSelection);
  const clearSelection = useHeaderUiStore((state) => state.clearSelection);

  const setValue = useCallback(
    (nextValue: string) => {
      setSelection(key, nextValue);
    },
    [key, setSelection],
  );

  useEffect(() => {
    if (!hasHydrated || isLoading) {
      return;
    }

    const enabledOptions = options.filter((option) => !option.disabled);

    if (enabledOptions.length === 0) {
      if (value) {
        clearSelection(key);
      }
      return;
    }

    if (enabledOptions.some((option) => option.id === value)) {
      return;
    }

    const preferredValue = getDefaultValue?.(options) ?? '';
    const fallbackValue = enabledOptions.some((option) => option.id === preferredValue)
      ? preferredValue
      : enabledOptions[0]?.id ?? '';

    if (!fallbackValue) {
      clearSelection(key);
      return;
    }

    if (fallbackValue !== value) {
      setSelection(key, fallbackValue);
    }
  }, [clearSelection, getDefaultValue, hasHydrated, isLoading, key, options, setSelection, value]);

  return {
    hasHydrated,
    setValue,
    value,
  };
}
