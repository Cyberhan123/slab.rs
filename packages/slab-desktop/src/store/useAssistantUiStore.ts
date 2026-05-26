import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';

import { createUiStateStorage } from './ui-state-storage';

type SessionLabelMap = Record<string, string>;

type PersistedAssistantUiState = {
  currentSessionId: string;
  deepThink: boolean;
  sessionLabels: SessionLabelMap;
};

type AssistantUiState = PersistedAssistantUiState & {
  hasHydrated: boolean;
  setHasHydrated: (hasHydrated: boolean) => void;
  setCurrentSessionId: (sessionId: string) => void;
  setDeepThink: (deepThink: boolean) => void;
  setSessionLabel: (sessionId: string, label: string) => void;
  removeSessionLabel: (sessionId: string) => void;
};

const initialPersistedState: PersistedAssistantUiState = {
  currentSessionId: '',
  deepThink: true,
  sessionLabels: {},
};

function createAssistantUiStorage() {
  const storage = createUiStateStorage();

  return {
    ...storage,
    getItem: async (name: string) => {
      const value = await storage.getItem(name);
      if (value !== null || name !== 'assistant-ui') {
        return value;
      }

      return storage.getItem('chat-ui');
    },
  };
}

export const useAssistantUiStore = create<AssistantUiState>()(
  persist(
    (set) => ({
      hasHydrated: false,
      ...initialPersistedState,
      setHasHydrated: (hasHydrated) => set({ hasHydrated }),
      setCurrentSessionId: (sessionId) =>
        set({
          currentSessionId: sessionId.trim(),
        }),
      setDeepThink: (deepThink) => set({ deepThink }),
      setSessionLabel: (sessionId, label) => {
        const trimmedSessionId = sessionId.trim();
        const trimmedLabel = label.trim();

        if (!trimmedSessionId || !trimmedLabel) {
          return;
        }

        set((state) => ({
          sessionLabels: {
            ...state.sessionLabels,
            [trimmedSessionId]: trimmedLabel,
          },
        }));
      },
      removeSessionLabel: (sessionId) => {
        const trimmedSessionId = sessionId.trim();

        if (!trimmedSessionId) {
          return;
        }

        set((state) => {
          if (!state.sessionLabels[trimmedSessionId]) {
            return state;
          }

          const nextLabels = { ...state.sessionLabels };
          delete nextLabels[trimmedSessionId];
          return { sessionLabels: nextLabels };
        });
      },
    }),
    {
      name: 'assistant-ui',
      storage: createJSONStorage(() => createAssistantUiStorage()),
      partialize: ({ currentSessionId, deepThink, sessionLabels }) => ({
        currentSessionId,
        deepThink,
        sessionLabels,
      }),
      onRehydrateStorage: () => (state, error) => {
        if (error) {
          console.warn('Failed to rehydrate assistant UI state.', error);
        }

        state?.setHasHydrated(true);
      },
    }
  )
);
