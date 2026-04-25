import type { paths } from '@slab/api';

export type SettingsDocumentResponse =
  paths['/v1/settings']['get']['responses'][200]['content']['application/json'];
export type SettingsSectionResponse = SettingsDocumentResponse['sections'][number];
export type SettingsSubsectionResponse = SettingsSectionResponse['subsections'][number];
export type SettingResponse =
  paths['/v1/settings/{pmid}']['get']['responses'][200]['content']['application/json'];
export type UpdateSettingRequest =
  paths['/v1/settings/{pmid}']['put']['requestBody']['content']['application/json'];

export type JsonPrimitive = boolean | number | string | null;
export type JsonValue = JsonPrimitive | JsonObject | JsonValue[];
export type JsonObject = {
  [key: string]: JsonValue;
};

export type DraftValue = JsonValue;

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
