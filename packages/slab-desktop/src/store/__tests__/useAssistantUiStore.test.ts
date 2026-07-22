import { describe, it, expect, beforeEach } from 'vitest';
import './mock-ui-state-storage';
import { migrateAssistantUiState, normalizeToolConcurrency, useAssistantUiStore } from '../useAssistantUiStore';

describe('useAssistantUiStore', () => {
  beforeEach(() => {
    useAssistantUiStore.setState({
      currentSessionId: '',
      reasoningEffort: 'medium',
      systemPrompt: '',
      toolConcurrency: 1,
      toolChoice: { type: 'auto' },
      advancedPanelOpen: false,
      sessionLabels: {},
      hasHydrated: false,
    });
  });

  it('should have initial state', () => {
    const state = useAssistantUiStore.getState();
    expect(state.currentSessionId).toBe('');
    expect(state.reasoningEffort).toBe('medium');
    expect(state.systemPrompt).toBe('');
    expect(state.toolConcurrency).toBe(1);
    expect(state.toolChoice).toEqual({ type: 'auto' });
    expect(state.advancedPanelOpen).toBe(false);
    expect(state.sessionLabels).toEqual({});
    expect(state.hasHydrated).toBe(false);
  });

  it('should set current session ID', () => {
    useAssistantUiStore.getState().setCurrentSessionId('session-123');
    expect(useAssistantUiStore.getState().currentSessionId).toBe('session-123');
  });

  it('should trim whitespace from session ID', () => {
    useAssistantUiStore.getState().setCurrentSessionId('  session-123  ');
    expect(useAssistantUiStore.getState().currentSessionId).toBe('session-123');
  });

  it('should set assistant config state', () => {
    const state = useAssistantUiStore.getState();
    state.setReasoningEffort('high');
    state.setSystemPrompt('  follow project rules  ');
    state.setToolConcurrency(6);
    state.setToolChoice({ type: 'required' });
    state.setAdvancedPanelOpen(true);

    expect(useAssistantUiStore.getState().reasoningEffort).toBe('high');
    expect(useAssistantUiStore.getState().systemPrompt).toBe('  follow project rules  ');
    expect(useAssistantUiStore.getState().toolConcurrency).toBe(4);
    expect(useAssistantUiStore.getState().toolChoice).toEqual({ type: 'required' });
    expect(useAssistantUiStore.getState().advancedPanelOpen).toBe(true);
  });

  it('should set session label', () => {
    useAssistantUiStore.getState().setSessionLabel('session-123', 'My Chat');
    expect(useAssistantUiStore.getState().sessionLabels['session-123']).toBe('My Chat');
  });

  it('should trim whitespace from session label', () => {
    useAssistantUiStore.getState().setSessionLabel('session-123', '  My Chat  ');
    expect(useAssistantUiStore.getState().sessionLabels['session-123']).toBe('My Chat');
  });

  it('should not set session label for empty session ID', () => {
    useAssistantUiStore.getState().setSessionLabel('', 'My Chat');
    expect(useAssistantUiStore.getState().sessionLabels).toEqual({});
  });

  it('should not set session label for empty label', () => {
    useAssistantUiStore.getState().setSessionLabel('session-123', '');
    expect(useAssistantUiStore.getState().sessionLabels).toEqual({});
  });

  it('should remove session label', () => {
    const state = useAssistantUiStore.getState();
    state.setSessionLabel('session-123', 'My Chat');
    state.removeSessionLabel('session-123');
    expect(useAssistantUiStore.getState().sessionLabels['session-123']).toBeUndefined();
  });

  it('should handle removing non-existent session label', () => {
    useAssistantUiStore.getState().removeSessionLabel('non-existent');
    expect(useAssistantUiStore.getState().sessionLabels).toEqual({});
  });

  it('should set hasHydrated state', () => {
    useAssistantUiStore.getState().setHasHydrated(true);
    expect(useAssistantUiStore.getState().hasHydrated).toBe(true);
  });

  it('should maintain multiple session labels', () => {
    const state = useAssistantUiStore.getState();
    state.setSessionLabel('session-1', 'Chat 1');
    state.setSessionLabel('session-2', 'Chat 2');
    state.setSessionLabel('session-3', 'Chat 3');

    const nextState = useAssistantUiStore.getState();
    expect(Object.keys(nextState.sessionLabels)).toHaveLength(3);
    expect(nextState.sessionLabels['session-1']).toBe('Chat 1');
    expect(nextState.sessionLabels['session-2']).toBe('Chat 2');
    expect(nextState.sessionLabels['session-3']).toBe('Chat 3');
  });
});

const initialPersistedSnapshot = {
  currentSessionId: '',
  reasoningEffort: 'medium',
  systemPrompt: '',
  toolConcurrency: 1,
  toolChoice: { type: 'auto' },
  advancedPanelOpen: false,
  sessionLabels: {},
};

describe('normalizeToolConcurrency', () => {
  it.each([
    [1, 1],
    [2, 2],
    [4, 4],
    [6, 4],
    [100, 4],
    [0, 1],
    [-5, 1],
    [2.9, 2],
    [3.7, 3],
    [Number.NaN, 1],
    [Number.POSITIVE_INFINITY, 1],
    [Number.NEGATIVE_INFINITY, 1],
  ])('clamps %p to %p', (input, expected) => {
    expect(normalizeToolConcurrency(input)).toBe(expected);
  });
});

describe('migrateAssistantUiState', () => {
  it('returns the initial persisted state for non-object input', () => {
    expect(migrateAssistantUiState(null)).toEqual(initialPersistedSnapshot);
    expect(migrateAssistantUiState(undefined)).toEqual(initialPersistedSnapshot);
    expect(migrateAssistantUiState('not-an-object')).toEqual(initialPersistedSnapshot);
  });

  it('maps the legacy deepThink flag to reasoningEffort', () => {
    expect(migrateAssistantUiState({ deepThink: true })).toMatchObject({ reasoningEffort: 'medium' });
    expect(migrateAssistantUiState({ deepThink: false })).toMatchObject({ reasoningEffort: 'none' });
    // A non-boolean deepThink falls back to the medium default.
    expect(migrateAssistantUiState({ deepThink: 'yes' })).toMatchObject({ reasoningEffort: 'medium' });
  });

  it('prefers an explicit reasoningEffort over the legacy deepThink flag', () => {
    expect(
      migrateAssistantUiState({ reasoningEffort: 'high', deepThink: false }),
    ).toMatchObject({ reasoningEffort: 'high' });
  });

  it('coerces each persisted field defensively back to defaults', () => {
    expect(
      migrateAssistantUiState({
        currentSessionId: 123,
        systemPrompt: 456,
        toolConcurrency: 'many',
        advancedPanelOpen: 'yes',
        sessionLabels: null,
        toolChoice: undefined,
      }),
    ).toEqual(initialPersistedSnapshot);
  });

  it('preserves valid persisted values and trims the session id', () => {
    expect(
      migrateAssistantUiState({
        currentSessionId: '  s1  ',
        reasoningEffort: 'low',
        systemPrompt: 'rules',
        toolConcurrency: 3,
        toolChoice: { type: 'required' },
        advancedPanelOpen: true,
        sessionLabels: { s1: 'Chat 1' },
      }),
    ).toEqual({
      currentSessionId: 's1',
      reasoningEffort: 'low',
      systemPrompt: 'rules',
      toolConcurrency: 3,
      toolChoice: { type: 'required' },
      advancedPanelOpen: true,
      sessionLabels: { s1: 'Chat 1' },
    });
  });
});
