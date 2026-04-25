import { page } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import ChatPage from '@/pages/chat';
import type { ChatMessageRecord } from '@/pages/chat/chat-context';
import type { ChatConversationItem } from '@/pages/chat/hooks/use-chat-sessions';
import { renderDesktopScene } from '../test-utils';

const { mockUseChat } = vi.hoisted(() => ({
  mockUseChat: vi.fn<() => unknown>(),
}));

const { mockUseChatSessions } = vi.hoisted(() => ({
  mockUseChatSessions: vi.fn<() => unknown>(),
}));

const { mockUseChatLocale } = vi.hoisted(() => ({
  mockUseChatLocale: vi.fn<() => unknown>(),
}));

const { mockUseMarkdownTheme } = vi.hoisted(() => ({
  mockUseMarkdownTheme: vi.fn<() => unknown>(),
}));

vi.mock('@/pages/chat/hooks/use-chat', () => ({
  useChat: mockUseChat,
}));

vi.mock('@/pages/chat/hooks/use-chat-sessions', () => ({
  useChatSessions: mockUseChatSessions,
}));

vi.mock('@/pages/chat/chat-locale', () => ({
  useChatLocale: mockUseChatLocale,
}));

vi.mock('@/pages/chat/hooks/use-markdowm-theme', () => ({
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

vi.mock('@/store/useChatUiStore', () => ({
  useChatUiStore: vi.fn<(selector?: (state: Record<string, unknown>) => unknown) => unknown>((selector) => {
    const state = {
      deepThink: false,
      setDeepThink: vi.fn<() => void>(),
      hasHydrated: true,
      currentSessionId: 'session-1',
      setCurrentSessionId: vi.fn<() => void>(),
      sessionLabels: {},
      setSessionLabel: vi.fn<() => void>(),
      removeSessionLabel: vi.fn<() => void>(),
    };
    return selector ? selector(state) : state;
  }),
}));

vi.mock('@slab/api', () => ({
  apiClient: {
    GET: vi.fn<(...args: unknown[]) => unknown>(),
    POST: vi.fn<(...args: unknown[]) => unknown>(),
    PUT: vi.fn<(...args: unknown[]) => unknown>(),
    DELETE: vi.fn<(...args: unknown[]) => unknown>(),
  },
  default: {
    useQuery: vi.fn<() => unknown>(() => ({
      data: null,
      isLoading: false,
      refetch: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    })),
    useMutation: vi.fn<() => unknown>(() => ({
      isPending: false,
      mutateAsync: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    })),
  },
  getErrorMessage: vi.fn<(err: Error) => string>((err) => err.message),
  isApiError: vi.fn<() => boolean>(() => false),
  queryClient: {},
}));

vi.mock('@slab/i18n', () => ({
  useTranslation: vi.fn<() => unknown>(() => ({
    t: vi.fn<(key: string) => string>((key) => key),
  })),
  getResolvedAppLanguage: vi.fn<() => string>(() => 'en'),
  DEFAULT_CHAT_LABELS: ['New Chat'],
  LEGACY_DEFAULT_CHAT_LABELS: ['New Conversation'],
  Trans: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

const createVoidMock = () => vi.fn<(...args: unknown[]) => void>();
const createAsyncVoidMock = () =>
  vi.fn<(...args: unknown[]) => Promise<void>>().mockResolvedValue(undefined);

function createMockMessage(overrides: Partial<ChatMessageRecord> = {}): ChatMessageRecord {
  return {
    id: 'msg-1',
    message: {
      role: 'user',
      content: 'Hello, how are you?',
    },
    status: 'success',
    ...overrides,
  };
}

function createChatViewModel(overrides = {}) {
  return {
    messages: [] as ChatMessageRecord[],
    isRequesting: false,
    isHistoryLoading: false,
    abort: createVoidMock(),
    onReload: createVoidMock(),
    onContinue: createVoidMock(),
    activeConversation: 'session-1',
    setActiveConversation: createVoidMock(),
    handleSubmit: createAsyncVoidMock(),
    ...overrides,
  };
}

function createChatSessionsViewModel(overrides = {}) {
  return {
    conversationList: [] as ChatConversationItem[],
    createSession: vi.fn<() => Promise<{ id: string }>>().mockResolvedValue({ id: 'session-new' }),
    currentSessionId: 'session-1',
    isCreatingSession: false,
    isDeletingSession: false,
    isSessionMutating: false,
    isSessionsLoading: false,
    setCurrentSessionId: createVoidMock(),
    setSessionLabel: createVoidMock(),
    deleteSession: vi.fn<() => Promise<boolean>>().mockResolvedValue(true),
    ...overrides,
  };
}

describe('ChatPage browser visual regression', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(Date.prototype, 'getHours').mockReturnValue(15);
    mockUseChatLocale.mockReturnValue({
      requestFailed: 'Request failed',
      requestAborted: 'Request aborted',
      noData: 'No data',
      regenerate: 'Regenerate',
      continueGenerating: 'Continue generating',
      copy: 'Copy',
      copied: 'Copied',
    });
    mockUseMarkdownTheme.mockReturnValue(['markdown-theme-dark']);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('captures the chat page empty state', async () => {
    mockUseChat.mockReturnValue(createChatViewModel());
    mockUseChatSessions.mockReturnValue(
      createChatSessionsViewModel({
        conversationList: [
          {
            key: 'session-1',
            label: 'New Chat',
            group: 'workspace',
          },
        ],
      }),
    );

    await renderDesktopScene(<ChatPage />, { route: '/chat' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('chat-page-empty.png');
  });

  it('captures the chat page loading state', async () => {
    mockUseChat.mockReturnValue(
      createChatViewModel({
        isHistoryLoading: true,
        messages: [],
      }),
    );
    mockUseChatSessions.mockReturnValue(
      createChatSessionsViewModel({
        isSessionsLoading: true,
        conversationList: [],
      }),
    );

    await renderDesktopScene(<ChatPage />, { route: '/chat' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('chat-page-loading.png');
  });

  it('captures the chat page with messages', async () => {
    const mockMessages: ChatMessageRecord[] = [
      createMockMessage({
        id: 'msg-1',
        message: {
          role: 'user',
          content: 'What is the capital of France?',
        },
        status: 'success',
      }),
      createMockMessage({
        id: 'msg-2',
        message: {
          role: 'assistant',
          content: 'The capital of France is Paris.',
        },
        status: 'success',
      }),
    ];

    mockUseChat.mockReturnValue(
      createChatViewModel({
        messages: mockMessages,
      }),
    );
    mockUseChatSessions.mockReturnValue(
      createChatSessionsViewModel({
        conversationList: [
          {
            key: 'session-1',
            label: 'France Discussion',
            group: 'workspace',
          },
        ],
      }),
    );

    await renderDesktopScene(<ChatPage />, { route: '/chat' });

    await expect.element(page.getByText('What is the capital of France?')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('chat-page-with-messages.png');
  });

  it('captures the chat page requesting state', async () => {
    const mockMessages: ChatMessageRecord[] = [
      createMockMessage({
        id: 'msg-1',
        message: {
          role: 'user',
          content: 'Tell me a story',
        },
        status: 'success',
      }),
      createMockMessage({
        id: 'msg-2',
        message: {
          role: 'assistant',
          content: '',
        },
        status: 'loading',
      }),
    ];

    mockUseChat.mockReturnValue(
      createChatViewModel({
        messages: mockMessages,
        isRequesting: true,
      }),
    );
    mockUseChatSessions.mockReturnValue(
      createChatSessionsViewModel({
        conversationList: [
          {
            key: 'session-1',
            label: 'Story Time',
            group: 'workspace',
          },
        ],
      }),
    );

    await renderDesktopScene(<ChatPage />, { route: '/chat' });

    await expect.element(page.getByText('Tell me a story')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('chat-page-requesting.png');
  });
});
