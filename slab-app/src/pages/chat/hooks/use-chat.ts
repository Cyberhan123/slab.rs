import { useXChat } from '@ant-design/x-sdk';
import { useState } from 'react';
import locale from '../local';
import { providerFactory, historyMessageFactory } from '../chat-context';

export const useChat = (conversationKey: string) => {
  const [activeConversation, setActiveConversation] = useState<string>();

  const { onRequest, messages, isRequesting, abort, onReload } = useXChat({
    provider: providerFactory(conversationKey),
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

  const handleSubmit = (val: string) => {
    if (!val) return;
    onRequest({
      messages: [{ role: 'user', content: val }],
      thinking: {
        type: 'disabled',
      },
    });
    setActiveConversation(conversationKey);
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
