import { describe, it, expect, beforeEach } from 'vitest';
import { useAppStore } from '../useAppStore';

describe('useAppStore', () => {
  beforeEach(() => {
    useAppStore.setState({ isLoading: false, greetMessage: '' });
  });

  it('should have initial state', () => {
    const state = useAppStore.getState();
    expect(state.isLoading).toBe(false);
    expect(state.greetMessage).toBe('');
  });

  it('should set loading state', () => {
    useAppStore.getState().setIsLoading(true);
    expect(useAppStore.getState().isLoading).toBe(true);
  });

  it('should set greet message', () => {
    useAppStore.getState().setGreetMessage('Hello, World!');
    expect(useAppStore.getState().greetMessage).toBe('Hello, World!');
  });

  it('should reset state', () => {
    const state = useAppStore.getState();
    state.setIsLoading(true);
    state.setGreetMessage('Test');
    state.resetState();

    expect(useAppStore.getState().isLoading).toBe(false);
    expect(useAppStore.getState().greetMessage).toBe('');
  });
});
