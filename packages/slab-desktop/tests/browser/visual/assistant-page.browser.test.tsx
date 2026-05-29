import { page } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import AssistantPage from '@/pages/assistant';
import type { AssistantMessageRecord } from '@/pages/assistant/assistant-context';
import type { AssistantConversationItem } from '@/pages/assistant/hooks/use-assistant-sessions';
import { renderDesktopScene } from '../test-utils';

const { mockUseAssistantAgent } = vi.hoisted(() => ({
  mockUseAssistantAgent: vi.fn<() => unknown>(),
}));

const { mockUseAssistantSessions } = vi.hoisted(() => ({
  mockUseAssistantSessions: vi.fn<() => unknown>(),
}));

const { mockUseAssistantLocale } = vi.hoisted(() => ({
  mockUseAssistantLocale: vi.fn<() => unknown>(),
}));

const { mockUseMarkdownTheme } = vi.hoisted(() => ({
  mockUseMarkdownTheme: vi.fn<() => unknown>(),
}));

vi.mock('@/pages/assistant/hooks/use-assistant-agent', () => ({
  useAssistantAgent: mockUseAssistantAgent,
}));

vi.mock('@/pages/assistant/hooks/use-assistant-sessions', () => ({
  useAssistantSessions: mockUseAssistantSessions,
}));

vi.mock('@/pages/assistant/assistant-locale', () => ({
  useAssistantLocale: mockUseAssistantLocale,
}));

vi.mock('@/pages/assistant/hooks/use-markdown-theme', () => ({
  useMarkdownTheme: mockUseMarkdownTheme,
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderControl: vi.fn<() => void>(),
}));

vi.mock('@/hooks/use-persisted-header-select', () => ({
  usePersistedHeaderSelect: vi.fn<() => unknown>(() => ({
    value: 'model-1',
    setValue: vi.fn<() => void>(),
  })),
}));

vi.mock('@/store/useAssistantUiStore', () => ({
  useAssistantUiStore: vi.fn<(selector?: (state: Record<string, unknown>) => unknown) => unknown>((selector) => {
    const state = {
      currentSessionId: 'session-1',
      deepThink: false,
      hasHydrated: true,
      removeSessionLabel: vi.fn<() => void>(),
      sessionLabels: {},
      setCurrentSessionId: vi.fn<() => void>(),
      setDeepThink: vi.fn<() => void>(),
      setSessionLabel: vi.fn<() => void>(),
    };
    return selector ? selector(state) : state;
  }),
}));

vi.mock('@slab/api', () => ({
  apiClient: {
    DELETE: vi.fn<(...args: unknown[]) => unknown>(),
    GET: vi.fn<(...args: unknown[]) => unknown>(),
    POST: vi.fn<(...args: unknown[]) => unknown>(),
    PUT: vi.fn<(...args: unknown[]) => unknown>(),
  },
  default: {
    useMutation: vi.fn<() => unknown>(() => ({
      isPending: false,
      mutateAsync: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    })),
    useQuery: vi.fn<() => unknown>(() => ({
      data: null,
      isLoading: false,
      refetch: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    })),
  },
  getErrorMessage: vi.fn<(err: Error) => string>((err) => err.message),
  isApiError: vi.fn<() => boolean>(() => false),
  queryClient: {},
}));

vi.mock('@slab/i18n', () => ({
  DEFAULT_ASSISTANT_LABELS: ['New assistant'],
  LEGACY_DEFAULT_CHAT_LABELS: ['New Conversation'],
  Trans: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  getResolvedAppLanguage: vi.fn<() => string>(() => 'en'),
  useTranslation: vi.fn<() => unknown>(() => ({
    t: vi.fn<(key: string) => string>((key) => key),
  })),
}));

const createVoidMock = () => vi.fn<(...args: unknown[]) => void>();
const createAsyncVoidMock = () =>
  vi.fn<(...args: unknown[]) => Promise<void>>().mockResolvedValue(undefined);

function createMockMessage(
  overrides: Partial<AssistantMessageRecord> = {},
): AssistantMessageRecord {
  return {
    id: 'msg-1',
    message: {
      content: 'Hello, how are you?',
      role: 'user',
    },
    status: 'success',
    ...overrides,
  };
}

function createAssistantAgentViewModel(overrides = {}) {
  return {
    abort: createVoidMock(),
    activeConversation: 'session-1',
    eventsConnected: false,
    handleSubmit: createAsyncVoidMock(),
    isHistoryLoading: false,
    isRequesting: false,
    messages: [] as AssistantMessageRecord[],
    submitApproval: createAsyncVoidMock(),
    ...overrides,
  };
}

