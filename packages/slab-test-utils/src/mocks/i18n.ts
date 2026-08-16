import { vi } from 'vitest';

/**
 * Result shape of the mocked `useTranslation()` hook. `t` defaults to a
 * passthrough that also interpolates `{ count }` (`key` → `key`, `key` + count
 * → `key:count`); tests needing full-param or custom translation pass an
 * override.
 */
export interface SlabI18nTranslationResult {
  t: (key: string, options?: { count?: number }) => string;
  i18n: { resolvedLanguage: string; language: string };
}

/**
 * Mock shape for `@slab/i18n`. Mirrors the subset of the real module surface
 * used across slab-desktop tests: the default i18next instance (`.t`), the
 * react-i18next re-exports (`useTranslation` / `Trans` / `initReactI18next`),
 * and the package's own named exports.
 *
 * Use as `vi.mock('@slab/i18n', () => setupSlabI18nMock())` and read handles
 * back through `vi.mocked((await import('@slab/i18n')).useTranslation)`.
 */
export interface SlabI18nMockShape {
  default: { t: (key: string) => string };
  useTranslation: () => SlabI18nTranslationResult;
  Trans: (props: unknown) => unknown;
  initReactI18next: { type: string; init: () => void };
  translateServerField: (key: string) => string;
  getResolvedAppLanguage: () => string;
  SUPPORTED_LANGUAGES: string[];
  DEFAULT_ASSISTANT_LABELS: string[];
  LEGACY_DEFAULT_CHAT_LABELS: string[];
}

/**
 * Build a fresh `@slab/i18n` mock shape. Each call creates new `vi.fn()` handles
 * so tests stay isolated. The default is the widest practical superset covering
 * the observed variants (passthrough + count-interpolation `t`, `i18n` context,
 * common named exports); pass `overrides` to replace any field.
 */
export function setupSlabI18nMock(overrides: Partial<SlabI18nMockShape> = {}): SlabI18nMockShape {
  const t = vi.fn<(key: string, options?: { count?: number }) => string>(
    (key, options) => (options?.count != null ? `${key}:${options.count}` : key),
  );
  const i18n = { resolvedLanguage: 'en-US', language: 'en-US' };
  return {
    default: { t: (key: string) => key },
    useTranslation: vi.fn<() => SlabI18nTranslationResult>(() => ({ t, i18n })),
    Trans: vi.fn<(props: unknown) => unknown>(),
    initReactI18next: { type: '3rdParty', init: vi.fn<() => void>() },
    translateServerField: vi.fn<(key: string) => string>((key) => key),
    getResolvedAppLanguage: vi.fn<() => string>(() => 'en-US'),
    SUPPORTED_LANGUAGES: ['en-US', 'zh-CN'],
    DEFAULT_ASSISTANT_LABELS: ['pages.assistant.runtime.newChat'],
    LEGACY_DEFAULT_CHAT_LABELS: ['New Conversation'],
    ...overrides,
  };
}
