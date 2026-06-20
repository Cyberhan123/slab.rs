import { beforeEach, describe, expect, it } from 'vitest';

import './mock-ui-state-storage';
import { usePluginAuthorizationStore } from '../usePluginAuthorizationStore';

describe('usePluginAuthorizationStore', () => {
  beforeEach(() => {
    usePluginAuthorizationStore.setState({ grants: {} });
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
});
