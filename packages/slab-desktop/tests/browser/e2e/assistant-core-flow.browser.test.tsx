import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useState } from 'react';

import AssistantPage from '@/pages/assistant';
import type { AssistantMessageRecord } from '@/pages/assistant/assistant-context';
import { renderDesktopScene } from '../test-utils';

const {
  mockCatalogState,
  mockHandleSubmit,
  mockMutationState,
  mockSetDeepThink,
  mockUseAssistantAgent,
  mockUpdateSessionLabel,
} = vi.hoisted(() => ({
  mockCatalogState: {
    isLoading: false,
  },
  mockHandleSubmit: vi.fn<(value: string) => void>(),
  mockMutationState: {
    isPending: false,
  },
  mockSetDeepThink: vi.fn<() => void>(),
  mockUseAssistantAgent: vi.fn<() => unknown>(),
  mockUpdateSessionLabel: vi.fn<(sessionId: string, label: string) => Promise<boolean>>().mockResolvedValue(true),
}));

vi.mock('@/pages/assistant/hooks/use-assistant-agent', () => ({
  useAssistantAgent: mockUseAssistantAgent,
}));

vi.mock('@/pages/assistant/hooks/use-assistant-sessions', () => ({
  useAssistantSessions: vi.fn<() => unknown>(() => ({
    conversationList: [
      {
        group: 'workspace',
        key: 'session-1',
        label: 'New assistant',
      },
    ],
    createSession: vi.fn<() => Promise<{ id: string }>>().mockResolvedValue({ id: 'session-2' }),
    currentSessionId: 'session-1',
    deleteSession: vi.fn<() => Promise<boolean>>().mockResolvedValue(true),
    isCreatingSession: false,
    isDeletingSession: false,
    isSessionMutating: false,
    isSessionsLoading: false,
    setCurrentSessionId: vi.fn<() => void>(),
    setSessionLabel: vi.fn<() => void>(),
    updateSessionLabel: mockUpdateSessionLabel,
  })),
}));

vi.mock('@/pages/assistant/hooks/use-markdown-theme', () => ({
  useMarkdownTheme: vi.fn<() => unknown>(() => ['markdown-theme-dark']),
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderControl: vi.fn<() => void>(),
}));

vi.mock('@/hooks/use-persisted-header-select', () => ({
  usePersistedHeaderSelect: vi.fn<() => unknown>(() => ({
    setValue: vi.fn<() => void>(),
    value: 'cloud-model',
  })),
}));

vi.mock('@/store/useAssistantUiStore', () => ({
  useAssistantUiStore: vi.fn<(selector?: (state: Record<string, unknown>) => unknown) => unknown>((selector) => {
    const state = {
      advancedPanelOpen: false,
      reasoningEffort: 'medium',
      setAdvancedPanelOpen: vi.fn<() => void>(),
      setReasoningEffort: mockSetDeepThink,
      setSystemPrompt: vi.fn<() => void>(),
      setToolChoice: vi.fn<() => void>(),
      setToolConcurrency: vi.fn<() => void>(),
      systemPrompt: '',
      toolChoice: { type: 'auto' },
      toolConcurrency: 1,
    };
    return selector ? selector(state) : state;
  }),
}));

vi.mock('@slab/api', async () => {
  const { createSlabApiMock } = await import('../support/mock-slab-api');

  return createSlabApiMock({
    defaultExport: {
      useMutation: vi.fn<() => unknown>(() => ({
        isPending: mockMutationState.isPending,
        mutateAsync: vi.fn<() => Promise<Record<string, never>>>().mockResolvedValue({}),
      })),
      useQuery: vi.fn<() => unknown>(() => ({
        data: [
          {
            backend_ids: ['cloud.openai'],
            capabilities: ['chat_generation'],
            chat_capabilities: {
              raw_gbnf: false,
              reasoning_controls: true,
              structured_output: true,
            },
            display_name: 'Cloud Assistant',
            id: 'cloud-model',
            kind: 'cloud',
            local_path: null,
            pending: false,
            runtime_presets: null,
            spec: {
              context_window: 8192,
            },
            status: 'ready',
          },
        ],
        isLoading: mockCatalogState.isLoading,
        refetch: vi.fn<() => Promise<{ data: unknown[] }>>().mockResolvedValue({ data: [] }),
      })),
    },
  });
});

vi.mock('@slab/api/models', () => ({
  toCatalogModelList: vi.fn<(data: unknown) => unknown[]>((data) => (Array.isArray(data) ? data : [])),
}));

function createMockMessage(overrides: Partial<AssistantMessageRecord>): AssistantMessageRecord {
  return {
    id: 'msg-1',
    message: {
      content: '',
      role: 'user',
    },
    status: 'success',
    ...overrides,
  };
}

type MockAssistantAgentOptions = {
  echoOnSubmit?: boolean;
  isRequesting?: boolean;
  messages?: AssistantMessageRecord[];
};

function useMockAssistantAgent({
  echoOnSubmit = false,
  isRequesting = false,
  messages = [],
}: MockAssistantAgentOptions = {}) {
  const [visibleMessages, setVisibleMessages] = useState(messages);

  return {
    abort: vi.fn<() => void>(),
    activeConversation: 'session-1',
    eventsConnected: true,
    handleSubmit: async (value: string) => {
      mockHandleSubmit(value);
      if (echoOnSubmit) {
        setVisibleMessages((current) => [
          ...current,
          createMockMessage({
            id: 'echo-user-message',
            message: {
              content: value,
              role: 'user',
            },
          }),
        ]);
      }
    },
    isHistoryLoading: false,
    isRequesting,
    messages: visibleMessages,
    editAndResend: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    pendingApprovals: [],
    regenerateResponse: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    retryLastResponse: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    submitApproval: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  };
}

