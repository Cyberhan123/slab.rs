import { useXChat } from '@ant-design/x-sdk';
import { useState } from 'react';
import locale from '../local';
import { providerFactory, historyMessageFactory } from '../chat-context';

export const useChat = (
  conversationKey: string,
  model: string,
  deepThink: boolean,
  beforeRequest?: () => Promise<void> | void
) => {
  const [activeConversation, setActiveConversation] = useState<string>();

  const { onRequest, messages, isRequesting, abort, onReload: rawOnReload } = useXChat({
    provider: providerFactory(conversationKey, model),
    conversationKey: conversationKey,
    defaultMessages: historyMessageFactory(conversationKey),
    requestPlaceholder: () => {
      return {
        content: locale.noData,
        role: 'assistant',
      };
    },
    requestFallback: (_, { error, errorInfo, messageInfo }) => {
      if (error.name === 'AbortError') {
        return {
          content: messageInfo?.message?.content || locale.requestAborted,
          role: 'assistant',
        };
      }
      return {
        content: errorInfo?.error?.message || locale.requestFailed,
        role: 'assistant',
      };
    },
  });

  const handleSubmit = async (val: string) => {
    if (!val) return;
    try {
      await beforeRequest?.();
      onRequest({
        model,
        messages: [{ role: 'user', content: val }],
        thinking: {
          type: deepThink ? 'enabled' : 'disabled',
        },
      });
      setActiveConversation(conversationKey);
    } catch (_e) {
      // beforeRequest should handle its own user-facing errors.
    }
  };

  const onReload = (id: string | number, requestParams: any, opts?: any) => {
    Promise.resolve(beforeRequest?.())
      .then(() => {
        rawOnReload(
          id,
          {
            ...(requestParams ?? {}),
            model,
            thinking: {
              type: deepThink ? 'enabled' : 'disabled',
            },
          },
          opts,
        );
      })
      .catch(() => {
        // beforeRequest should handle its own user-facing errors.
      });
  };

  return {
    messages,
    isRequesting,
    abort,
    onReload,
    activeConversation,
    setActiveConversation,
    handleSubmit
  };
};
