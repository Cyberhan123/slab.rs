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
import { chatMessagesStoreHelper } from '@ant-design/x-sdk/es/x-chat/store';

import type { components } from '@/lib/api/v1.d.ts';
import { SERVER_BASE_URL } from '@/lib/config';

export const ChatContext = createContext<{
    onReload?: ReturnType<typeof useXChat>['onReload'];
}>({});

export const API_BASE_URL = SERVER_BASE_URL;

type ChatApiError = components['schemas']['OpenAiError'];
type ChatApiErrorResponse = components['schemas']['OpenAiErrorResponse'];
type SessionMessageResponse = components['schemas']['MessageResponse'];
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

export class ChatTransportError extends Error {
    readonly transport_status: number;
    readonly code?: ChatApiError['code'];
    readonly param?: ChatApiError['param'];
    readonly request_id?: string | null;
    readonly error_type?: ChatRequestErrorType;

    constructor(options: {
        message: string;
        transport_status: number;
        code?: ChatApiError['code'];
        param?: ChatApiError['param'];
        request_id?: string | null;
        error_type?: ChatRequestErrorType;
    }) {
        super(options.message);
        this.name = 'ChatTransportError';
        this.transport_status = options.transport_status;
        this.code = options.code;
        this.param = options.param;
        this.request_id = options.request_id;
        this.error_type = options.error_type;
    }
}

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

export const DEFAULT_CONVERSATION_KEY = '__pending_session__';

const isRecord = (value: unknown): value is Record<string, unknown> =>
    typeof value === 'object' && value !== null;

const isEphemeralConversationKey = (value?: string): boolean => {
    const key = value?.trim();
    return !key || key === DEFAULT_CONVERSATION_KEY;
};

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

export const isChatTransportError = (value: unknown): value is ChatTransportError =>
    value instanceof ChatTransportError;

export const getChatRequestErrorMessage = (value: unknown): string | undefined => {
    if (isChatTransportError(value)) {
        return value.message;
    }

    return isChatRequestErrorInfo(value) ? value.error.message : undefined;
};

