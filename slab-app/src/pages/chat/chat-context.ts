import { createContext } from 'react';
import {
    DeepSeekChatProvider,
    DefaultMessageInfo,
    MessageInfo,
    SSEFields,
    useXChat,
    XModelMessage,
    XModelParams,
    XModelResponse,
    XRequest,
} from '@ant-design/x-sdk';

import type { components } from '@/lib/api/v1.d.ts';

export const ChatContext = createContext<{
    onReload?: ReturnType<typeof useXChat>['onReload'];
}>({});

export const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL as string | undefined) ?? 'http://localhost:3000';

type ChatApiError = components['schemas']['OpenAiError'];
type ChatApiErrorResponse = components['schemas']['OpenAiErrorResponse'];
type ProviderTransformParamsOptions = Parameters<
    DeepSeekChatProvider<
        ChatUiMessage,
        ChatRequestParams,
        Partial<Record<SSEFields, XModelResponse>>
    >['transformParams']
>[1];
type ProviderTransformMessageInfo = Parameters<
    DeepSeekChatProvider<
        ChatUiMessage,
        ChatRequestParams,
        Partial<Record<SSEFields, XModelResponse>>
    >['transformMessage']
>[0];

export type ChatRequestErrorType = ChatApiError['type'];

export type ChatUiMessage = XModelMessage & {
    errorCode?: ChatApiError['code'];
    errorParam?: ChatApiError['param'];
    errorStatus?: number;
    errorType?: ChatRequestErrorType;
};

export type ChatMessageRecord = MessageInfo<ChatUiMessage>;
export type ChatRequestParams = XModelParams & {
    continue_generation?: boolean;
    thinking?: {
        type: 'enabled' | 'disabled';
    };
    userAction?: string;
};

