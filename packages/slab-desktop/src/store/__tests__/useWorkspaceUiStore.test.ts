import { beforeEach, describe, expect, it } from 'vitest';

import './mock-ui-state-storage';
import {
  defaultEditorSettings,
  emptyWorkspaceUiSnapshot,
  useWorkspaceUiStore,
} from '../useWorkspaceUiStore';

describe('useWorkspaceUiStore', () => {
  beforeEach(() => {
    useWorkspaceUiStore.setState({
      hasHydrated: false,
      recentWorkspaces: [],
      workspaces: {},
    });
  });

  it('starts with no workspace snapshots', () => {
    const state = useWorkspaceUiStore.getState();

    expect(state.hasHydrated).toBe(false);
    expect(state.recentWorkspaces).toEqual([]);
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

  it('remembers recent workspaces newest first and de-dupes by root path', () => {
    const state = useWorkspaceUiStore.getState();

    state.rememberRecentWorkspace({ lastOpenedAt: 10, name: 'Repo A', rootPath: '  C:/repo-a  ' });
    state.rememberRecentWorkspace({ lastOpenedAt: 20, name: 'Repo B', rootPath: 'C:/repo-b' });
    state.rememberRecentWorkspace({ lastOpenedAt: 30, name: 'Repo A renamed', rootPath: 'C:/repo-a' });

    expect(useWorkspaceUiStore.getState().recentWorkspaces).toEqual([
      { lastOpenedAt: 30, name: 'Repo A renamed', rootPath: 'C:/repo-a' },
      { lastOpenedAt: 20, name: 'Repo B', rootPath: 'C:/repo-b' },
    ]);
  });

  it('caps recent workspaces at ten entries', () => {
    const state = useWorkspaceUiStore.getState();

    for (let index = 0; index < 12; index += 1) {
      state.rememberRecentWorkspace({
        lastOpenedAt: index,
        name: `Repo ${index}`,
        rootPath: `C:/repo-${index}`,
      });
    }

    const recentWorkspaces = useWorkspaceUiStore.getState().recentWorkspaces;
    expect(recentWorkspaces).toHaveLength(10);
    expect(recentWorkspaces[0]?.rootPath).toBe('C:/repo-11');
    expect(recentWorkspaces.at(-1)?.rootPath).toBe('C:/repo-2');
  });

  it('ignores empty recent workspace roots and falls back to a root-derived name', () => {
    const state = useWorkspaceUiStore.getState();

    state.rememberRecentWorkspace({ name: 'Ignored', rootPath: '   ' });
    state.rememberRecentWorkspace({ lastOpenedAt: 40, name: '   ', rootPath: 'C:/projects/slab' });

    expect(useWorkspaceUiStore.getState().recentWorkspaces).toEqual([
      { lastOpenedAt: 40, name: 'slab', rootPath: 'C:/projects/slab' },
    ]);
  });
});
