import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

import { createUiStateStorage } from "./ui-state-storage";

type PersistedPluginAuthorization = {
  /** pluginId -> set of slabApi permissions the user has explicitly allowed. */
  grants: Record<string, string[]>;
};

type PluginAuthorizationState = PersistedPluginAuthorization & {
  hasHydrated: boolean;
  setHasHydrated: (hasHydrated: boolean) => void;
  isAuthorized: (pluginId: string, permission: string) => boolean;
  grant: (pluginId: string, permission: string) => void;
  revoke: (pluginId: string, permission?: string) => void;
};

/**
 * Records the (pluginId × slabApi permission) pairs a user has explicitly approved
 * at the runtime first-reject prompt. Persisted to the shared UI state so approvals
 * survive reloads; revoking a grant re-prompts on the next call. This is a UX guard
 * only — the backend `authorize_slab_api_request` remains the source of truth for
 * whether a permission is honored.
 */
export const usePluginAuthorizationStore = create<PluginAuthorizationState>()(
  persist(
    (set, get) => ({
      grants: {},
      hasHydrated: false,
      setHasHydrated: (hasHydrated) => set({ hasHydrated }),
      isAuthorized: (pluginId, permission) =>
        get().grants[pluginId]?.includes(permission) ?? false,
      grant: (pluginId, permission) =>
        set((state) => {
          const current = state.grants[pluginId] ?? [];
          if (current.includes(permission)) {
            return state;
          }
          return { grants: { ...state.grants, [pluginId]: [...current, permission] } };
        }),
      revoke: (pluginId, permission) =>
        set((state) => {
          if (!permission) {
            if (!state.grants[pluginId]) {
              return state;
            }
            const { [pluginId]: _removed, ...rest } = state.grants;
            return { grants: rest };
          }
          const current = state.grants[pluginId];
          if (!current || !current.includes(permission)) {
            return state;
          }
          const next = current.filter((entry) => entry !== permission);
          const grants = { ...state.grants };
          if (next.length === 0) {
            delete grants[pluginId];
          } else {
            grants[pluginId] = next;
          }
          return { grants };
        }),
    }),
    {
      name: "plugin-authorization",
      storage: createJSONStorage(() => createUiStateStorage()),
      partialize: ({ grants }) => ({ grants }),
      onRehydrateStorage: () => (state, error) => {
        if (error) {
          console.warn("Failed to rehydrate plugin authorization grants.", error);
        }
        state?.setHasHydrated(true);
      },
    },
  ),
);
