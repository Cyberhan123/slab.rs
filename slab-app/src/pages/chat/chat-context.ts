import { createContext } from 'react';
import { DeepSeekChatProvider, DefaultMessageInfo, useXChat, XModelMessage, SSEFields, XModelResponse, XRequest, XModelParams } from '@ant-design/x-sdk';

export const ChatContext = createContext<{
    onReload?: ReturnType<typeof useXChat>['onReload'];
}>({});

export const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL as string | undefined) ?? 'http://localhost:3000';

export const providerCaches = new Map<string, DeepSeekChatProvider>();

export const providerFactory = (conversationKey: string, model: string) => {
    const cacheKey = `${conversationKey}::${model}`;
    if (!providerCaches.get(cacheKey)) {
        providerCaches.set(
            cacheKey,
            new DeepSeekChatProvider({
                request: XRequest<XModelParams, Partial<Record<SSEFields, XModelResponse>>>(
                    `${API_BASE_URL}/v1/chat/completions`,
                    {
                        manual: true,
                        params: {
                            stream: true,
                            model,
                            id: conversationKey,
                        },
                    },
                ),
            }),
        );
    }
    return providerCaches.get(cacheKey);
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
