import type { components } from './v1.d.ts';

const LOCAL_PROVIDER_PREFIX = 'local.';

type UnknownRecord = Record<string, unknown>;

export type UnifiedModelResponse = components['schemas']['UnifiedModelResponse'];
export type CatalogModelStatus = 'ready' | 'not_downloaded' | 'downloading' | 'error';

export type CatalogModel = Omit<UnifiedModelResponse, 'status'> & {
  status: CatalogModelStatus;
  backend_id: string | null;
  backend_ids: string[];
  repo_id: string;
  filename: string;
  local_path: string | null;
  pending: boolean;
};

export function getLocalBackendId(provider: string): string | null {
  if (!provider.startsWith(LOCAL_PROVIDER_PREFIX)) {
    return null;
  }

  const backendId = provider.slice(LOCAL_PROVIDER_PREFIX.length).trim();
  return backendId.length > 0 ? backendId : null;
}

export function normalizeModelStatus(status: string): CatalogModelStatus {
  switch (status) {
    case 'ready':
    case 'not_downloaded':
    case 'downloading':
    case 'error':
      return status;
    default:
      return 'error';
  }
}

export function normalizeCatalogModel(model: UnifiedModelResponse): CatalogModel {
  const backendId = getLocalBackendId(model.provider);
  const status = normalizeModelStatus(model.status);
  const localPath = model.spec.local_path ?? null;

  return {
    ...model,
    status,
    backend_id: backendId,
    backend_ids: backendId ? [backendId] : [],
    repo_id: model.spec.repo_id ?? '',
    filename: model.spec.filename ?? '',
    local_path: localPath,
    pending: status === 'downloading',
  };
}

export function toCatalogModelList(payload: unknown): CatalogModel[] {
  return toUnifiedModelList(payload).map(normalizeCatalogModel);
}

export function inferWhisperVadModel(
  model: Pick<CatalogModel, 'display_name' | 'repo_id' | 'filename'>,
): boolean {
  const haystack = `${model.display_name} ${model.repo_id} ${model.filename}`.toLowerCase();
  return (
    haystack.includes(' silero') ||
    haystack.includes('silero ') ||
    haystack.includes('-vad') ||
    haystack.includes('_vad') ||
    haystack.includes(' vad') ||
    haystack.includes('vad ') ||
    haystack.endsWith('vad')
  );
}

export function toUnifiedModelList(payload: unknown): UnifiedModelResponse[] {
  return Array.isArray(payload)
    ? payload.filter((item): item is UnifiedModelResponse => isUnifiedModelResponse(item))
    : [];
}

function isUnifiedModelResponse(value: unknown): value is UnifiedModelResponse {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const model = value as UnknownRecord;
  if (
    typeof model.id !== 'string' ||
    typeof model.display_name !== 'string' ||
    typeof model.provider !== 'string' ||
    typeof model.status !== 'string' ||
    typeof model.created_at !== 'string' ||
    typeof model.updated_at !== 'string'
  ) {
    return false;
  }

  if (typeof model.spec !== 'object' || model.spec === null || Array.isArray(model.spec)) {
    return false;
  }

  const spec = model.spec as UnknownRecord;
  return (
    isOptionalString(spec.provider_id) &&
    isOptionalString(spec.remote_model_id) &&
    isOptionalString(spec.repo_id) &&
    isOptionalString(spec.filename) &&
    isOptionalString(spec.local_path)
  );
}

function isOptionalString(value: unknown): boolean {
  return value === undefined || value === null || typeof value === 'string';
}