export const getChatRequestErrorMeta = (
    value: unknown,
): Pick<ChatUiMessage, 'errorCode' | 'errorParam' | 'errorStatus' | 'errorType'> => {
    if (isChatTransportError(value)) {
        return {
            errorCode: value.code ?? undefined,
            errorParam: value.param ?? undefined,
            errorStatus: value.transport_status,
            errorType: value.error_type,
        };
    }

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

const getResponseRequestId = (response: Response): string | null => {
    const requestId = response.headers.get('x-request-id') ?? response.headers.get('x-requestid');
    const trimmed = requestId?.trim();
    return trimmed ? trimmed : null;
};

const buildChatTransportError = async (response: Response): Promise<ChatTransportError> => {
    const contentType = response.headers.get('content-type') ?? '';
    const request_id = getResponseRequestId(response);

    if (contentType.includes('application/json')) {
        const payload = await response.clone().json().catch(() => null);
        if (isChatApiErrorResponse(payload)) {
            return new ChatTransportError({
                message: payload.error.message,
                transport_status: response.status,
                code: payload.error.code ?? undefined,
                param: payload.error.param ?? undefined,
                request_id,
                error_type: payload.error.type,
            });
        }
    }

    const rawBody = await response.clone().text().catch(() => '');
    return new ChatTransportError({
        message: rawBody.trim() || response.statusText || `HTTP ${response.status}`,
        transport_status: response.status,
        request_id,
    });
};

const adaptChatTransportResponse = async (response: Response): Promise<Response> => {
    if (response.ok) {
        return response;
    }

    throw await buildChatTransportError(response);
};

const isSessionMessageResponse = (value: unknown): value is SessionMessageResponse => {
    if (!isRecord(value)) {
        return false;
    }

    return typeof value.id === 'string'
        && typeof value.session_id === 'string'
        && typeof value.role === 'string'
        && typeof value.content === 'string'
        && typeof value.created_at === 'string';
};

const isStoredSessionEnvelope = (
    value: unknown,
): value is {
    message: {
        role?: unknown;
        content?: unknown;
        tool_call_id?: unknown;
        tool_calls?: unknown;
    };
} => isRecord(value) && isRecord(value.message);

const isStoredConversationMessage = (
    value: unknown,
): value is {
    role?: unknown;
    content?: unknown;
    tool_call_id?: unknown;
    tool_calls?: unknown;
} => isRecord(value) && ('content' in value || 'tool_calls' in value || 'tool_call_id' in value);

const renderStoredToolCall = (value: unknown): string => {
    if (!isRecord(value) || !isRecord(value.function) || typeof value.function.name !== 'string') {
        return '';
    }

    const rawArguments = value.function.arguments;
    const argumentsText = typeof rawArguments === 'string'
        ? rawArguments
        : JSON.stringify(rawArguments ?? '');
    const callId = typeof value.id === 'string' && value.id.trim() ? ` id=${value.id.trim()}` : '';

    return `tool_call${callId}: ${value.function.name}(${argumentsText})`;
};

const renderStoredContentPart = (value: unknown): string => {
    if (!isRecord(value) || typeof value.type !== 'string') {
        return typeof value === 'string' ? value : JSON.stringify(value ?? '');
    }

    switch (value.type) {
        case 'text':
        case 'input_text':
        case 'output_text':
        case 'refusal':
            return typeof value.text === 'string' ? value.text : '';
        case 'json':
            return JSON.stringify(value.value ?? null);
        case 'tool_result': {
            const rendered = JSON.stringify(value.value ?? null);
            const prefix =
                typeof value.tool_call_id === 'string' && value.tool_call_id.trim()
                    ? `tool_result[${value.tool_call_id.trim()}]`
                    : 'tool_result';
            return `${prefix}: ${rendered}`;
        }
        case 'image': {
            const mime = typeof value.mime_type === 'string' && value.mime_type.trim()
                ? value.mime_type.trim()
                : 'unknown';
            const source = typeof value.image_url === 'string' && value.image_url.trim()
                ? value.image_url.trim()
                : 'embedded';
            const detail = typeof value.detail === 'string' && value.detail.trim()
                ? ` detail=${value.detail.trim()}`
                : '';
            return `[image mime=${mime} src=${source}${detail}]`;
        }
        default:
            return JSON.stringify(value);
    }
};

const renderStoredMessageContent = (content: unknown): string => {
    if (typeof content === 'string') {
        return content;
    }

    if (isRecord(content) && typeof content.text === 'string') {
        return content.text;
    }

    if (Array.isArray(content)) {
        return content
            .map(renderStoredContentPart)
            .filter((part) => part.trim().length > 0)
            .join('\n');
    }

    if (content === undefined || content === null) {
        return '';
    }

    return JSON.stringify(content);
};

const toStoredSessionChatMessage = (
    fallbackRole: string,
    content: string,
): Pick<ChatUiMessage, 'content' | 'role'> => {
    const payload = JSON.parse(content) as unknown;
    const message = isStoredSessionEnvelope(payload)
        ? payload.message
        : isStoredConversationMessage(payload)
            ? payload
            : null;

    if (!message) {
        return {
            content,
            role: fallbackRole as ChatUiMessage['role'],
        };
    }

    const segments: string[] = [];
    const body = renderStoredMessageContent(message.content);
    if (body.trim()) {
        segments.push(body);
    }

    if (typeof message.tool_call_id === 'string' && message.tool_call_id.trim()) {
        segments.push(`tool_call_id: ${message.tool_call_id.trim()}`);
    }

    if (Array.isArray(message.tool_calls)) {
        const toolCalls = message.tool_calls.map(renderStoredToolCall).filter((part) => part.trim().length > 0);
        if (toolCalls.length > 0) {
            segments.push(toolCalls.join('\n'));
        }
    }

    return {
        content: segments.join('\n'),
        role: (typeof message.role === 'string' && message.role.trim()
            ? message.role.trim()
            : fallbackRole) as ChatUiMessage['role'],
    };
};

const fetchSessionMessages = async (conversationKey?: string): Promise<SessionMessageResponse[]> => {
    if (isEphemeralConversationKey(conversationKey)) {
        return [];
    }

    try {
        const response = await fetch(
            `${SERVER_BASE_URL}/v1/sessions/${encodeURIComponent(conversationKey ?? '')}/messages`,
        );

        if (response.status === 404) {
            return [];
        }

        if (!response.ok) {
            throw new Error(`failed to load session messages: ${response.status}`);
        }

        const payload = await response.json().catch(() => []);
        return Array.isArray(payload)
            ? payload.filter((item): item is SessionMessageResponse => isSessionMessageResponse(item))
            : [];
    } catch (error) {
        console.warn('failed to load session messages', { conversationKey, error });
        return [];
    }
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

export const clearConversationCache = (conversationKey: string) => {
    if (isEphemeralConversationKey(conversationKey)) {
        return;
    }

    const providerCachePrefix = `${conversationKey}::`;
    Array.from(providerCaches.keys()).forEach((cacheKey) => {
        if (cacheKey.startsWith(providerCachePrefix)) {
            providerCaches.delete(cacheKey);
        }
    });

    chatMessagesStoreHelper.delete(conversationKey);
};

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
                            onResponse: adaptChatTransportResponse,
                        },
                        params: {
                            stream: true,
                            model,
                            ...(isEphemeralConversationKey(conversationKey) ? {} : { id: conversationKey }),
                        },
                    },
                ),
            }),
        );
    }
    return providerCaches.get(cacheKey);
};

export const historyMessageFactory = async (info?: {
    conversationKey?: string | number;
}): Promise<DefaultMessageInfo<ChatUiMessage>[]> => {
    const conversationKey = typeof info?.conversationKey === 'number'
        ? String(info.conversationKey)
        : info?.conversationKey;
    const messages = await fetchSessionMessages(conversationKey);

    return messages.map((message) => ({
        id: message.id,
        message: (() => {
            try {
                return toStoredSessionChatMessage(message.role, message.content);
            } catch {
                return {
                    role: message.role as ChatUiMessage['role'],
                    content: message.content,
                };
            }
        })(),
    }));
};

export const DEFAULT_CONVERSATIONS_ITEMS: {
    key: string;
    label: string;
    group: string;
}[] = [
    {
        key: DEFAULT_CONVERSATION_KEY,
        label: 'New chat',
        group: 'Workspace',
    },
];
