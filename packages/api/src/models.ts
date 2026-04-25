import type { components } from './v1.d.ts';

type UnknownRecord = Record<string, unknown>;

export type UnifiedModelResponse = components['schemas']['UnifiedModelResponse'];
export type ModelCapability = components['schemas']['ModelCapability'];
export type ChatModelCapabilities = components['schemas']['ChatModelCapabilities'];
export type CatalogModelStatus = 'ready' | 'not_downloaded' | 'downloading' | 'error';

export type CatalogModel = Omit<UnifiedModelResponse, 'status'> & {
  status: CatalogModelStatus;
  backend_id: string | null;
  backend_ids: string[];
  capabilities: ModelCapability[];
  chat_capabilities: ChatModelCapabilities | null;
  repo_id: string;
  filename: string;
  local_path: string | null;
  pending: boolean;
};

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
  const backendId = model.backend_id ?? null;
  const status = normalizeModelStatus(model.status);
  const localPath = model.spec.local_path ?? null;
  const chatCapabilities =
    model.chat_capabilities && isChatModelCapabilities(model.chat_capabilities)
      ? model.chat_capabilities
      : null;

  return {
    ...model,
    status,
    backend_id: backendId,
    backend_ids: backendId ? [backendId] : [],
    capabilities: model.capabilities,
    chat_capabilities: chatCapabilities,
    repo_id: model.spec.repo_id ?? '',
    filename: model.spec.filename ?? '',
    local_path: localPath,
    pending: status === 'downloading',
  };
}

export function modelSupportsCapability(
  model: Pick<CatalogModel, 'capabilities'>,
  capability: ModelCapability,
): boolean {
  return model.capabilities.includes(capability);
}

export function toCatalogModelList(payload: unknown): CatalogModel[] {
  return toUnifiedModelList(payload).map(normalizeCatalogModel);
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
    (model.kind !== 'local' && model.kind !== 'cloud') ||
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
    isOptionalString(model.backend_id) &&
    isCapabilityList(model.capabilities) &&
    isOptionalChatModelCapabilities(model.chat_capabilities) &&
    isOptionalString(spec.provider_id) &&
    isOptionalString(spec.remote_model_id) &&
    isOptionalString(spec.repo_id) &&
    isOptionalString(spec.filename) &&
    isOptionalString(spec.local_path)
  );
}

function isCapabilityList(value: unknown): value is ModelCapability[] {
  return Array.isArray(value) && value.every((capability) => isModelCapability(capability));
}

function isModelCapability(value: unknown): value is ModelCapability {
  switch (value) {
    case 'text_generation':
    case 'audio_transcription':
    case 'image_generation':
    case 'image_embedding':
    case 'chat_generation':
    case 'audio_vad':
    case 'video_generation':
      return true;
    default:
      return false;
  }
}

function isOptionalChatModelCapabilities(
  value: unknown,
): value is ChatModelCapabilities | null | undefined {
  return value === undefined || value === null || isChatModelCapabilities(value);
}

function isChatModelCapabilities(value: unknown): value is ChatModelCapabilities {
  return (
    typeof value === 'object' &&
    value !== null &&
    !Array.isArray(value) &&
    typeof (value as UnknownRecord).raw_gbnf === 'boolean' &&
    typeof (value as UnknownRecord).structured_output === 'boolean' &&
    typeof (value as UnknownRecord).reasoning_controls === 'boolean'
  );
}

function isOptionalString(value: unknown): boolean {
  return value === undefined || value === null || typeof value === 'string';
}
