import { createContext } from 'react';
import locale from './local';
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
                    'https://api.x.ant.design/api/big_model_glm-4.5-flash',
                    {
                        manual: true,
                        params: {
                            stream: true,
                            model: 'glm-4.5-flash',
                        },
                    },
                ),
            }),
        );
    }
    return providerCaches.get(conversationKey);
};

export const historyMessageFactory = (conversationKey: string): DefaultMessageInfo<XModelMessage>[] => {
  return HISTORY_MESSAGES[conversationKey] || [];
};

export const DEFAULT_CONVERSATIONS_ITEMS = [
    {
        key: 'default-0',
        label: locale.whatIsAntDesignX,
        group: locale.today,
    },
    {
        key: 'default-1',
        label: locale.howToQuicklyInstallAndImportComponents,
        group: locale.today,
    },
    {
        key: 'default-2',
        label: locale.newAgiHybridInterface,
        group: locale.yesterday,
    },
];

export const HISTORY_MESSAGES: {
    [key: string]: DefaultMessageInfo<XModelMessage>[];
} = {
    'default-1': [
        {
            message: { role: 'user', content: locale.howToQuicklyInstallAndImportComponents },
            status: 'success',
        },
        {
            message: {
                role: 'assistant',
                content: locale.aiMessage_2,
            },
            status: 'success',
        },
    ],
    'default-2': [
        { message: { role: 'user', content: locale.newAgiHybridInterface }, status: 'success' },
        {
            message: {
                role: 'assistant',
                content: locale.aiMessage_1,
            },
            status: 'success',
        },
    ],
};