function installAssistantAgentMock(options: MockAssistantAgentOptions = {}) {
  mockUseAssistantAgent.mockImplementation(function useAssistantAgentMock() {
    return useMockAssistantAgent(options);
  });
}

describe('assistant core flow e2e', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCatalogState.isLoading = false;
    mockMutationState.isPending = false;
    installAssistantAgentMock();
  });

  it('renders the submitted user message path before slow session label work completes', async () => {
    let labelDone = false;
    let resolveLabel!: (value: boolean) => void;

    mockUpdateSessionLabel.mockImplementation(
      () =>
        new Promise<boolean>((resolve) => {
          resolveLabel = (value) => {
            labelDone = true;
            resolve(value);
          };
        }),
    );
    installAssistantAgentMock({ echoOnSubmit: true });

    await renderDesktopScene(<AssistantPage />, { route: '/' });

    const composer = page.getByTestId('assistant-composer-input').getByRole('textbox');
    await composer.fill('Investigate failing desktop tests');
    await page.getByTestId('assistant-send-button').getByRole('button').click();

    await expect.element(page.getByTestId('assistant-message-echo-user-message')).toHaveTextContent(
      'Investigate failing desktop tests',
    );
    expect(labelDone).toBe(false);
    expect(mockHandleSubmit).toHaveBeenCalledWith('Investigate failing desktop tests');
    expect(mockUpdateSessionLabel).toHaveBeenCalledWith(
      'session-1',
      'Investigate failing desktop tests',
    );

    resolveLabel(true);
  });

  it('inserts the web search slash command from the helper control', async () => {
    await renderDesktopScene(<AssistantPage />, { route: '/' });

    const composer = page.getByTestId('assistant-composer-input').getByRole('textbox');
    await page.getByTestId('assistant-web-search-toggle').click();

    await expect.element(composer).toHaveValue('/web_search ');
  });

  it('shows the model loading system bubble after the user message', async () => {
    mockMutationState.isPending = true;
    installAssistantAgentMock({
      messages: [
        createMockMessage({
          id: 'user-before-model-load',
          message: {
            content: 'Use the local model for this answer',
            role: 'user',
          },
        }),
      ],
    });

    await renderDesktopScene(<AssistantPage />, { route: '/' });

    await expect.element(page.getByTestId('assistant-message-user-before-model-load')).toHaveTextContent(
      'Use the local model for this answer',
    );
    await expect.element(page.getByTestId('assistant-model-loading')).toBeVisible();

    const sceneText = document.querySelector('[data-testid="desktop-browser-scene"]')?.textContent ?? '';
    expect(sceneText.indexOf('Use the local model for this answer')).toBeLessThan(
      sceneText.indexOf('Loading model...'),
    );
  });

  it('renders thinking content in the thought chain without duplicating it in the answer body', async () => {
    installAssistantAgentMock({
      messages: [
        createMockMessage({
          id: 'assistant-with-thinking',
          message: {
            content: '<think>plan-only-thinking</think>\n\nFinal answer body',
            role: 'assistant',
          },
          status: 'success',
        }),
      ],
    });

    await renderDesktopScene(<AssistantPage />, { route: '/' });

    await expect.element(page.getByTestId('assistant-message-assistant-with-thinking')).toHaveTextContent(
      'Final answer body',
    );
    await expect.element(page.getByTestId('assistant-thinking-assistant-with-thinking')).toHaveTextContent(
      'plan-only-thinking',
    );

    const sceneText = document.querySelector('[data-testid="desktop-browser-scene"]')?.textContent ?? '';
    expect(sceneText.match(/plan-only-thinking/g)).toHaveLength(1);
  });

  it('keeps long tool JSON inside the assistant scene width', async () => {
    const toolJson =
      '{"entries":[{"name":"ipc","is_dir":true,"size_bytes":0,"modified":1775143656},{"name":"runtime","is_dir":true,"size_bytes":0,"modified":1775833862},{"name":"slab-agent-session-1fe184a9-3485-4f1b-b914-bd934d763f60-2026-6-6.log","is_dir":false,"size_bytes":1369404,"modified":1780744212},{"name":"slab-app.log","is_dir":false,"size_bytes":23786,"modified":1780745608},{"name":"slab-server.log","is_dir":false,"size_bytes":919059921,"modified":1780745609}]}';

    installAssistantAgentMock({
      messages: [
        createMockMessage({
          id: 'tool-json-message',
          message: {
            content: 'Tool result received.',
            role: 'assistant',
            thoughts: [
              {
                callId: 'call-1',
                detail: toolJson,
                id: 'call-1',
                status: 'success',
                title: 'tool_call',
                toolName: 'list_files',
              },
            ],
          },
          status: 'success',
        }),
      ],
    });

    await renderDesktopScene(<AssistantPage />, { route: '/' });

    await expect.element(page.getByTestId('assistant-thought-call-1')).toHaveTextContent(
      'slab-server.log',
    );
    const sceneText = document.querySelector('[data-testid="desktop-browser-scene"]')?.textContent ?? '';
    expect(sceneText).toContain('"entries": [');
    expect(sceneText).not.toContain('{"entries":[{"name":"ipc"');

    await vi.waitFor(() => {
      const scene = document.querySelector('[data-testid="desktop-browser-scene"]') as HTMLElement | null;
      if (!scene) {
        throw new Error('Desktop browser scene is missing');
      }
      expect(scene.scrollWidth).toBeLessThanOrEqual(scene.clientWidth + 1);
    });
  });
});
