import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';

import { createUiStateStorage } from './ui-state-storage';

type SessionLabelMap = Record<string, string>;

type PersistedChatUiState = {
  currentSessionId: string;
  deepThink: boolean;
  sessionLabels: SessionLabelMap;
};

type ChatUiState = PersistedChatUiState & {
  hasHydrated: boolean;
  setHasHydrated: (hasHydrated: boolean) => void;
  setCurrentSessionId: (sessionId: string) => void;
  setDeepThink: (deepThink: boolean) => void;
  setSessionLabel: (sessionId: string, label: string) => void;
  removeSessionLabel: (sessionId: string) => void;
};

const initialPersistedState: PersistedChatUiState = {
  currentSessionId: '',
  deepThink: true,
  sessionLabels: {},
};

export const useChatUiStore = create<ChatUiState>()(
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
      name: 'chat-ui',
      storage: createJSONStorage(() => createUiStateStorage()),
      partialize: ({ currentSessionId, deepThink, sessionLabels }) => ({
        currentSessionId,
        deepThink,
        sessionLabels,
      }),
      onRehydrateStorage: () => (state, error) => {
        if (error) {
          console.warn('Failed to rehydrate chat UI state.', error);
        }

        state?.setHasHydrated(true);
      },
    }
  )
);
