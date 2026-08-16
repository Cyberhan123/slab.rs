import { translateServerField } from '@slab/i18n';

import type { SettingResponse } from './types';
import type { SettingsTranslate } from './schema';

export function settingsPropertyLabel(
  property: SettingResponse,
  t: SettingsTranslate,
): string {
  if (property.pmid === 'general.language') {
    return t('pages.settings.language.title');
  }

  if (property.pmid === 'models.download_source') {
    return t('pages.settings.modelSource.title');
  }

  return translateServerField(property.i18n, 'label', property.label, t);
}

export function settingsPropertyDescription(
  property: SettingResponse,
  t: SettingsTranslate,
): string {
  if (property.pmid === 'general.language') {
    return t('pages.settings.language.description');
  }

  if (property.pmid === 'models.download_source') {
    return t('pages.settings.modelSource.description');
  }

  return translateServerField(property.i18n, 'description_md', property.description_md, t);
}
