import { vi } from 'vitest';

/** Signature shared by every mocked `sonner` toast method. */
export type ToastMethod = (message?: string, options?: unknown) => void;

/**
 * Mock shape for the `sonner` toast module. Covers the union observed across
 * slab-desktop tests: `success`, `error`, `info`, `message`.
 *
 * Use as `vi.mock('sonner', () => setupToastMock())` and read handles back
 * through `vi.mocked((await import('sonner')).toast.error)`.
 */
export interface ToastMockShape {
  toast: {
    success: ToastMethod;
    error: ToastMethod;
    info: ToastMethod;
    message: ToastMethod;
  };
}

/**
 * Build a fresh `sonner` toast mock shape. Each call creates new `vi.fn()`
 * handles so tests stay isolated.
 */
export function setupToastMock(overrides: Partial<ToastMockShape['toast']> = {}): ToastMockShape {
  return {
    toast: {
      success: vi.fn<ToastMethod>(),
      error: vi.fn<ToastMethod>(),
      info: vi.fn<ToastMethod>(),
      message: vi.fn<ToastMethod>(),
      ...overrides,
    },
  };
}
