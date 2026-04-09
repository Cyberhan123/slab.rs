import type { UnifiedModelResponse } from '@/lib/api/models';
import { tauriAwareFetch } from '@/lib/api/tauri-transport';
import { SERVER_BASE_URL } from '@/lib/config';

export type ModelConfigFieldScope =
  | 'summary'
  | 'source'
  | 'load'
  | 'inference'
  | 'advanced';

export type ModelConfigValueType = 'string' | 'integer' | 'number' | 'boolean' | 'path' | 'json';

export type ModelConfigOrigin =
  | 'pack_manifest'
  | 'selected_preset'
  | 'selected_variant'
  | 'selected_backend_config'
  | 'pmid_fallback'
  | 'derived';

export type ModelConfigPresetOption = {
  id: string;
  label: string;
  description?: string | null;
  variant_id?: string | null;
  is_default: boolean;
};

export type ModelConfigVariantOption = {
  id: string;
  label: string;
  description?: string | null;
  repo_id?: string | null;
  filename?: string | null;
  local_path?: string | null;
  is_default: boolean;
};

export type ModelConfigSelectionResponse = {
  default_preset_id?: string | null;
  default_variant_id?: string | null;
  selected_preset_id?: string | null;
  selected_variant_id?: string | null;
  effective_preset_id?: string | null;
  effective_variant_id?: string | null;
  presets: ModelConfigPresetOption[];
  variants: ModelConfigVariantOption[];
};

export type ModelConfigSourceArtifact = {
  id: string;
  label: string;
  value: string;
};

export type ModelConfigSourceSummary = {
  source_kind: string;
  repo_id?: string | null;
  filename?: string | null;
  local_path?: string | null;
  artifacts: ModelConfigSourceArtifact[];
};

export type ModelConfigFieldResponse = {
  path: string;
  scope: ModelConfigFieldScope;
  label: string;
  description_md?: string | null;
  value_type: ModelConfigValueType;
  effective_value: unknown;
  origin: ModelConfigOrigin;
  editable: boolean;
  locked: boolean;
  json_schema?: unknown;
};

export type ModelConfigSectionResponse = {
  id: string;
  label: string;
  description_md?: string | null;
  fields: ModelConfigFieldResponse[];
};

export type ModelConfigDocumentResponse = {
  model_summary: UnifiedModelResponse;
  selection: ModelConfigSelectionResponse;
  sections: ModelConfigSectionResponse[];
  source_summary: ModelConfigSourceSummary;
  resolved_load_spec: unknown;
  resolved_inference_spec: unknown;
  warnings: string[];
};

export type UpdateModelConfigSelectionRequest = {
  selected_preset_id?: string | null;
  selected_variant_id?: string | null;
};

export async function fetchModelConfigDocument(id: string): Promise<ModelConfigDocumentResponse> {
  const response = await tauriAwareFetch(
    new URL(`/v1/models/${encodeURIComponent(id)}/config-document`, `${SERVER_BASE_URL}/`),
    { method: 'GET' },
  );
  const raw = await response.text();
  if (!response.ok) {
    throw new Error(parseApiError(raw, response.status));
  }

  return JSON.parse(raw) as ModelConfigDocumentResponse;
}

export async function updateModelConfigSelection(
  id: string,
  body: UpdateModelConfigSelectionRequest,
): Promise<UnifiedModelResponse> {
  const response = await tauriAwareFetch(
    new URL(`/v1/models/${encodeURIComponent(id)}/config-selection`, `${SERVER_BASE_URL}/`),
    {
      method: 'PUT',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    },
  );
  const raw = await response.text();
  if (!response.ok) {
    throw new Error(parseApiError(raw, response.status));
  }

  return JSON.parse(raw) as UnifiedModelResponse;
}

function parseApiError(raw: string, status: number) {
  if (!raw.trim()) {
    return `HTTP ${status}`;
  }

  try {
    const parsed = JSON.parse(raw) as { message?: unknown; error?: unknown };
    if (typeof parsed.message === 'string' && parsed.message.trim()) {
      return parsed.message;
    }
    if (typeof parsed.error === 'string' && parsed.error.trim()) {
      return parsed.error;
    }
  } catch {
    // Fall through to raw string handling.
  }

  return raw.trim();
}
