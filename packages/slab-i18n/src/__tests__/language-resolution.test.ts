import { beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * `src/index.ts` runs `i18n.use(initReactI18next).init({...})` at module load.
 * Mock the bare `i18next` dependency (not `@slab/i18n`, not `react-i18next`) so
 * the load-time side effect is a no-op and we control `changeLanguage` /
 * `resolvedLanguage` deterministically. `vi.hoisted` is hoisted above all
 * imports by vitest, so the `vi.mock` factory can close over `i18nextMock`.
 * The factory returns `{ default: i18nextMock }`, which is the same object
 * `import i18n from 'i18next'` resolves to inside `index.ts` — so mutating
 * `i18nextMock.resolvedLanguage` here is visible to `getResolvedAppLanguage`.
 */
interface I18nMockInstance {
  use: (plugin: unknown) => I18nMockInstance;
  init: (options?: unknown) => void;
  changeLanguage: (lng?: string) => Promise<string>;
  resolvedLanguage: string | undefined;
  language: string | undefined;
}

const i18nextMock = vi.hoisted((): I18nMockInstance => {
  const instance: I18nMockInstance = {
    use: vi.fn<(plugin: unknown) => I18nMockInstance>(() => instance),
    init: vi.fn<(options?: unknown) => void>(() => undefined),
    changeLanguage: vi.fn<(lng?: string) => Promise<string>>(() => Promise.resolve('en-US')),
    resolvedLanguage: 'en-US',
    language: 'en-US',
  };
  return instance;
});

vi.mock('i18next', () => ({ default: i18nextMock }));

import {
  APP_LANGUAGE_STORAGE_KEY,
  applyAppLanguagePreference,
  getResolvedAppLanguage,
  getStoredAppLanguagePreference,
  isAppLanguagePreference,
  resolveAppLanguage,
} from '../index';

/** Stub `globalThis.window` with a fresh in-memory `localStorage`. Returns the
 * mock so the test can drive `getItem` / assert on `setItem`. */
function stubLocalStorage() {
  const localStorage = {
    getItem: vi.fn<(key: string) => string | null>(() => null),
    setItem: vi.fn<(key: string, value: string) => void>(),
  };
  vi.stubGlobal('window', { localStorage });
  return localStorage;
}

beforeEach(() => {
  i18nextMock.resolvedLanguage = 'en-US';
  i18nextMock.language = 'en-US';
});

describe('normalizeLanguage routing (via resolveAppLanguage("auto"))', () => {
  // Traditional Chinese (region or script) must route to en-US, never zh-CN.
  it.each<[navLanguage: string, expected: string]>([
    ['zh-TW', 'en-US'],
    ['zh-HK', 'en-US'],
    ['zh-MO', 'en-US'],
    ['zh-Hant', 'en-US'],
    ['zh-tw', 'en-US'],
    ['zh-Hant-TW', 'en-US'],
    ['zh-TW-Hant', 'en-US'],
    ['zh_TW', 'en-US'],
    ['zh-Hant_TW', 'en-US'],
    ['zh-CN', 'zh-CN'],
    ['zh-cn', 'zh-CN'],
    ['zh', 'zh-CN'],
    ['zh-Hans', 'zh-CN'],
    ['zh-Hans-CN', 'zh-CN'],
    ['en', 'en-US'],
    ['en-US', 'en-US'],
    ['ja-JP', 'en-US'],
    ['fr', 'en-US'],
    ['', 'en-US'],
  ])('routes navigator language %j to %j', (navLanguage, expected) => {
    vi.stubGlobal('navigator', { language: navLanguage, languages: [navLanguage] });
    expect(resolveAppLanguage('auto')).toBe(expected);
  });
});

describe('detectNavigatorLanguage (via resolveAppLanguage("auto"))', () => {
  it('defaults to en-US when navigator is undefined', () => {
    // Node 21+ exposes a global `navigator`, so the undefined-guard branch is
    // only reachable by explicitly stubbing navigator away. unstubGlobals
    // restores the real navigator after the test.
    vi.stubGlobal('navigator', undefined);
    expect(resolveAppLanguage('auto')).toBe('en-US');
  });

  it('falls back to navigator.language when languages[0] is absent', () => {
    vi.stubGlobal('navigator', { language: 'zh-CN' });
    expect(resolveAppLanguage('auto')).toBe('zh-CN');
  });

  it('prefers navigator.languages[0] over navigator.language', () => {
    vi.stubGlobal('navigator', { language: 'en-US', languages: ['zh-TW', 'en-US'] });
    expect(resolveAppLanguage('auto')).toBe('en-US');
  });
});

describe('resolveAppLanguage', () => {
  it('returns the preference directly when not "auto"', () => {
    vi.stubGlobal('navigator', { language: 'zh-CN', languages: ['zh-CN'] });
    expect(resolveAppLanguage('en-US')).toBe('en-US');
    expect(resolveAppLanguage('zh-CN')).toBe('zh-CN');
  });

  it('detects via navigator when preference is "auto"', () => {
    vi.stubGlobal('navigator', { language: 'zh-CN', languages: ['zh-CN'] });
    expect(resolveAppLanguage('auto')).toBe('zh-CN');
  });
});

describe('getStoredAppLanguagePreference', () => {
  it.each<[stored: string | null, expected: string]>([
    ['en-US', 'en-US'],
    ['zh-CN', 'zh-CN'],
    ['auto', 'auto'],
    ['fr-FR', 'auto'],
    ['zh-TW', 'auto'],
    ['', 'auto'],
    [null, 'auto'],
  ])('returns %j for stored value %j', (stored, expected) => {
    const localStorage = stubLocalStorage();
    localStorage.getItem.mockReturnValue(stored);
    expect(getStoredAppLanguagePreference()).toBe(expected);
    expect(localStorage.getItem).toHaveBeenCalledWith(APP_LANGUAGE_STORAGE_KEY);
  });

  it('returns "auto" when localStorage.getItem throws', () => {
    const localStorage = stubLocalStorage();
    localStorage.getItem.mockImplementation(() => {
      throw new Error('denied');
    });
    expect(getStoredAppLanguagePreference()).toBe('auto');
  });

  it('returns "auto" when window is undefined', () => {
    expect(getStoredAppLanguagePreference()).toBe('auto');
  });
});

describe('applyAppLanguagePreference', () => {
  it('persists an explicit preference and switches i18next to it', async () => {
    const localStorage = stubLocalStorage();

    await expect(applyAppLanguagePreference('zh-CN')).resolves.toBe('zh-CN');

    expect(localStorage.setItem).toHaveBeenCalledWith(APP_LANGUAGE_STORAGE_KEY, 'zh-CN');
    expect(i18nextMock.changeLanguage).toHaveBeenCalledWith('zh-CN');
  });

  it('persists the preference but switches to the resolved language for "auto"', async () => {
    // Regression: "auto" persists the PREFERENCE ("auto"), while changeLanguage
    // is called with the *resolved* navigator language (e.g. zh-CN).
    vi.stubGlobal('navigator', { language: 'zh-CN', languages: ['zh-CN'] });
    const localStorage = stubLocalStorage();

    await expect(applyAppLanguagePreference('auto')).resolves.toBe('zh-CN');

    expect(localStorage.setItem).toHaveBeenCalledWith(APP_LANGUAGE_STORAGE_KEY, 'auto');
    expect(i18nextMock.changeLanguage).toHaveBeenCalledWith('zh-CN');
  });

  it('still switches i18next when window is undefined (persist is a no-op)', async () => {
    await expect(applyAppLanguagePreference('en-US')).resolves.toBe('en-US');
    expect(i18nextMock.changeLanguage).toHaveBeenCalledWith('en-US');
  });
});

describe('getResolvedAppLanguage', () => {
  it('normalizes the resolved i18next language', () => {
    i18nextMock.resolvedLanguage = 'zh-CN';
    expect(getResolvedAppLanguage()).toBe('zh-CN');
  });

  it('falls back to language when resolvedLanguage is unset', () => {
    i18nextMock.resolvedLanguage = undefined;
    i18nextMock.language = 'zh-CN';
    expect(getResolvedAppLanguage()).toBe('zh-CN');
  });

  it('routes traditional Chinese resolved language to en-US', () => {
    i18nextMock.resolvedLanguage = 'zh-TW';
    expect(getResolvedAppLanguage()).toBe('en-US');
  });

  it('defaults unsupported languages to en-US', () => {
    i18nextMock.resolvedLanguage = 'en';
    expect(getResolvedAppLanguage()).toBe('en-US');
  });

  it('defaults to en-US when neither resolvedLanguage nor language is set', () => {
    i18nextMock.resolvedLanguage = undefined;
    i18nextMock.language = undefined;
    expect(getResolvedAppLanguage()).toBe('en-US');
  });
});

describe('isAppLanguagePreference', () => {
  it.each<[value: string | null | undefined, expected: boolean]>([
    ['auto', true],
    ['en-US', true],
    ['zh-CN', true],
    ['zh-TW', false],
    ['en', false],
    ['AUTO', false],
    ['', false],
    [null, false],
    [undefined, false],
  ])('is %j for %j', (value, expected) => {
    expect(isAppLanguagePreference(value)).toBe(expected);
  });
});
