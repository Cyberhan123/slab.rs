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
  usePersistedHeaderSelect: vi.fn(() => ({ value: 'model-1', setValue: vi.fn() })),
}));

vi.mock('@/store/useChatUiStore', () => ({
  useChatUiStore: vi.fn((selector?: (state: Record<string, unknown>) => unknown) => {
    const state = {
      deepThink: false,
      setDeepThink: vi.fn(),
      hasHydrated: true,
      currentSessionId: 'session-1',
      setCurrentSessionId: vi.fn(),
      sessionLabels: {},
      setSessionLabel: vi.fn(),
      removeSessionLabel: vi.fn(),
    };
    return selector ? selector(state) : state;
  }),
}));

vi.mock('@/lib/api', () => ({
  apiClient: {
    GET: vi.fn(),
    POST: vi.fn(),
    PUT: vi.fn(),
    DELETE: vi.fn(),
  },
  default: {
    useQuery: vi.fn(() => ({
      data: null,
      isLoading: false,
      refetch: vi.fn().mockResolvedValue(undefined),
    })),
    useMutation: vi.fn(() => ({
      isPending: false,
      mutateAsync: vi.fn().mockResolvedValue(undefined),
    })),
  },
  getErrorMessage: vi.fn((err: Error) => err.message),
  isApiError: vi.fn(() => false),
  queryClient: {},
}));

vi.mock('@slab/i18n', () => ({
  useTranslation: vi.fn(() => ({
    t: vi.fn((key: string) => key),
  })),
  getResolvedAppLanguage: vi.fn(() => 'en'),
  DEFAULT_CHAT_LABELS: ['New Chat'],
  LEGACY_DEFAULT_CHAT_LABELS: ['New Conversation'],
  Trans: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

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
    abort: vi.fn(),
    onReload: vi.fn(),
    onContinue: vi.fn(),
    activeConversation: 'session-1',
    setActiveConversation: vi.fn(),
    handleSubmit: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function createChatSessionsViewModel(overrides = {}) {
  return {
    conversationList: [] as ChatConversationItem[],
    createSession: vi.fn().mockResolvedValue({ id: 'session-new' }),
    currentSessionId: 'session-1',
    isCreatingSession: false,
    isDeletingSession: false,
    isSessionMutating: false,
    isSessionsLoading: false,
    setCurrentSessionId: vi.fn(),
    setSessionLabel: vi.fn(),
    deleteSession: vi.fn().mockResolvedValue(true),
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
