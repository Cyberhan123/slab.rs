import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useChatUiStore } from '../useChatUiStore';

// Mock the UI state storage
vi.mock('../ui-state-storage', () => ({
  createUiStateStorage: () => ({
    getItem: vi.fn(async () => null),
    setItem: vi.fn(async () => {}),
    removeItem: vi.fn(async () => {}),
  }),
}));

describe('useChatUiStore', () => {
  beforeEach(() => {
    useChatUiStore.setState({
      currentSessionId: '',
      deepThink: true,
      sessionLabels: {},
      hasHydrated: false,
    });
  });

  it('should have initial state', () => {
    const state = useChatUiStore.getState();
    expect(state.currentSessionId).toBe('');
    expect(state.deepThink).toBe(true);
    expect(state.sessionLabels).toEqual({});
    expect(state.hasHydrated).toBe(false);
  });

  it('should set current session ID', () => {
    useChatUiStore.getState().setCurrentSessionId('session-123');
    expect(useChatUiStore.getState().currentSessionId).toBe('session-123');
  });

  it('should trim whitespace from session ID', () => {
    useChatUiStore.getState().setCurrentSessionId('  session-123  ');
    expect(useChatUiStore.getState().currentSessionId).toBe('session-123');
  });

  it('should set deep think state', () => {
    useChatUiStore.getState().setDeepThink(false);
    expect(useChatUiStore.getState().deepThink).toBe(false);
  });

  it('should set session label', () => {
    useChatUiStore.getState().setSessionLabel('session-123', 'My Chat');
    expect(useChatUiStore.getState().sessionLabels['session-123']).toBe('My Chat');
  });

  it('should trim whitespace from session label', () => {
    useChatUiStore.getState().setSessionLabel('session-123', '  My Chat  ');
    expect(useChatUiStore.getState().sessionLabels['session-123']).toBe('My Chat');
  });

  it('should not set session label for empty session ID', () => {
    useChatUiStore.getState().setSessionLabel('', 'My Chat');
    expect(useChatUiStore.getState().sessionLabels).toEqual({});
  });

  it('should not set session label for empty label', () => {
    useChatUiStore.getState().setSessionLabel('session-123', '');
    expect(useChatUiStore.getState().sessionLabels).toEqual({});
  });

  it('should remove session label', () => {
    const state = useChatUiStore.getState();
    state.setSessionLabel('session-123', 'My Chat');
    state.removeSessionLabel('session-123');
    expect(useChatUiStore.getState().sessionLabels['session-123']).toBeUndefined();
  });

  it('should handle removing non-existent session label', () => {
    useChatUiStore.getState().removeSessionLabel('non-existent');
    expect(useChatUiStore.getState().sessionLabels).toEqual({});
  });

  it('should set hasHydrated state', () => {
    useChatUiStore.getState().setHasHydrated(true);
    expect(useChatUiStore.getState().hasHydrated).toBe(true);
  });

  it('should maintain multiple session labels', () => {
    const state = useChatUiStore.getState();
    state.setSessionLabel('session-1', 'Chat 1');
    state.setSessionLabel('session-2', 'Chat 2');
    state.setSessionLabel('session-3', 'Chat 3');

    const nextState = useChatUiStore.getState();
    expect(Object.keys(nextState.sessionLabels)).toHaveLength(3);
    expect(nextState.sessionLabels['session-1']).toBe('Chat 1');
    expect(nextState.sessionLabels['session-2']).toBe('Chat 2');
    expect(nextState.sessionLabels['session-3']).toBe('Chat 3');
  });
});
