import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';

import { createUiStateStorage } from './ui-state-storage';

type PersistedHeaderUiState = {
  selections: Record<string, string>;
};

type HeaderUiState = PersistedHeaderUiState & {
  hasHydrated: boolean;
  clearSelection: (key: string) => void;
  setHasHydrated: (hasHydrated: boolean) => void;
  setSelection: (key: string, value: string) => void;
};

const initialPersistedState: PersistedHeaderUiState = {
  selections: {},
};

export const useHeaderUiStore = create<HeaderUiState>()(
  persist(
    (set) => ({
      hasHydrated: false,
      ...initialPersistedState,
      clearSelection: (key) => {
        const trimmedKey = key.trim();

        if (!trimmedKey) {
          return;
        }

        set((state) => {
          if (!state.selections[trimmedKey]) {
            return state;
          }

          const selections = { ...state.selections };
          delete selections[trimmedKey];
          return { selections };
        });
      },
      setHasHydrated: (hasHydrated) => set({ hasHydrated }),
      setSelection: (key, value) => {
        const trimmedKey = key.trim();
        const trimmedValue = value.trim();

        if (!trimmedKey) {
          return;
        }

        if (!trimmedValue) {
          set((state) => {
            if (!state.selections[trimmedKey]) {
              return state;
            }

            const selections = { ...state.selections };
            delete selections[trimmedKey];
            return { selections };
          });
          return;
        }

        set((state) => ({
          selections: {
            ...state.selections,
            [trimmedKey]: trimmedValue,
          },
        }));
      },
    }),
    {
      name: 'header-ui',
      storage: createJSONStorage(() => createUiStateStorage()),
      partialize: ({ selections }) => ({
        selections,
      }),
      onRehydrateStorage: () => (state, error) => {
        if (error) {
          console.warn('Failed to rehydrate header UI state.', error);
        }

        state?.setHasHydrated(true);
      },
    },
  ),
);