export type ChatRequestErrorInfo = {
    error: ChatApiError;
    message: string;
    name: string;
    status: number;
    statusText: string;
    success: false;
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
    typeof value === 'object' && value !== null;

const isChatApiErrorResponse = (value: unknown): value is ChatApiErrorResponse => {
    if (!isRecord(value) || !isRecord(value.error)) {
        return false;
    }

    const { error } = value;

    return typeof error.message === 'string'
        && typeof error.type === 'string'
        && (!('code' in error) || error.code === null || typeof error.code === 'string')
        && (!('param' in error) || error.param === null || typeof error.param === 'string');
};

export const isChatRequestErrorInfo = (value: unknown): value is ChatRequestErrorInfo => {
    if (!isRecord(value) || value.success !== false || !isRecord(value.error)) {
        return false;
    }

    return typeof value.message === 'string'
        && typeof value.name === 'string'
        && typeof value.status === 'number'
        && typeof value.statusText === 'string'
        && typeof value.error.message === 'string'
        && typeof value.error.type === 'string'
        && (!('code' in value.error) || value.error.code === null || typeof value.error.code === 'string')
        && (!('param' in value.error) || value.error.param === null || typeof value.error.param === 'string');
};

export const getChatRequestErrorMessage = (value: unknown): string | undefined =>
    isChatRequestErrorInfo(value) ? value.error.message : undefined;

export const getChatRequestErrorMeta = (
    value: unknown,
): Pick<ChatUiMessage, 'errorCode' | 'errorParam' | 'errorStatus' | 'errorType'> => {
    if (!isChatRequestErrorInfo(value)) {
        return {};
    }

    return {
        errorCode: value.error.code ?? undefined,
        errorParam: value.error.param ?? undefined,
        errorStatus: value.status,
        errorType: value.error.type,
    };
};

export const getChatMessageTextContent = (
    message?: Pick<XModelMessage, 'content'> | null,
): string => {
    const content = message?.content;

    if (typeof content === 'string') {
        return content;
    }

    if (content && typeof content.text === 'string') {
        return content.text;
    }

    return '';
};

export const toChatRequestMessage = (
    message?: Pick<XModelMessage, 'role' | 'content'> | null,
): XModelMessage | null => {
    if (!message) {
        return null;
    }

    return {
        role: message.role,
        content: typeof message.content === 'string'
            ? message.content
            : {
                text: message.content?.text ?? '',
                type: message.content?.type ?? 'text',
            },
    };
};

export const toChatRequestMessages = (
    messages?: Array<Pick<XModelMessage, 'role' | 'content'> | null | undefined>,
): XModelMessage[] => {
    return (messages ?? [])
        .map(toChatRequestMessage)
        .filter((message): message is XModelMessage => Boolean(message));
};

export const getContinueGenerationPrefix = (
    messages?: Array<Pick<XModelMessage, 'role' | 'content'> | null | undefined>,
): string => {
    for (let index = (messages?.length ?? 0) - 1; index >= 0; index -= 1) {
        const message = messages?.[index];
        if (!message) {
            continue;
        }

        const content = getChatMessageTextContent(message);
        if (!content.trim()) {
            continue;
        }

        return message.role === 'assistant' ? content : '';
    }

    return '';
};

const mergeContinuationContent = (prefix: string, generated: string): string => {
    if (!prefix) {
        return generated;
    }

    if (!generated) {
        return prefix;
    }

    const maxOverlap = Math.min(prefix.length, generated.length);
    for (let size = maxOverlap; size > 0; size -= 1) {
        if (prefix.slice(-size) === generated.slice(0, size)) {
            return `${prefix}${generated.slice(size)}`;
        }
    }

    return `${prefix}${generated}`;
};

const toChatRequestErrorInfo = (
    response: Response,
    payload: ChatApiErrorResponse,
): ChatRequestErrorInfo => ({
    error: payload.error,
    message: payload.error.message,
    name: payload.error.type,
    status: response.status,
    statusText: response.statusText,
    success: false,
});

const normalizeChatErrorResponse = async (response: Response): Promise<Response> => {
    if (response.ok) {
        return response;
    }

    const contentType = response.headers.get('content-type') ?? '';
    if (!contentType.includes('application/json')) {
        return response;
    }

    const payload = await response.clone().json().catch(() => null);
    if (!isChatApiErrorResponse(payload)) {
        return response;
    }

    const headers = new Headers(response.headers);
    headers.set('content-type', 'application/json');

    return new Response(JSON.stringify(toChatRequestErrorInfo(response, payload)), {
        headers,
        status: 200,
        statusText: 'OK',
    });
};

class ContinueGenerationChatProvider extends DeepSeekChatProvider<
    ChatUiMessage,
    ChatRequestParams,
    Partial<Record<SSEFields, XModelResponse>>
> {
    private pendingContinuationPrefix = '';

    override transformParams(
        requestParams: Partial<ChatRequestParams>,
        options: ProviderTransformParamsOptions,
    ): ChatRequestParams {
        const requestMessages = requestParams.continue_generation && requestParams.messages?.length
            ? requestParams.messages
            : this.getMessages();

        this.pendingContinuationPrefix = requestParams.continue_generation
            ? getContinueGenerationPrefix(requestParams.messages)
            : '';

        return {
            ...(options?.params || {}),
            ...requestParams,
            messages: toChatRequestMessages(requestMessages),
        };
    }

    override transformLocalMessage(requestParams: Partial<ChatRequestParams>): ChatUiMessage[] {
        if (requestParams?.continue_generation) {
            return [];
        }

        return super.transformLocalMessage(requestParams);
    }

    override transformMessage(info: ProviderTransformMessageInfo): ChatUiMessage {
        const message = super.transformMessage(info);
        if (!this.pendingContinuationPrefix) {
            return message;
        }

        const prefix = this.pendingContinuationPrefix;
        this.pendingContinuationPrefix = '';

        return {
            ...message,
            content: mergeContinuationContent(prefix, getChatMessageTextContent(message)),
        };
    }
}

export const providerCaches = new Map<string, ContinueGenerationChatProvider>();

export const providerFactory = (conversationKey: string, model: string) => {
    const cacheKey = `${conversationKey}::${model}`;
    if (!providerCaches.get(cacheKey)) {
        providerCaches.set(
            cacheKey,
            new ContinueGenerationChatProvider({
                request: XRequest<ChatRequestParams, Partial<Record<SSEFields, XModelResponse>>, ChatUiMessage>(
                    `${API_BASE_URL}/v1/chat/completions`,
                    {
                        manual: true,
                        middlewares: {
                            onResponse: normalizeChatErrorResponse,
                        },
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

export const historyMessageFactory = (_conversationKey: string): DefaultMessageInfo<ChatUiMessage>[] => {
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
