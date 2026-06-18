import { describe, expect, it } from 'vitest';

import {
  createConversationLabel,
  getGreeting,
  getSelectedModelStatusLabel,
  resolveAssistantModelCapabilities,
  type ModelOption,
} from '../assistant-page-state';

const t = (key: string, values?: Record<string, unknown>) =>
  values ? `${key}:${values.formatted}` : key;

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
});
