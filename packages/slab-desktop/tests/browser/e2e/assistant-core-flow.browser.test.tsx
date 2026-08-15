import type { UIMessage } from 'ai';
import type { ReactNode } from 'react';
import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import AssistantPage from '@slab/ui/pages/assistant';
import { renderDesktopScene } from '../test-utils';

/**
 * Browser smoke for the refactored assistant page (harness control plane +
 * `useHarnessConversation`). The pre-refactor version of this file mocked the
 * deleted `useAssistantAgent` hook and asserted on removed `assistant-message-*` /
 * `assistant-thinking-*` test ids; it is rewritten here to render the real page
 * in chromium and verify the new contract: the composer mounts, restored
 * messages render, and a pending command approval surfaces its card.
 */

const mocks = vi.hoisted(() => {
  const translate = (key: string) => key;
  const harnessConversation = {
    activeConversation: 'session-a' as string | undefined,
    error: null as string | null,
    isHistoryLoading: false,
    restoredMessages: [] as UIMessage[],
    restoredThreadId: 'thread-a' as string | null,
    restoreVersion: 1,
    transport: {},
    approvals: [] as Array<Record<string, unknown>>,
    approvalStatusByItemId: new Map<string, 'pending' | 'approved' | 'denied'>(),
    liveOutputByItemId: new Map<string, string>(),
    resolveApproval: vi.fn<
      (itemId: string, approved: boolean, scope: 'run_once' | 'always_in_workspace' | 'always' | 'deny') => Promise<void>
    >(),
  };
  return {
    harnessConversation,
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
    sendMessage: vi.fn(),
    stop: vi.fn(),
    setSelectedModelId: vi.fn(),
    setCurrentSessionId: vi.fn(),
    translate,
  };
});

vi.mock('@ai-sdk/react', () => ({
  useChat: vi.fn(({ messages = [] }: { messages?: UIMessage[] }) => ({
    messages,
    sendMessage: mocks.sendMessage,
    status: 'ready',
    stop: mocks.stop,
  })),
}));

vi.mock('@slab/ui/pages/assistant/hooks/use-harness-conversation', () => ({
  useHarnessConversation: vi.fn(() => mocks.harnessConversation),
}));

vi.mock('@slab/i18n', () => ({
  DEFAULT_ASSISTANT_LABELS: ['pages.assistant.runtime.newChat'],
  LEGACY_DEFAULT_CHAT_LABELS: ['New Chat'],
  getResolvedAppLanguage: () => 'en-US',
  Trans: ({ i18nKey }: { i18nKey: string }) => <span>{i18nKey}</span>,
  useTranslation: () => ({ t: mocks.translate }),
}));

vi.mock('sonner', () => ({
  toast: { error: vi.fn(), info: vi.fn(), message: vi.fn(), success: vi.fn() },
  Toaster: () => null,
}));

vi.mock('@slab/ui/hooks/use-ai-model', () => ({
  useAiModel: vi.fn(() => ({
    ensureDownloaded: vi.fn().mockResolvedValue({ downloadedNow: false }),
    ensureLoaded: vi.fn().mockResolvedValue({ runtimeStatus: null }),
    loading: false,
    localModels: [],
    models: mocks.models,
    selectedId: 'model-a',
    setSelectedId: mocks.setSelectedModelId,
    status: { busy: false },
  })),
}));

vi.mock('@slab/ui/pages/assistant/hooks/use-assistant-sessions', () => ({
  useAssistantSessions: vi.fn(() => ({
    conversationList: [{ group: 'Workspace', key: 'session-a', label: 'Session A' }],
    createSession: vi.fn().mockResolvedValue({ id: 'session-new' }),
    currentSessionId: 'session-a',
    deleteSession: vi.fn().mockResolvedValue(true),
    isCreatingSession: false,
    isDeletingSession: false,
    isSessionMutating: false,
    isSessionsLoading: false,
    setCurrentSessionId: mocks.setCurrentSessionId,
    updateSessionLabel: vi.fn().mockResolvedValue(true),
  })),
}));

vi.mock('@slab/ui/pages/assistant/components/message-list', () => ({
  default: ({ messages }: { messages: UIMessage[] }) => (
    <div data-testid="assistant-message-list">
      {messages.map((message) => (
        <div key={message.id}>
          {message.parts
            .filter((part) => part.type === 'text')
            .map((part) => part.text)
            .join('')}
        </div>
      ))}
    </div>
  ),
}));

vi.mock('@slab/components/select', () => ({
  Select: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectGroup: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectItem: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectLabel: ({ children }: { children: ReactNode }) => <span>{children}</span>,
  SelectTrigger: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectValue: ({ placeholder }: { placeholder?: string }) => <span>{placeholder}</span>,
}));

vi.mock('@slab/components/dialog', () => ({
  Dialog: ({ children, open }: { children: ReactNode; open?: boolean }) =>
    open ? <div>{children}</div> : null,
  DialogContent: ({ children }: { children: ReactNode }) => <div role="dialog">{children}</div>,
  DialogDescription: ({ children }: { children: ReactNode }) => <p>{children}</p>,
  DialogFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
}));

vi.mock('@slab/components/sheet', () => ({
  Sheet: ({ children, open }: { children: ReactNode; open?: boolean }) =>
    open ? <div>{children}</div> : null,
  SheetContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SheetDescription: ({ children }: { children: ReactNode }) => <p>{children}</p>,
  SheetHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SheetTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
}));

describe('assistant core flow (harness) browser smoke', () => {
  beforeEach(() => {
    mocks.sendMessage.mockClear();
    mocks.stop.mockClear();
    mocks.harnessConversation.restoredMessages = [
      { id: 'message-assistant', parts: [{ text: 'previous answer', type: 'text' }], role: 'assistant' },
    ];
    mocks.harnessConversation.restoredThreadId = 'thread-a';
    mocks.harnessConversation.activeConversation = 'session-a';
    mocks.harnessConversation.isHistoryLoading = false;
    mocks.harnessConversation.error = null;
    mocks.harnessConversation.approvals = [];
    mocks.harnessConversation.approvalStatusByItemId = new Map();
    mocks.harnessConversation.liveOutputByItemId = new Map();
  });

  it('mounts the composer and renders restored messages', async () => {
    await renderDesktopScene(<AssistantPage />, { route: '/' });

    // The composer + send button expose stable test ids used by the e2e suite.
    await expect.element(page.getByTestId('assistant-composer-input')).toBeVisible();
    await expect.element(page.getByTestId('assistant-send-button')).toBeVisible();
    await expect.element(page.getByText('previous answer')).toBeVisible();
  });

  it('surfaces a pending command approval as a card above the composer', async () => {
    mocks.harnessConversation.approvals = [
      {
        itemId: 'call-1',
        threadId: 'thread-a',
        kind: 'command',
        command: 'echo slab-approval',
        cwd: '/repo',
        status: 'pending',
        allowedScopes: ['run_once', 'always_in_workspace', 'deny'],
      },
    ];
    mocks.harnessConversation.approvalStatusByItemId = new Map([['call-1', 'pending']]);

    await renderDesktopScene(<AssistantPage />, { route: '/' });

    // The ApprovalCard renders the command text above the composer and exposes
    // a per-scope test id; clicking run_once resolves the approval.
    await expect.element(page.getByText('echo slab-approval')).toBeVisible();
    await page.getByTestId('assistant-approval-run_once').click();
    await expect.poll(() => mocks.harnessConversation.resolveApproval.mock.calls.length).toBeGreaterThan(0);
    expect(mocks.harnessConversation.resolveApproval).toHaveBeenCalledWith('call-1', true, 'run_once');
  });
});
