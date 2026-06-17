import { renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import useDesktopPlatform, { getDesktopPlatform, type DesktopPlatform } from '../use-desktop-platform';

function mockNavigatorPlatform({
  platform,
  userAgent,
}: {
  platform: string;
  userAgent: string;
}) {
  vi.spyOn(window.navigator, 'platform', 'get').mockReturnValue(platform);
  vi.spyOn(window.navigator, 'userAgent', 'get').mockReturnValue(userAgent);
}

describe('desktop platform hooks', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it.each([
    ['MacIntel', 'Mozilla/5.0', 'macos'],
    ['x86_64', 'Mozilla/5.0 (Mac OS X)', 'macos'],
    ['Win32', 'Mozilla/5.0', 'windows'],
    ['x86_64', 'Mozilla/5.0 (Windows NT 10.0)', 'windows'],
    ['Linux x86_64', 'Mozilla/5.0', 'linux'],
    ['x86_64', 'Mozilla/5.0 (X11; Linux x86_64)', 'linux'],
    ['FreeBSD amd64', 'Mozilla/5.0', 'unknown'],
  ])('detects %s / %s as %s', (platform, userAgent, expected) => {
    mockNavigatorPlatform({ platform, userAgent });

    expect(getDesktopPlatform()).toBe(expected);
  });

  it('exposes the detected platform through the React hook', () => {
    mockNavigatorPlatform({
      platform: 'Win32',
      userAgent: 'Mozilla/5.0',
    });

    const { result } = renderHook(() => useDesktopPlatform());

    expect(result.current).toBe('windows' satisfies DesktopPlatform);
  });
});
