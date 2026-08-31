import { afterEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from 'vitest-browser-react';

import useDesktopPlatform, { getDesktopPlatform, type DesktopPlatform } from '../use-desktop-platform';

type NavigatorWithUserAgentData = Navigator & {
  userAgentData?: { platform?: string };
};

const originalUserAgentData =
  Object.getOwnPropertyDescriptor(window.navigator, 'userAgentData') ??
  Object.getOwnPropertyDescriptor(Navigator.prototype, 'userAgentData');

function mockPlatformHints({
  userAgentDataPlatform,
  userAgent,
}: {
  userAgentDataPlatform?: string;
  userAgent: string;
}) {
  vi.spyOn(window.navigator, 'userAgent', 'get').mockReturnValue(userAgent);
  // `userAgentData` is a prototype getter in Chromium — shadow it with a plain
  // value so the low-entropy hint (or its absence) is fully controlled.
  Object.defineProperty(window.navigator, 'userAgentData', {
    configurable: true,
    value: userAgentDataPlatform == null ? undefined : { platform: userAgentDataPlatform },
  });
}

describe('desktop platform hooks', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    if (originalUserAgentData) {
      Object.defineProperty(window.navigator, 'userAgentData', originalUserAgentData);
    } else {
      delete (window.navigator as NavigatorWithUserAgentData).userAgentData;
    }
  });

  it.each([
    ['macOS', 'Mozilla/5.0', 'macos'],
    [undefined, 'Mozilla/5.0 (Mac OS X)', 'macos'],
    ['Windows', 'Mozilla/5.0', 'windows'],
    [undefined, 'Mozilla/5.0 (Windows NT 10.0)', 'windows'],
    ['Linux x86_64', 'Mozilla/5.0', 'linux'],
    [undefined, 'Mozilla/5.0 (X11; Linux x86_64)', 'linux'],
    ['FreeBSD amd64', 'Mozilla/5.0', 'unknown'],
  ])('detects hint %p / UA %p as %s', (userAgentDataPlatform, userAgent, expected) => {
    mockPlatformHints({ userAgentDataPlatform, userAgent });

    expect(getDesktopPlatform()).toBe(expected);
  });

  it('exposes the detected platform through the React hook', async () => {
    mockPlatformHints({
      userAgentDataPlatform: 'Windows',
      userAgent: 'Mozilla/5.0',
    });

    const { result } = await renderHook(() => useDesktopPlatform());

    expect(result.current).toBe('windows' satisfies DesktopPlatform);
  });
});
