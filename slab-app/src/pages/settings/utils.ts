import { getErrorMessage, isApiError } from '@/lib/api';

import { isJsonObject } from './schema';
import type {
  DraftValue,
  FieldErrorState,
  SettingResponse,
  SettingValidationErrorData,
  SettingsSectionResponse,
  SettingsSubsectionResponse,
  UpdateSettingRequest,
} from './types';

export function countProperties(sections: SettingsSectionResponse[]): number {
  return sections.reduce(
    (sectionTotal, section) =>
      sectionTotal +
      section.subsections.reduce(
        (subsectionTotal, subsection) => subsectionTotal + subsection.properties.length,
        0,
      ),
    0,
  );
}

export function countSectionProperties(section: SettingsSectionResponse): number {
  return countProperties([section]);
}

export function matchesSearch(
  section: SettingsSectionResponse,
  subsection: SettingsSubsectionResponse,
  property: SettingResponse,
  query: string,
): boolean {
  if (!query) {
    return true;
  }

  const haystack = [
    section.title,
    section.description_md,
    subsection.title,
    subsection.description_md,
    property.pmid,
    property.label,
    property.description_md,
    ...property.search_terms,
  ]
    .join(' ')
    .toLowerCase();

  return haystack.includes(query);
}

export function valueToEditorString(value: unknown): string {
  if (typeof value === 'string') {
    return value;
  }

  if (typeof value === 'number') {
    return Number.isFinite(value) ? String(value) : '';
  }

  if (value === null || value === undefined) {
    return '';
  }

  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return '';
  }
}

export function summarizeValue(value: unknown): string {
  if (typeof value === 'string') {
    return value.length === 0 ? '(empty string)' : value;
  }

  if (
    typeof value === 'number' ||
    typeof value === 'boolean' ||
    value === null ||
    value === undefined
  ) {
    return String(value ?? 'null');
  }

  const text = valueToEditorString(value);
  return text.length > 80 ? `${text.slice(0, 77)}...` : text;
}

export function extractStructuredError(error: unknown): FieldErrorState | null {
  if (!isApiError(error) || typeof error.data !== 'object' || error.data === null) {
    return null;
  }

  const data = error.data as SettingValidationErrorData;
  if (typeof data.message !== 'string' || typeof data.path !== 'string') {
    return null;
  }

  return {
    message: data.message,
    path: data.path,
  };
}

export function buildRequestBody(
  property: SettingResponse,
  draftValue: DraftValue | undefined,
): UpdateSettingRequest {
  const propertyType = property.schema.type;

  if (propertyType === 'boolean') {
    return {
      op: 'set',
      value:
        typeof draftValue === 'boolean'
          ? draftValue
          : Boolean(property.effective_value),
    };
  }

  const rawValue =
    typeof draftValue === 'string'
      ? draftValue
      : valueToEditorString(property.effective_value);

  if (propertyType === 'integer') {
    const trimmed = rawValue.trim();
    if (!trimmed) {
      return { op: 'unset' };
    }
    if (!/^-?\d+$/.test(trimmed)) {
      throw new Error('Value must be an integer.');
    }
    return {
      op: 'set',
      value: Number.parseInt(trimmed, 10),
    };
  }

  if (propertyType === 'array' || propertyType === 'object') {
    if (propertyType === 'array' && Array.isArray(draftValue)) {
      return {
        op: 'set',
        value: draftValue,
      };
    }

    if (propertyType === 'object' && isJsonObject(draftValue)) {
      return {
        op: 'set',
        value: draftValue,
      };
    }

    const trimmed = rawValue.trim();
    if (!trimmed) {
      return { op: 'unset' };
    }

    try {
      return {
        op: 'set',
        value: JSON.parse(trimmed),
      };
    } catch {
      throw new Error('Value must be valid JSON.');
    }
  }

  return {
    op: 'set',
    value: rawValue,
  };
}

export function autoSaveDelay(property: SettingResponse): number {
  const propertyType = property.schema.type;
  const isEnum =
    propertyType === 'string' &&
    Array.isArray(property.schema.enum) &&
    property.schema.enum.length > 0;

  if (propertyType === 'boolean' || isEnum) {
    return 150;
  }

  if (propertyType === 'array' || propertyType === 'object' || property.schema.multiline) {
    return 900;
  }

  return 650;
}

export function sectionAnchorId(sectionId: string): string {
  return `settings-section-${sectionId}`;
}

export function subsectionAnchorId(sectionId: string, subsectionId: string): string {
  return `settings-subsection-${sectionId}-${subsectionId}`;
}

export function fallbackErrorMessage(error: unknown): string {
  return getErrorMessage(error);
}
