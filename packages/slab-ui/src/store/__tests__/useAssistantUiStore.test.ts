import { describe, it, expect, beforeEach } from 'vitest';
import './mock-ui-state-storage';
import {
  migrateAssistantUiState,
  normalizePermissionMode,
  normalizeToolConcurrency,
  useAssistantUiStore,
} from '../useAssistantUiStore';

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
      permissionMode: 'request_approval',
      approvalReviewModel: '',
      approvalReviewPrompt: '',
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
    expect(state.permissionMode).toBe('request_approval');
    expect(state.approvalReviewModel).toBe('');
    expect(state.approvalReviewPrompt).toBe('');
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

  it('should set the approve-for-me reviewer config', () => {
    const state = useAssistantUiStore.getState();
    state.setPermissionMode('approve_for_me');
    state.setApprovalReviewModel('  glm-4-flash  ');
    state.setApprovalReviewPrompt('deny git push');

    const next = useAssistantUiStore.getState();
    expect(next.permissionMode).toBe('approve_for_me');
    expect(next.approvalReviewModel).toBe('glm-4-flash');
    expect(next.approvalReviewPrompt).toBe('deny git push');
  });

  it('should reject invalid permission modes', () => {
    useAssistantUiStore.getState().setPermissionMode('yolo' as never);
    expect(useAssistantUiStore.getState().permissionMode).toBe('request_approval');
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
  permissionMode: 'request_approval',
  approvalReviewModel: '',
  approvalReviewPrompt: '',
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
        permissionMode: 'approve_for_me',
        approvalReviewModel: 'glm-4-flash',
        approvalReviewPrompt: 'deny git push',
      }),
    ).toEqual({
      currentSessionId: 's1',
      reasoningEffort: 'low',
      systemPrompt: 'rules',
      toolConcurrency: 3,
      toolChoice: { type: 'required' },
      advancedPanelOpen: true,
      sessionLabels: { s1: 'Chat 1' },
      permissionMode: 'approve_for_me',
      approvalReviewModel: 'glm-4-flash',
      approvalReviewPrompt: 'deny git push',
    });
  });

  it('defaults the v2 reviewer fields on pre-v2 state', () => {
    // A v1 payload (no reviewer fields) migrates with the defaults.
    expect(migrateAssistantUiState({ currentSessionId: 's1' })).toMatchObject({
      permissionMode: 'request_approval',
      approvalReviewModel: '',
      approvalReviewPrompt: '',
    });
  });

  it('falls back to request_approval for an invalid persisted permission mode', () => {
    expect(
      migrateAssistantUiState({ permissionMode: 'yolo', approvalReviewModel: 42 }),
    ).toMatchObject({
      permissionMode: 'request_approval',
      approvalReviewModel: '',
    });
  });
});

describe('normalizePermissionMode', () => {
  it.each([
    ['request_approval', 'request_approval'],
    ['approve_for_me', 'approve_for_me'],
    ['full_control', 'full_control'],
    ['custom', 'custom'],
    ['yolo', 'request_approval'],
    [undefined, 'request_approval'],
    [null, 'request_approval'],
    [42, 'request_approval'],
  ])('normalizes %p to %p', (input, expected) => {
    expect(normalizePermissionMode(input)).toBe(expected);
  });
});
