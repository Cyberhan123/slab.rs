import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';

import { createUiStateStorage } from './ui-state-storage';

export type WorkspaceFileTab = {
  relativePath: string;
  name: string;
};

export type WorkspaceExplorerPanel = 'files' | 'git';
export type WorkspaceMarkdownMode = 'preview' | 'source';

type WorkspaceUiSnapshot = {
  activeFilePath: string | null;
  explorerPanel: WorkspaceExplorerPanel;
  markdownMode: WorkspaceMarkdownMode;
  consoleOpen: boolean;
  openDirectoryPaths: string[];
  openFiles: WorkspaceFileTab[];
};

type PersistedWorkspaceUiState = {
  workspaces: Record<string, WorkspaceUiSnapshot>;
};

type WorkspaceUiState = PersistedWorkspaceUiState & {
  hasHydrated: boolean;
  patchWorkspaceState: (rootPath: string, patch: Partial<WorkspaceUiSnapshot>) => void;
  setHasHydrated: (hasHydrated: boolean) => void;
};

export const emptyWorkspaceUiSnapshot: WorkspaceUiSnapshot = {
  activeFilePath: null,
  explorerPanel: 'files',
  markdownMode: 'preview',
  consoleOpen: false,
  openDirectoryPaths: [],
  openFiles: [],
};

const initialPersistedState: PersistedWorkspaceUiState = {
  workspaces: {},
};

export const useWorkspaceUiStore = create<WorkspaceUiState>()(
  persist(
    (set) => ({
      hasHydrated: false,
      ...initialPersistedState,
      patchWorkspaceState: (rootPath, patch) => {
        const trimmedRootPath = rootPath.trim();

        if (!trimmedRootPath) {
          return;
        }

        set((state) => ({
          workspaces: {
            ...state.workspaces,
            [trimmedRootPath]: {
              ...emptyWorkspaceUiSnapshot,
              ...state.workspaces[trimmedRootPath],
              ...patch,
            },
          },
        }));
      },
      setHasHydrated: (hasHydrated) => set({ hasHydrated }),
    }),
    {
      name: 'workspace-ui',
      storage: createJSONStorage(() => createUiStateStorage()),
      partialize: ({ workspaces }) => ({
        workspaces,
      }),
      onRehydrateStorage: () => (state, error) => {
        if (error) {
          console.warn('Failed to rehydrate workspace UI state.', error);
        }

        state?.setHasHydrated(true);
      },
    },
  ),
);
