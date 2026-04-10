import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';

import type { ImageGenerationControls } from '@/pages/image/lib/image-generation-controls';
import { createUiStateStorage } from './ui-state-storage';

type PersistedImageUiState = {
  modelControls: Record<string, ImageGenerationControls>;
};

type ImageUiState = PersistedImageUiState & {
  hasHydrated: boolean;
  setHasHydrated: (hasHydrated: boolean) => void;
  setModelControls: (modelId: string, controls: ImageGenerationControls) => void;
};

const initialPersistedState: PersistedImageUiState = {
  modelControls: {},
};

export const useImageUiStore = create<ImageUiState>()(
  persist(
    (set) => ({
      hasHydrated: false,
      ...initialPersistedState,
      setHasHydrated: (hasHydrated) => set({ hasHydrated }),
      setModelControls: (modelId, controls) => {
        const trimmedModelId = modelId.trim();

        if (!trimmedModelId) {
          return;
        }

        set((state) => ({
          modelControls: {
            ...state.modelControls,
            [trimmedModelId]: controls,
          },
        }));
      },
    }),
    {
      name: 'image-ui',
      storage: createJSONStorage(() => createUiStateStorage()),
      partialize: ({ modelControls }) => ({
        modelControls,
      }),
      onRehydrateStorage: () => (state, error) => {
        if (error) {
          console.warn('Failed to rehydrate image UI state.', error);
        }

        state?.setHasHydrated(true);
      },
    },
  ),
);
