import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import AssistantPage from '@/pages/assistant';
import { renderDesktopScene } from '../test-utils';

const {
  mockHandleSubmit,
  mockSetDeepThink,
  mockUpdateSessionLabel,
} = vi.hoisted(() => ({
  mockHandleSubmit: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockSetDeepThink: vi.fn<() => void>(),
  mockUpdateSessionLabel: vi.fn<() => Promise<boolean>>().mockResolvedValue(true),
}));

vi.mock('@/pages/assistant/hooks/use-assistant-agent', () => ({
  useAssistantAgent: vi.fn<() => unknown>(() => ({
    abort: vi.fn<() => void>(),
    activeConversation: 'session-1',
    eventsConnected: true,
    handleSubmit: mockHandleSubmit,
    isHistoryLoading: false,
    isRequesting: false,
    messages: [],
    submitApproval: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  })),
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
      deepThink: false,
      setDeepThink: mockSetDeepThink,
    };
    return selector ? selector(state) : state;
  }),
}));

vi.mock('@slab/api', () => ({
  default: {
    useMutation: vi.fn<() => unknown>(() => ({
      isPending: false,
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
      isLoading: false,
      refetch: vi.fn<() => Promise<{ data: unknown[] }>>().mockResolvedValue({ data: [] }),
    })),
  },
}));

vi.mock('@slab/api/models', () => ({
  toCatalogModelList: vi.fn<(data: unknown) => unknown[]>((data) => (Array.isArray(data) ? data : [])),
}));

describe('assistant core flow e2e', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('submits a prompt and persists the first useful session label', async () => {
    await renderDesktopScene(<AssistantPage />, { route: '/' });

    const composer = page.getByPlaceholder('Type a message or drop files...');
    await composer.fill('Investigate failing desktop tests');
    await page.getByRole('button', { name: 'Send message' }).click();

    await vi.waitFor(() => {
      expect(mockUpdateSessionLabel).toHaveBeenCalledWith(
        'session-1',
        'Investigate failing desktop tests',
      );
      expect(mockHandleSubmit).toHaveBeenCalledWith('Investigate failing desktop tests');
    });
  });

  it('inserts the web search slash command from the helper control', async () => {
    await renderDesktopScene(<AssistantPage />, { route: '/' });

    const composer = page.getByPlaceholder('Type a message or drop files...');
    await page.getByRole('button', { name: 'Web search' }).click();

    await expect.element(composer).toHaveValue('/web_search ');
  });
});
