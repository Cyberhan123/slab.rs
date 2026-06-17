import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

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

  it('uses the mobile breakpoint with an effect-time media query initial value', () => {
    const { result } = renderHook(() => useIsMobile());

    expect(result.current).toBe(false);
    expect(useMediaQueryMock).toHaveBeenCalledWith('(max-width: 767px)', false, {
      getInitialValueInEffect: true,
    });
  });

  it('returns the current media query match value', () => {
    useMediaQueryMock.mockReturnValue(true);

    const { result } = renderHook(() => useIsMobile());

    expect(result.current).toBe(true);
  });
});
