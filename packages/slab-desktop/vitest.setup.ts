import '@testing-library/jest-dom';
import { cleanup } from '@testing-library/react';
import { afterEach, vi } from 'vitest';

// Clean up after each test
afterEach(() => {
  cleanup();
});

// Mock IntersectionObserver
global.IntersectionObserver = vi.fn<typeof IntersectionObserver>().mockImplementation(
  () =>
    ({
      observe: vi.fn<() => void>(),
      unobserve: vi.fn<() => void>(),
      disconnect: vi.fn<() => void>(),
    }) as unknown as IntersectionObserver,
);

// Mock ResizeObserver
global.ResizeObserver = vi.fn<typeof ResizeObserver>().mockImplementation(
  () =>
    ({
      observe: vi.fn<() => void>(),
      unobserve: vi.fn<() => void>(),
      disconnect: vi.fn<() => void>(),
    }) as unknown as ResizeObserver,
);

// Mock matchMedia
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn<(query: string) => MediaQueryList>().mockImplementation((query) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn<() => void>(),
    removeListener: vi.fn<() => void>(),
    addEventListener: vi.fn<() => void>(),
    removeEventListener: vi.fn<() => void>(),
    dispatchEvent: vi.fn<() => boolean>(),
  })),
});
