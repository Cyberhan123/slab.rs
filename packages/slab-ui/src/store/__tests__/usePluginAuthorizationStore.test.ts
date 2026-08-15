import { beforeEach, describe, expect, it } from 'vitest';

import './mock-ui-state-storage';
import { usePluginAuthorizationStore } from '../usePluginAuthorizationStore';

describe('usePluginAuthorizationStore', () => {
  beforeEach(() => {
    usePluginAuthorizationStore.setState({ grants: {}, hasHydrated: false });
  });

  it('starts with no grants', () => {
    const { isAuthorized } = usePluginAuthorizationStore.getState();
    expect(isAuthorized('plugin-a', 'chat:complete')).toBe(false);
  });

  it('grants a permission and recognizes it on later calls', () => {
    const store = usePluginAuthorizationStore.getState();

    expect(store.isAuthorized('plugin-a', 'chat:complete')).toBe(false);
    store.grant('plugin-a', 'chat:complete');

    expect(usePluginAuthorizationStore.getState().isAuthorized('plugin-a', 'chat:complete')).toBe(true);
    // Other plugins / permissions remain unauthorized.
    expect(usePluginAuthorizationStore.getState().isAuthorized('plugin-a', 'models:read')).toBe(false);
    expect(usePluginAuthorizationStore.getState().isAuthorized('plugin-b', 'chat:complete')).toBe(false);
  });

  it('does not duplicate an already-granted permission', () => {
    usePluginAuthorizationStore.getState().grant('plugin-a', 'chat:complete');
    usePluginAuthorizationStore.getState().grant('plugin-a', 'chat:complete');

    expect(usePluginAuthorizationStore.getState().grants['plugin-a']).toEqual(['chat:complete']);
  });

  it('revokes a single permission and re-prompts on the next call', () => {
    const store = usePluginAuthorizationStore.getState();
    store.grant('plugin-a', 'chat:complete');
    store.grant('plugin-a', 'models:read');

    usePluginAuthorizationStore.getState().revoke('plugin-a', 'chat:complete');

    const next = usePluginAuthorizationStore.getState();
    expect(next.isAuthorized('plugin-a', 'chat:complete')).toBe(false);
    expect(next.isAuthorized('plugin-a', 'models:read')).toBe(true);
  });

  it('revokes all grants for a plugin when no permission is given', () => {
    usePluginAuthorizationStore.getState().grant('plugin-a', 'chat:complete');

    usePluginAuthorizationStore.getState().revoke('plugin-a');

    expect(usePluginAuthorizationStore.getState().grants['plugin-a']).toBeUndefined();
  });

  it('deletes the plugin entry when its last granted permission is revoked', () => {
    usePluginAuthorizationStore.getState().grant('plugin-a', 'chat:complete');
    usePluginAuthorizationStore.getState().revoke('plugin-a', 'chat:complete');

    // The whole entry is removed rather than left behind as an empty array.
    expect(usePluginAuthorizationStore.getState().grants['plugin-a']).toBeUndefined();
    expect(Object.keys(usePluginAuthorizationStore.getState().grants)).not.toContain('plugin-a');
  });

  it('is a no-op when revoking a permission the plugin does not hold', () => {
    usePluginAuthorizationStore.getState().grant('plugin-a', 'chat:complete');
    usePluginAuthorizationStore.getState().revoke('plugin-a', 'models:read');
    usePluginAuthorizationStore.getState().revoke('plugin-unknown', 'chat:complete');

    expect(usePluginAuthorizationStore.getState().grants['plugin-a']).toEqual(['chat:complete']);
    expect(usePluginAuthorizationStore.getState().grants['plugin-unknown']).toBeUndefined();
  });

  it('reports unauthorized for both a missing entry and an empty grant list', () => {
    usePluginAuthorizationStore.setState({ grants: { 'plugin-empty': [] } });
    const { isAuthorized } = usePluginAuthorizationStore.getState();

    expect(isAuthorized('plugin-missing', 'chat:complete')).toBe(false);
    expect(isAuthorized('plugin-empty', 'chat:complete')).toBe(false);
  });

  it('sets hydration state', () => {
    usePluginAuthorizationStore.getState().setHasHydrated(true);
    expect(usePluginAuthorizationStore.getState().hasHydrated).toBe(true);
  });
});
