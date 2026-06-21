import { describe, it, expect, beforeEach } from 'vitest';
import './mock-ui-state-storage';
import { useAssistantUiStore } from '../useAssistantUiStore';

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
