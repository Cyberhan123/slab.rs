import type { paths } from '@/lib/api';

export type SettingsDocumentResponse =
  paths['/v1/settings']['get']['responses'][200]['content']['application/json'];
export type SettingsSectionResponse = SettingsDocumentResponse['sections'][number];
export type SettingsSubsectionResponse = SettingsSectionResponse['subsections'][number];
export type SettingResponse =
  paths['/v1/settings/{pmid}']['get']['responses'][200]['content']['application/json'];
export type UpdateSettingRequest =
  paths['/v1/settings/{pmid}']['put']['requestBody']['content']['application/json'];

export type DraftValue = boolean | string;

export type FieldErrorState = {
  message: string;
  path: string;
};

export type FieldStatusState = {
  tone: 'dirty' | 'saving' | 'saved' | 'error';
  message: string;
};

export type SettingValidationErrorData = {
  message?: unknown;
  path?: unknown;
  pmid?: unknown;
  type?: unknown;
};