function createAssistantSessionsViewModel(overrides = {}) {
  return {
    conversationList: [] as AssistantConversationItem[],
    createSession: vi.fn<() => Promise<{ id: string }>>().mockResolvedValue({ id: 'session-new' }),
    currentSessionId: 'session-1',
    deleteSession: vi.fn<() => Promise<boolean>>().mockResolvedValue(true),
    isCreatingSession: false,
    isDeletingSession: false,
    isSessionMutating: false,
    isSessionsLoading: false,
    setCurrentSessionId: createVoidMock(),
    setSessionLabel: createVoidMock(),
    updateSessionLabel: vi.fn<() => Promise<boolean>>().mockResolvedValue(true),
    ...overrides,
  };
}

describe('AssistantPage browser visual regression', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(Date.prototype, 'getHours').mockReturnValue(15);
    mockUseAssistantLocale.mockReturnValue({
      approvalFailed: 'Approval failed',
      approvalNotDelivered: 'Approval not delivered',
      eventStreamLagged: 'Lagged',
      interruptFailed: 'Interrupt failed',
      noData: 'No data',
      requestAborted: 'Request aborted',
      requestFailed: 'Request failed',
    });
    mockUseMarkdownTheme.mockReturnValue(['markdown-theme-dark']);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('captures the assistant page empty state', async () => {
    mockUseAssistantAgent.mockReturnValue(createAssistantAgentViewModel());
    mockUseAssistantSessions.mockReturnValue(
      createAssistantSessionsViewModel({
        conversationList: [
          {
            group: 'workspace',
            key: 'session-1',
            label: 'New assistant',
          },
        ],
      }),
    );

    await renderDesktopScene(<AssistantPage />, { route: '/' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('assistant-page-empty.png');
  });

  it('captures the assistant page loading state', async () => {
    mockUseAssistantAgent.mockReturnValue(
      createAssistantAgentViewModel({
        isHistoryLoading: true,
        messages: [],
      }),
    );
    mockUseAssistantSessions.mockReturnValue(
      createAssistantSessionsViewModel({
        conversationList: [],
        isSessionsLoading: true,
      }),
    );

    await renderDesktopScene(<AssistantPage />, { route: '/' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('assistant-page-loading.png');
  });

  it('captures the assistant page with messages', async () => {
    const mockMessages: AssistantMessageRecord[] = [
      createMockMessage({
        id: 'msg-1',
        message: {
          content: 'What is the capital of France?',
          role: 'user',
        },
        status: 'success',
      }),
      createMockMessage({
        id: 'msg-2',
        message: {
          content: 'The capital of France is Paris.',
          role: 'assistant',
        },
        status: 'success',
      }),
    ];

    mockUseAssistantAgent.mockReturnValue(
      createAssistantAgentViewModel({
        messages: mockMessages,
      }),
    );
    mockUseAssistantSessions.mockReturnValue(
      createAssistantSessionsViewModel({
        conversationList: [
          {
            group: 'workspace',
            key: 'session-1',
            label: 'France Discussion',
          },
        ],
      }),
    );

    await renderDesktopScene(<AssistantPage />, { route: '/' });

    await expect.element(page.getByText('What is the capital of France?')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('assistant-page-with-messages.png');
  });

  it('captures active agent thought chain and approval', async () => {
    const mockMessages: AssistantMessageRecord[] = [
      createMockMessage({
        id: 'msg-1',
        message: {
          content: 'Inspect the repository status',
          role: 'user',
        },
        status: 'success',
      }),
      createMockMessage({
        id: 'msg-2',
        message: {
          content: '<think>Checking the workspace before answering.</think>',
          role: 'assistant',
          thoughts: [
            {
              callId: 'call-1',
              detail: 'git status --short',
              id: 'call-1',
              pendingApproval: {
                callId: 'call-1',
                command: 'git status --short',
                toolName: 'shell',
              },
              status: 'loading',
              title: 'shell approval',
              toolName: 'shell',
            },
          ],
        },
        status: 'loading',
      }),
    ];

    mockUseAssistantAgent.mockReturnValue(
      createAssistantAgentViewModel({
        isRequesting: true,
        messages: mockMessages,
      }),
    );
    mockUseAssistantSessions.mockReturnValue(
      createAssistantSessionsViewModel({
        conversationList: [
          {
            group: 'workspace',
            key: 'session-1',
            label: 'Agent Run',
          },
        ],
      }),
    );

    await renderDesktopScene(<AssistantPage />, { route: '/' });

    await expect.element(page.getByText('Inspect the repository status')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('assistant-page-agent-chain.png');
  });
});
