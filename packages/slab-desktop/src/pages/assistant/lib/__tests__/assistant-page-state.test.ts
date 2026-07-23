import { describe, expect, it } from 'vitest';

import type { AiModel } from '@/hooks/use-ai-model';

import {
  createConversationLabel,
  getGreeting,
  getSelectedModelStatusLabel,
  resolveAssistantModelCapabilities,
  toAssistantModelOption,
  type ModelOption,
} from '../assistant-page-state';

const t = (key: string, values?: Record<string, unknown>) =>
  values ? `${key}:${values.formatted}` : key;

function aiModel(overrides: Partial<AiModel> = {}): AiModel {
  return {
    backend_id: null,
    backend_ids: [],
    capabilities: ['chat_generation'],
    chat_capabilities: null,
    created_at: '2026-01-01T00:00:00Z',
    display_name: 'Model',
    filename: 'model.gguf',
    id: 'model-1',
    kind: 'local',
    local_path: null,
    pending: false,
    repo_id: 'owner/model',
    runtime_state: null,
    size_bytes: null,
    spec: {
      filename: 'model.gguf',
      local_path: null,
      provider_id: null,
      remote_model_id: null,
      repo_id: 'owner/model',
    },
    status: 'ready',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

function modelOption(overrides: Partial<ModelOption> = {}): ModelOption {
  return {
    capabilities: {
      raw_gbnf: true,
      reasoning_controls: false,
      structured_output: true,
    },
    downloaded: true,
    id: 'model-1',
    label: 'Local Model',
    pending: false,
    source: 'local',
    ...overrides,
  };
}

describe('assistant page state helpers', () => {
  it('normalizes conversation labels without keeping blank or overlong values', () => {
    expect(createConversationLabel('  Project review  ', 'New assistant')).toBe('Project review');
    expect(createConversationLabel('   ', 'New assistant')).toBe('New assistant');
    expect(createConversationLabel('x'.repeat(43), 'New assistant')).toBe(`${'x'.repeat(42)}...`);
    // Boundary: exactly 42 chars is not truncated.
    expect(createConversationLabel('x'.repeat(42), 'New assistant')).toBe('x'.repeat(42));
  });

  it('uses explicit model capabilities before source defaults', () => {
    expect(resolveAssistantModelCapabilities({
      chat_capabilities: null,
      kind: 'local',
    })).toEqual({
      raw_gbnf: true,
      reasoning_controls: false,
      structured_output: true,
    });
    expect(resolveAssistantModelCapabilities({
      chat_capabilities: null,
      kind: 'cloud',
    })).toEqual({
      raw_gbnf: false,
      reasoning_controls: true,
      structured_output: true,
    });
    expect(resolveAssistantModelCapabilities({
      chat_capabilities: {
        raw_gbnf: false,
        reasoning_controls: false,
        structured_output: false,
      },
      kind: 'cloud',
    })).toEqual({
      raw_gbnf: false,
      reasoning_controls: false,
      structured_output: false,
    });
  });

  it('selects greetings by local hour boundaries', () => {
    expect(getGreeting(new Date('2026-06-18T07:00:00'), t)).toBe(
      'pages.assistant.greeting.morning',
    );
    expect(getGreeting(new Date('2026-06-18T13:00:00'), t)).toBe(
      'pages.assistant.greeting.afternoon',
    );
    expect(getGreeting(new Date('2026-06-18T20:00:00'), t)).toBe(
      'pages.assistant.greeting.evening',
    );
    // Boundary hours: 00:00 and 11:xx are morning, 12:00 tips into afternoon,
    // 18:00 tips into evening.
    expect(getGreeting(new Date('2026-06-18T00:00:00'), t)).toBe(
      'pages.assistant.greeting.morning',
    );
    expect(getGreeting(new Date('2026-06-18T12:00:00'), t)).toBe(
      'pages.assistant.greeting.afternoon',
    );
    expect(getGreeting(new Date('2026-06-18T18:00:00'), t)).toBe(
      'pages.assistant.greeting.evening',
    );
  });

  it('keeps session and model readiness labels in priority order', () => {
    const base = {
      curConversation: 'session-1',
      eventsConnected: false,
      isCreatingSession: false,
      isDeletingSession: false,
      isHistoryLoading: false,
      isPreparingModel: false,
      isSessionBootstrapping: false,
      modelLoading: false,
      resolvedLanguage: 'en-US',
      selectedModel: modelOption(),
      selectedRuntimeContextLength: null,
      t,
    };

    expect(getSelectedModelStatusLabel({ ...base, curConversation: null })).toBe(
      'pages.assistant.status.preparingSession',
    );
    expect(getSelectedModelStatusLabel({ ...base, isHistoryLoading: true })).toBe(
      'pages.assistant.status.loadingSessionHistory',
    );
    expect(getSelectedModelStatusLabel({
      ...base,
      eventsConnected: true,
      selectedRuntimeContextLength: 8192,
    })).toBe('Local Model / pages.assistant.status.runtimeContextWindow:8,192 / pages.assistant.connection.connected');
    expect(getSelectedModelStatusLabel({
      ...base,
      selectedModel: modelOption({ downloaded: false }),
    })).toBe('Local Model / pages.assistant.status.needsDownload');
    expect(getSelectedModelStatusLabel({
      ...base,
      isPreparingModel: true,
      selectedModel: modelOption({ contextWindow: null }),
    })).toBe('Local Model / pages.assistant.status.preparing');
    expect(getSelectedModelStatusLabel({
      ...base,
      selectedModel: modelOption({ label: 'Cloud Model', source: 'cloud' }),
    })).toBe('Cloud Model / pages.assistant.status.cloudModel');
  });

  it('reports the remaining readiness states in priority order', () => {
    const base = {
      curConversation: 'session-1',
      eventsConnected: false,
      isCreatingSession: false,
      isDeletingSession: false,
      isHistoryLoading: false,
      isPreparingModel: false,
      isSessionBootstrapping: false,
      modelLoading: false,
      resolvedLanguage: 'en-US',
      selectedModel: modelOption(),
      selectedRuntimeContextLength: null,
      t,
    };

    // Session lifecycle states win over model states.
    expect(getSelectedModelStatusLabel({ ...base, isCreatingSession: true })).toBe(
      'pages.assistant.status.creatingSession',
    );
    expect(getSelectedModelStatusLabel({ ...base, isDeletingSession: true })).toBe(
      'pages.assistant.status.deletingSession',
    );
    expect(getSelectedModelStatusLabel({ ...base, modelLoading: true })).toBe(
      'pages.assistant.status.loadingModels',
    );
    // No selected model yet.
    expect(getSelectedModelStatusLabel({ ...base, selectedModel: undefined })).toBe(
      'pages.assistant.status.selectModel',
    );
    // A pending download surfaces before the local-not-downloaded branch.
    expect(
      getSelectedModelStatusLabel({ ...base, selectedModel: modelOption({ pending: true }) })
    ).toBe('Local Model / pages.assistant.status.downloading');
    // A connected indicator appends even when no status qualifier is selected.
    expect(
      getSelectedModelStatusLabel({ ...base, eventsConnected: true })
    ).toBe('Local Model / pages.assistant.connection.connected');
  });

  it('maps assistant models to picker options with downloaded/source derived state', () => {
    // Cloud models are always considered downloaded and carry cloud defaults.
    expect(toAssistantModelOption(aiModel({ kind: 'cloud', display_name: 'Cloud' }))).toEqual({
      capabilities: {
        raw_gbnf: false,
        reasoning_controls: true,
        structured_output: true,
      },
      contextWindow: null,
      downloaded: true,
      id: 'model-1',
      label: 'Cloud',
      pending: false,
      runtimePresets: null,
      source: 'cloud',
    });

    // Local model: downloaded only when ready AND a local_path is present.
    expect(
      toAssistantModelOption(aiModel({ status: 'ready', local_path: '/models/m.gguf' })).downloaded,
    ).toBe(true);
    expect(toAssistantModelOption(aiModel({ status: 'ready', local_path: null })).downloaded).toBe(
      false,
    );
    expect(toAssistantModelOption(aiModel({ status: 'not_downloaded' })).downloaded).toBe(false);

    // Pending flag and spec context window pass through; local source keeps local defaults.
    expect(
      toAssistantModelOption(
        aiModel({
          pending: true,
          spec: {
            context_window: 8192,
            filename: 'model.gguf',
            local_path: null,
            provider_id: null,
            remote_model_id: null,
            repo_id: 'owner/model',
          },
        }),
      ),
    ).toMatchObject({
      contextWindow: 8192,
      downloaded: false,
      pending: true,
      source: 'local',
      capabilities: {
        raw_gbnf: true,
        reasoning_controls: false,
        structured_output: true,
      },
    });
  });
});
