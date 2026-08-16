import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from 'vitest-browser-react';

const useMediaQueryMock = vi.hoisted(() =>
  vi.fn<
    (
      query: string,
      initialValue?: boolean,
      options?: { getInitialValueInEffect?: boolean },
    ) => boolean
  >(),
);

vi.mock('@mantine/hooks', () => ({
  useMediaQuery: useMediaQueryMock,
}));

import { useIsMobile } from '../use-mobile';

describe('useIsMobile', () => {
  beforeEach(() => {
    useMediaQueryMock.mockReset();
    useMediaQueryMock.mockReturnValue(false);
  });

  it('uses the mobile breakpoint with an effect-time media query initial value', async () => {
    const { result } = await renderHook(() => useIsMobile());

    expect(result.current).toBe(false);
    expect(useMediaQueryMock).toHaveBeenCalledWith('(max-width: 767px)', false, {
      getInitialValueInEffect: true,
    });
  });

  it('returns the current media query match value', async () => {
    useMediaQueryMock.mockReturnValue(true);

    const { result } = await renderHook(() => useIsMobile());

    expect(result.current).toBe(true);
  });
});
