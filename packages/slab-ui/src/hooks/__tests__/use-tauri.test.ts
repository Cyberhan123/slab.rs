import { renderHook } from 'vitest-browser-react';
import { afterEach, describe, expect, it } from 'vitest';

import useIsTauri, { isTauri } from '../use-tauri';

function clearTauriInternals() {
  Reflect.deleteProperty(window, '__TAURI_INTERNALS__');
}

function setTauriInternals() {
  Object.defineProperty(window, '__TAURI_INTERNALS__', {
    configurable: true,
    value: {},
  });
}

describe('tauri environment hooks', () => {
  afterEach(() => {
    clearTauriInternals();
  });

  it('detects a web runtime when Tauri internals are absent', async () => {
    clearTauriInternals();

    expect(isTauri()).toBe(false);
    expect((await renderHook(() => useIsTauri())).result.current).toBe(false);
  });

  it('detects a Tauri runtime when internals are present', async () => {
    setTauriInternals();

    expect(isTauri()).toBe(true);
    expect((await renderHook(() => useIsTauri())).result.current).toBe(true);
  });
});
