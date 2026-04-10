import { useCallback, useEffect } from 'react';

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
