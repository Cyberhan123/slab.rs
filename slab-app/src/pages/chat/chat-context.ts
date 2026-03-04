import { createContext } from 'react';
import { OpenAIChatProvider, DefaultMessageInfo, useXChat, XModelMessage, SSEFields, XModelResponse, XRequest, XModelParams } from '@ant-design/x-sdk';

export const ChatContext = createContext<{
    onReload?: ReturnType<typeof useXChat>['onReload'];
}>({});

const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL as string | undefined) ?? 'http://localhost:3000';

export const providerCaches = new Map<string, OpenAIChatProvider>();

export const providerFactory = (conversationKey: string) => {
    if (!providerCaches.get(conversationKey)) {
        providerCaches.set(
            conversationKey,
            new OpenAIChatProvider({
                request: XRequest<XModelParams, Partial<Record<SSEFields, XModelResponse>>>(
                    `${API_BASE_URL}/v1/chat/completions`,
                    {
                        manual: true,
                        params: {
                            stream: true,
                            model: 'slab-llama',
                        },
                    },
                ),
            }),
        );
    }
    return providerCaches.get(conversationKey);
};

export const historyMessageFactory = (_conversationKey: string): DefaultMessageInfo<XModelMessage>[] => {
  return [];
};

export const DEFAULT_CONVERSATION_KEY = 'default-conversation';

export const DEFAULT_CONVERSATIONS_ITEMS: {
    key: string;
    label: string;
    group: string;
}[] = [
    {
        key: DEFAULT_CONVERSATION_KEY,
        label: 'New chat',
        group: 'default',
    },
];
