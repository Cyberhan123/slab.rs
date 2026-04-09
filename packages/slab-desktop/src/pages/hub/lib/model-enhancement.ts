import type { UnifiedModelResponse } from '@/lib/api/models';
import { tauriAwareFetch } from '@/lib/api/tauri-transport';
import { SERVER_BASE_URL } from '@/lib/config';

export type ModelEnhancementPresetOption = {
  id: string;
  label: string;
  description?: string | null;
  variant_id?: string | null;
};

export type ModelEnhancementVariantOption = {
  id: string;
  label: string;
  description?: string | null;
  repo_id?: string | null;
  filename?: string | null;
  local_path?: string | null;
};

export type ModelEnhancementResponse = {
  model: UnifiedModelResponse;
  default_preset_id?: string | null;
  selected_preset_id?: string | null;
  selected_variant_id?: string | null;
  presets: ModelEnhancementPresetOption[];
  variants: ModelEnhancementVariantOption[];
  resolved_spec: UnifiedModelResponse['spec'];
  resolved_runtime_presets?: {
    temperature?: number | null;
    top_p?: number | null;
  } | null;
};

export type UpdateModelEnhancementRequest = {
  display_name: string;
  selected_preset_id?: string | null;
  selected_variant_id?: string | null;
  context_window?: number | null;
  chat_template?: string | null;
  runtime_presets?: {
    temperature?: number | null;
    top_p?: number | null;
  } | null;
};

export async function fetchModelEnhancement(id: string): Promise<ModelEnhancementResponse> {
  const response = await tauriAwareFetch(
    new URL(`/v1/models/${encodeURIComponent(id)}/enhancement`, `${SERVER_BASE_URL}/`),
    { method: 'GET' },
  );
  const raw = await response.text();
  if (!response.ok) {
    throw new Error(parseApiError(raw, response.status));
  }

  return JSON.parse(raw) as ModelEnhancementResponse;
}

export async function updateModelEnhancement(
  id: string,
  body: UpdateModelEnhancementRequest,
): Promise<UnifiedModelResponse> {
  const response = await tauriAwareFetch(
    new URL(`/v1/models/${encodeURIComponent(id)}/enhancement`, `${SERVER_BASE_URL}/`),
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
