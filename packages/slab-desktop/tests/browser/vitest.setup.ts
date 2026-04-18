import i18n, { APP_LANGUAGE_STORAGE_KEY } from '@slab/i18n';
import { beforeEach } from 'vitest';

import '@slab/components/globals.css';

beforeEach(async () => {
  window.localStorage.setItem(APP_LANGUAGE_STORAGE_KEY, 'en-US');
  document.documentElement.className = '';
  document.documentElement.lang = 'en-US';

  await i18n.changeLanguage('en-US');
});
