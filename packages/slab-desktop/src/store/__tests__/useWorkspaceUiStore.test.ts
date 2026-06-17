import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  defaultEditorSettings,
  emptyWorkspaceUiSnapshot,
  useWorkspaceUiStore,
} from '../useWorkspaceUiStore';

vi.mock('../ui-state-storage', () => ({
  createUiStateStorage: () => ({
    getItem: vi.fn<() => Promise<null>>(async () => null),
    removeItem: vi.fn<() => Promise<void>>(async () => {}),
    setItem: vi.fn<() => Promise<void>>(async () => {}),
  }),
}));

describe('useWorkspaceUiStore', () => {
  beforeEach(() => {
    useWorkspaceUiStore.setState({
      hasHydrated: false,
      workspaces: {},
    });
  });

  it('starts with no workspace snapshots', () => {
    const state = useWorkspaceUiStore.getState();

    expect(state.hasHydrated).toBe(false);
    expect(state.workspaces).toEqual({});
  });

  it('ignores empty workspace roots', () => {
    useWorkspaceUiStore.getState().patchWorkspaceState('   ', { consoleOpen: true });

    expect(useWorkspaceUiStore.getState().workspaces).toEqual({});
  });

  it('trims workspace roots and merges patches with defaults and existing state', () => {
    const state = useWorkspaceUiStore.getState();

    state.patchWorkspaceState('  C:/repo  ', {
      consoleOpen: true,
      editorSettings: {
        ...defaultEditorSettings,
        fontSize: 16,
      },
      openDirectoryPaths: ['src'],
    });
    state.patchWorkspaceState('C:/repo', {
      activeFilePath: 'src/main.rs',
    });

    expect(useWorkspaceUiStore.getState().workspaces['C:/repo']).toEqual({
      ...emptyWorkspaceUiSnapshot,
      activeFilePath: 'src/main.rs',
      consoleOpen: true,
      editorSettings: {
        ...defaultEditorSettings,
        fontSize: 16,
      },
      openDirectoryPaths: ['src'],
    });
  });

  it('keeps workspace snapshots isolated by root path', () => {
    const state = useWorkspaceUiStore.getState();

    state.patchWorkspaceState('C:/repo-a', { activeFilePath: 'a.ts' });
    state.patchWorkspaceState('C:/repo-b', { activeFilePath: 'b.ts', markdownMode: 'source' });

    expect(useWorkspaceUiStore.getState().workspaces['C:/repo-a'].activeFilePath).toBe('a.ts');
    expect(useWorkspaceUiStore.getState().workspaces['C:/repo-a'].markdownMode).toBe('preview');
    expect(useWorkspaceUiStore.getState().workspaces['C:/repo-b'].activeFilePath).toBe('b.ts');
    expect(useWorkspaceUiStore.getState().workspaces['C:/repo-b'].markdownMode).toBe('source');
  });

  it('sets hydration state', () => {
    useWorkspaceUiStore.getState().setHasHydrated(true);

    expect(useWorkspaceUiStore.getState().hasHydrated).toBe(true);
  });
});
