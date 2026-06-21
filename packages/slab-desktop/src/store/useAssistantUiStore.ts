import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';

import type { components } from '@slab/api';

import { createUiStateStorage } from './ui-state-storage';

type SessionLabelMap = Record<string, string>;
export type AssistantReasoningEffort = components['schemas']['ChatReasoningEffort'];
export type AssistantToolChoice = components['schemas']['AgentToolChoiceInput'];

type PersistedAssistantUiState = {
  currentSessionId: string;
  reasoningEffort: AssistantReasoningEffort;
  systemPrompt: string;
  toolConcurrency: number;
  toolChoice: AssistantToolChoice;
  advancedPanelOpen: boolean;
  sessionLabels: SessionLabelMap;
};

type AssistantUiState = PersistedAssistantUiState & {
  hasHydrated: boolean;
  setHasHydrated: (hasHydrated: boolean) => void;
  setCurrentSessionId: (sessionId: string) => void;
  setReasoningEffort: (reasoningEffort: AssistantReasoningEffort) => void;
  setSystemPrompt: (systemPrompt: string) => void;
  setToolConcurrency: (toolConcurrency: number) => void;
  setToolChoice: (toolChoice: AssistantToolChoice) => void;
  setAdvancedPanelOpen: (advancedPanelOpen: boolean) => void;
  setSessionLabel: (sessionId: string, label: string) => void;
  removeSessionLabel: (sessionId: string) => void;
};

const initialPersistedState: PersistedAssistantUiState = {
  currentSessionId: '',
  reasoningEffort: 'medium',
  systemPrompt: '',
  toolConcurrency: 1,
  toolChoice: { type: 'auto' },
  advancedPanelOpen: false,
  sessionLabels: {},
};

function normalizeToolConcurrency(value: number) {
  if (!Number.isFinite(value)) {
    return initialPersistedState.toolConcurrency;
  }

  return Math.min(4, Math.max(1, Math.trunc(value)));
}

function migrateAssistantUiState(value: unknown): PersistedAssistantUiState {
  if (typeof value !== 'object' || value === null) {
    return initialPersistedState;
  }

  const state = value as Partial<PersistedAssistantUiState> & { deepThink?: boolean };
  const reasoningEffort =
    state.reasoningEffort ??
    (typeof state.deepThink === 'boolean' ? (state.deepThink ? 'medium' : 'none') : 'medium');

  return {
    currentSessionId:
      typeof state.currentSessionId === 'string' ? state.currentSessionId.trim() : '',
    reasoningEffort,
    systemPrompt: typeof state.systemPrompt === 'string' ? state.systemPrompt : '',
    toolConcurrency: normalizeToolConcurrency(
      typeof state.toolConcurrency === 'number'
        ? state.toolConcurrency
        : initialPersistedState.toolConcurrency
    ),
    toolChoice: state.toolChoice ?? initialPersistedState.toolChoice,
    advancedPanelOpen:
      typeof state.advancedPanelOpen === 'boolean'
        ? state.advancedPanelOpen
        : initialPersistedState.advancedPanelOpen,
    sessionLabels:
      typeof state.sessionLabels === 'object' && state.sessionLabels !== null
        ? state.sessionLabels
        : {},
  };
}

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
      setReasoningEffort: (reasoningEffort) => set({ reasoningEffort }),
      setSystemPrompt: (systemPrompt) => set({ systemPrompt }),
      setToolConcurrency: (toolConcurrency) =>
        set({
          toolConcurrency: normalizeToolConcurrency(toolConcurrency),
        }),
      setToolChoice: (toolChoice) => set({ toolChoice }),
      setAdvancedPanelOpen: (advancedPanelOpen) => set({ advancedPanelOpen }),
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
      version: 1,
      migrate: (value) => migrateAssistantUiState(value),
      partialize: ({
        currentSessionId,
        reasoningEffort,
        systemPrompt,
        toolConcurrency,
        toolChoice,
        advancedPanelOpen,
        sessionLabels,
      }) => ({
        currentSessionId,
        reasoningEffort,
        systemPrompt,
        toolConcurrency,
        toolChoice,
        advancedPanelOpen,
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
