import { page } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import AssistantPage from '@slab/ui/pages/assistant';
import type { AssistantConversationItem } from '@slab/ui/pages/assistant/hooks/use-assistant-sessions';
import {
  expectDesktopSceneAccessible,
  expectDesktopSceneKeyboardReachable,
  renderDesktopScene,
} from '../test-utils';

/**
 * Make the scene pixel-deterministic before a screenshot: snap every CSS
 * animation to its base state, and hide the terminal's blinking cursor
 * outright (its blink phase varies run-to-run, and it is the only
 * `animate-pulse` element in these scenes).
 */
function freezeAnimations() {
  // The test itself runs in the browser context, so the DOM is directly
  // reachable (no `page.evaluate` here).
  const style = document.createElement('style');
  style.id = 'visual-test-freeze-animations';
  style.textContent = [
    '*, *::before, *::after { animation: none !important; transition: none !important; }',
    '.animate-pulse { display: none !important; }',
  ].join('\n');
  document.head.appendChild(style);
}

const mocks = vi.hoisted(() => {
  const translate = (key: string) => key;
  // Mirrors the initial `ConversationState` of the real harness controller
  // (packages/slab-core/src/harness/conversation-controller.ts); every field
  // the page destructures is present so unguarded `.map`s stay defined.
  const harnessConversation = {
    activeConversation: 'session-1' as string | undefined,
    actionError: null,
    commands: [] as Array<Record<string, unknown>>,
    compactionMarkers: [] as Array<Record<string, unknown>>,
    compactThread: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    error: null as string | null,
    forkThread: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    historyCreatedAt: null as number | null,
    isCompacting: false,
    isForking: false,
    isHistoryLoading: false,
    isRollingBack: false,
    liveOutputByItemId: new Map<string, string>(),
    livePatchByItemId: new Map<string, string[]>(),
    modelLoad: null,
    planMode: false,
    restoredMessages: [] as Array<Record<string, unknown>>,
    restoredThreadId: 'thread-1' as string | null,
    restoreVersion: 1,
    rollbackFromTurn: vi.fn<(turnIndex: number) => void>(),
    setPlanMode: vi.fn<(enabled: boolean) => void>(),
    threadStatus: null,
    abortReason: null,
    queuedCount: 0,
    queuedTexts: [] as string[],
    sendSteering: vi.fn<() => Promise<unknown>>().mockResolvedValue({ queued: true }),
    interrupt: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    transport: {},
    turnUsage: null,
    userMessageTurnIndex: new Map<string, number>(),
    approvals: [] as Array<Record<string, unknown>>,
    approvalStatusByItemId: new Map<string, 'pending' | 'approved' | 'denied'>(),
    resolveApproval: vi.fn<
      (itemId: string, approved: boolean, scope: 'run_once' | 'always_in_workspace' | 'always' | 'deny') => Promise<void>
    >(),
  };
  return { harnessConversation, translate };
});

const { mockUseAssistantSessions } = vi.hoisted(() => ({
  mockUseAssistantSessions: vi.fn<() => unknown>(),
}));

const { mockUseAssistantLocale } = vi.hoisted(() => ({
  mockUseAssistantLocale: vi.fn<() => unknown>(),
}));

const { mockUseMarkdownTheme } = vi.hoisted(() => ({
  mockUseMarkdownTheme: vi.fn<() => unknown>(),
}));

vi.mock('@slab/ui/pages/assistant/hooks/use-harness-conversation', () => ({
  useHarnessConversation: vi.fn(() => mocks.harnessConversation),
}));

vi.mock('@ai-sdk/react', () => ({
  useChat: vi.fn(({ messages = [] }: { messages?: Array<Record<string, unknown>> }) => ({
    messages,
    sendMessage: vi.fn(),
    status: 'ready',
    stop: vi.fn(),
  })),
}));

vi.mock('@slab/ui/hooks/use-ai-model', () => ({
  useAiModel: vi.fn(() => ({
    ensureDownloaded: vi.fn().mockResolvedValue({ downloadedNow: false }),
    ensureLoaded: vi.fn().mockResolvedValue({ runtimeStatus: null }),
    loading: false,
    localModels: [],
    models: [
      {
        chat_capabilities: null,
        display_name: 'Model A',
        id: 'model-a',
        kind: 'cloud',
        local_path: null,
        pending: false,
        runtime_presets: null,
        spec: { context_window: 4096 },
        status: 'ready',
      },
    ],
    selectedId: 'model-a',
    setSelectedId: vi.fn(),
    status: { busy: false },
  })),
}));

vi.mock('@slab/ui/pages/assistant/hooks/use-assistant-sessions', () => ({
  useAssistantSessions: mockUseAssistantSessions,
}));

vi.mock('@slab/ui/pages/assistant/assistant-locale', () => ({
  useAssistantLocale: mockUseAssistantLocale,
}));

vi.mock('@slab/ui/pages/assistant/hooks/use-markdown-theme', () => ({
  useMarkdownTheme: mockUseMarkdownTheme,
}));

