import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import { enUS } from './locales/en-US';
import { zhCN } from './locales/zh-CN';

export const SUPPORTED_LANGUAGES = ['en-US', 'zh-CN'] as const;
export const APP_LANGUAGE_PREFERENCES = ['auto', ...SUPPORTED_LANGUAGES] as const;
export const APP_LANGUAGE_STORAGE_KEY = 'slab.ui.language';

export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];
export type AppLanguagePreference = (typeof APP_LANGUAGE_PREFERENCES)[number];
export const DEFAULT_CHAT_LABELS = [
  enUS.pages.chat.runtime.newChat,
  enUS.pages.chat.runtime.newConversation,
  zhCN.pages.chat.runtime.newChat,
  zhCN.pages.chat.runtime.newConversation,
] as const;

const DEFAULT_LANGUAGE: SupportedLanguage = 'en-US';
const AUTO_LANGUAGE = 'auto' as const;
const LANGUAGE_LOOKUP = new Set<string>(SUPPORTED_LANGUAGES);
const SIMPLIFIED_CHINESE_PATTERNS = [/^zh$/i, /^zh[-_](cn|sg|hans)$/i];

function isSupportedLanguage(value: string | null | undefined): value is SupportedLanguage {
  return Boolean(value && LANGUAGE_LOOKUP.has(value));
}

export function isAppLanguagePreference(value: string | null | undefined): value is AppLanguagePreference {
  return value === AUTO_LANGUAGE || isSupportedLanguage(value);
}

function normalizeLanguage(value: string | null | undefined): SupportedLanguage {
  if (isSupportedLanguage(value)) {
    return value;
  }

  const normalized = value?.toLowerCase() ?? '';
  return SIMPLIFIED_CHINESE_PATTERNS.some((pattern) => pattern.test(normalized))
    ? 'zh-CN'
    : DEFAULT_LANGUAGE;
}

function detectNavigatorLanguage(): SupportedLanguage {
  if (typeof navigator === 'undefined') {
    return DEFAULT_LANGUAGE;
  }

  return normalizeLanguage(navigator.languages?.[0] ?? navigator.language);
}

function readStoredLanguagePreference(): AppLanguagePreference {
  if (typeof window === 'undefined') {
    return AUTO_LANGUAGE;
  }

  try {
    const value = window.localStorage.getItem(APP_LANGUAGE_STORAGE_KEY);
    return isAppLanguagePreference(value) ? value : AUTO_LANGUAGE;
  } catch {
    return AUTO_LANGUAGE;
  }
}

function persistLanguagePreference(preference: AppLanguagePreference) {
  if (typeof window === 'undefined') {
    return;
  }

  try {
    window.localStorage.setItem(APP_LANGUAGE_STORAGE_KEY, preference);
  } catch {
    // Ignore storage failures and keep the runtime language in memory.
  }
}

export function resolveAppLanguage(preference: AppLanguagePreference): SupportedLanguage {
  if (preference === AUTO_LANGUAGE) {
    return detectNavigatorLanguage();
  }

  return preference;
}

export function getStoredAppLanguagePreference(): AppLanguagePreference {
  return readStoredLanguagePreference();
}

export function getResolvedAppLanguage(): SupportedLanguage {
  return normalizeLanguage(i18n.resolvedLanguage ?? i18n.language);
}

export async function applyAppLanguagePreference(
  preference: AppLanguagePreference,
): Promise<SupportedLanguage> {
  const nextLanguage = resolveAppLanguage(preference);
  persistLanguagePreference(preference);
  await i18n.changeLanguage(nextLanguage);
  return nextLanguage;
}

const initialPreference = readStoredLanguagePreference();
const initialLanguage = resolveAppLanguage(initialPreference);

i18n.use(initReactI18next).init({
  resources: {
    'en-US': {
      translation: enUS,
    },
    'zh-CN': {
      translation: zhCN,
    },
  },
  supportedLngs: SUPPORTED_LANGUAGES,
  lng: initialLanguage,
  fallbackLng: DEFAULT_LANGUAGE,
  interpolation: {
    escapeValue: false,
  },
});

export * from 'react-i18next';
export default i18n;
