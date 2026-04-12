import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';

import type { AudioTranscriptionControls } from '@/pages/audio/lib/audio-transcription-controls';
import { createUiStateStorage } from './ui-state-storage';

type PersistedAudioUiState = {
  modelControlOverrides: Record<string, Partial<AudioTranscriptionControls>>;
};

type AudioUiState = PersistedAudioUiState & {
  hasHydrated: boolean;
  setHasHydrated: (hasHydrated: boolean) => void;
  setModelControlOverrides: (
    modelId: string,
    overrides: Partial<AudioTranscriptionControls>,
  ) => void;
  clearModelControlOverrides: (modelId: string) => void;
};

const initialPersistedState: PersistedAudioUiState = {
  modelControlOverrides: {},
};

export const useAudioUiStore = create<AudioUiState>()(
  persist(
    (set) => ({
      hasHydrated: false,
      ...initialPersistedState,
      setHasHydrated: (hasHydrated) => set({ hasHydrated }),
      setModelControlOverrides: (modelId, overrides) => {
        const trimmedModelId = modelId.trim();

        if (!trimmedModelId) {
          return;
        }

        if (Object.keys(overrides).length === 0) {
          set((state) => {
            if (!state.modelControlOverrides[trimmedModelId]) {
              return state;
            }

            const nextOverrides = { ...state.modelControlOverrides };
            delete nextOverrides[trimmedModelId];
            return { modelControlOverrides: nextOverrides };
          });
          return;
        }

        set((state) => ({
          modelControlOverrides: {
            ...state.modelControlOverrides,
            [trimmedModelId]: overrides,
          },
        }));
      },
      clearModelControlOverrides: (modelId) => {
        const trimmedModelId = modelId.trim();

        if (!trimmedModelId) {
          return;
        }

        set((state) => {
          if (!state.modelControlOverrides[trimmedModelId]) {
            return state;
          }

          const nextOverrides = { ...state.modelControlOverrides };
          delete nextOverrides[trimmedModelId];
          return { modelControlOverrides: nextOverrides };
        });
      },
    }),
    {
      name: 'audio-ui',
      storage: createJSONStorage(() => createUiStateStorage()),
      partialize: ({ modelControlOverrides }) => ({
        modelControlOverrides,
      }),
      onRehydrateStorage: () => (state, error) => {
        if (error) {
          console.warn('Failed to rehydrate audio UI state.', error);
        }

        state?.setHasHydrated(true);
      },
    },
  ),
);