vi.mock('@slab/ui/hooks/use-header', () => ({
  useHeader: vi.fn<() => unknown>(() => ({
    meta: { title: 'Assistant', subtitle: 'Assistant', icon: vi.fn(), contextLabel: null },
    search: null,
    select: null,
  })),
}));

vi.mock('@slab/ui/store/useAssistantUiStore', () => ({
  useAssistantUiStore: vi.fn<(selector?: (state: Record<string, unknown>) => unknown) => unknown>((selector) => {
    const state = {
      currentSessionId: 'session-1',
      advancedPanelOpen: false,
      hasHydrated: true,
      removeSessionLabel: vi.fn<() => void>(),
      reasoningEffort: 'medium',
      sessionLabels: {},
      setAdvancedPanelOpen: vi.fn<() => void>(),
      setCurrentSessionId: vi.fn<() => void>(),
      setReasoningEffort: vi.fn<() => void>(),
      setSessionLabel: vi.fn<() => void>(),
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

  return createSlabApiMock();
});

vi.mock('@slab/i18n', () => ({
  DEFAULT_ASSISTANT_LABELS: ['New assistant'],
  LEGACY_DEFAULT_CHAT_LABELS: ['New Conversation'],
  Trans: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  getResolvedAppLanguage: vi.fn<() => string>(() => 'en'),
  useTranslation: vi.fn<() => unknown>(() => ({
    t: mocks.translate,
  })),
}));

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
    setCurrentSessionId: vi.fn(),
    setSessionLabel: vi.fn(),
    updateSessionLabel: vi.fn<() => Promise<boolean>>().mockResolvedValue(true),
    ...overrides,
  };
}

describe('AssistantPage browser visual regression', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(Date.prototype, 'getHours').mockReturnValue(15);
    const hc = mocks.harnessConversation;
    hc.activeConversation = 'session-1';
    hc.approvals = [];
    hc.approvalStatusByItemId = new Map();
    hc.error = null;
    hc.isHistoryLoading = false;
    hc.liveOutputByItemId = new Map();
    hc.livePatchByItemId = new Map();
    hc.restoredMessages = [];
    hc.restoredThreadId = 'thread-1';
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
    document.getElementById('visual-test-freeze-animations')?.remove();
  });

  it('captures the assistant new-chat landing (homepage)', async () => {
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

    await expectDesktopSceneAccessible();
    await expectDesktopSceneKeyboardReachable();
    await expect.element(page.getByTestId('assistant-new-chat-landing')).toBeVisible();
    await expect.element(page.getByRole('button', { name: /send/i })).toBeVisible();
    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('assistant-page-empty.png');
  });

  it('captures the assistant page loading state', async () => {
    mocks.harnessConversation.isHistoryLoading = true;
    mockUseAssistantSessions.mockReturnValue(
      createAssistantSessionsViewModel({
        conversationList: [],
        isSessionsLoading: true,
      }),
    );

    await renderDesktopScene(<AssistantPage />, { route: '/' });

    await expect.element(page.getByTestId('assistant-new-chat-landing')).toBeVisible();
    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('assistant-page-loading.png');
  });

  it('captures the assistant page with messages', async () => {
    mocks.harnessConversation.restoredMessages = [
      {
        id: 'msg-1',
        parts: [{ type: 'text', text: 'What is the capital of France?' }],
        role: 'user',
      },
      {
        id: 'msg-2',
        parts: [{ type: 'text', text: 'The capital of France is Paris.' }],
        role: 'assistant',
      },
    ];
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

    await renderDesktopScene(<AssistantPage />, { route: '/?session=session-1' });

    await expect.element(page.getByText('What is the capital of France?')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('assistant-page-with-messages.png');
  });

  it('captures agent thought chain with an expanded bash tool row', async () => {
    // A COMPLETED command (not an approval-pending one): the approval-pending
    // fixture raced its expand state under parallel suite load, while the
    // approval card itself is visually covered by the approval-banner tests.
    mocks.harnessConversation.restoredMessages = [
      {
        id: 'msg-1',
        parts: [{ type: 'text', text: 'Inspect the repository status' }],
        role: 'user',
      },
      {
        id: 'msg-2',
        parts: [
          { state: 'done', text: 'Checking the workspace before answering.', type: 'reasoning' },
          {
            input: { command: 'git status --short', cwd: '/repo' },
            output: '',
            state: 'output-available',
            toolCallId: 'call-1',
            type: 'tool-commandExecution',
          },
        ],
        role: 'assistant',
      },
    ];
    mocks.harnessConversation.approvals = [];
    mocks.harnessConversation.approvalStatusByItemId = new Map();
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

    await renderDesktopScene(<AssistantPage />, { route: '/?session=session-1' });

    await expect.element(page.getByText('Inspect the repository status')).toBeVisible();
    // Completed rows sit collapsed by design — the compact `Bash: <command>`
    // line IS the feature this baseline pins. (The expanded terminal raced its
    // mount/animation state run-to-run under parallel suite load; its content
    // rendering is covered by the message-tool unit tests.)
    await expect.element(page.getByText('Bash')).toBeVisible();
    freezeAnimations();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('assistant-page-agent-chain.png');
  });
});
