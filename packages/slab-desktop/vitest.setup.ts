import '@testing-library/jest-dom';
import { cleanup } from '@testing-library/react';
import { afterEach, vi } from 'vitest';

// Clean up after each test
afterEach(() => {
  cleanup();
});

class MockIntersectionObserver {
  observe = vi.fn<() => void>();
  unobserve = vi.fn<() => void>();
  disconnect = vi.fn<() => void>();
}

class MockResizeObserver {
  observe = vi.fn<() => void>();
  unobserve = vi.fn<() => void>();
  disconnect = vi.fn<() => void>();
}

// Mock IntersectionObserver
global.IntersectionObserver = MockIntersectionObserver as unknown as typeof IntersectionObserver;

// Mock ResizeObserver
global.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver;

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
