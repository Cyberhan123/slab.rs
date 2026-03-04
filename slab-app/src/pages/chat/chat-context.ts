import { createContext } from 'react';
import { OpenAIChatProvider, DefaultMessageInfo, useXChat, XModelMessage, SSEFields, XModelResponse, XRequest, XModelParams } from '@ant-design/x-sdk';

export const ChatContext = createContext<{
    onReload?: ReturnType<typeof useXChat>['onReload'];
}>({});

export const providerCaches = new Map<string, OpenAIChatProvider>();

export const providerFactory = (conversationKey: string) => {
    if (!providerCaches.get(conversationKey)) {
        providerCaches.set(
            conversationKey,
            new OpenAIChatProvider({
                request: XRequest<XModelParams, Partial<Record<SSEFields, XModelResponse>>>(
                    'http://localhost:3000/v1/chat/completions',
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

export const DEFAULT_CONVERSATIONS_ITEMS: {
    key: string;
    label: string;
    group: string;
}[] = [];
