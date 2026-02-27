import { useState, useCallback } from 'react';
import { apiFetch, getApiConfig } from '@/lib/api';

export interface SlabChatMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
}

export interface SlabChatSession {
  id: string;
  name: string;
  created_at: string;
  updated_at: string;
}

const resolveApiUrl = (path: string): string => new URL(path, getApiConfig().baseUrl).toString();

const toChatRole = (role: string): SlabChatMessage['role'] => {
  if (role === 'assistant' || role === 'system') {
    return role;
  }
  return 'user';
};

const toErrorMessage = (error: unknown, fallback: string): string => {
  if (typeof error === 'string') {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return fallback;
};

export const useSlabChat = (sessionId?: string) => {
  const [messages, setMessages] = useState<SlabChatMessage[]>([]);
  const [isRequesting, setIsRequesting] = useState(false);
  const [currentSessionId, setCurrentSessionId] = useState<string | undefined>(sessionId);

  const loadSessionMessages = useCallback(async (sid: string) => {
    try {
      const { data, error } = await apiFetch.GET('/v1/sessions/{id}/messages', {
        params: { path: { id: sid } },
      });

      if (error || !data) {
        throw new Error('Failed to load session messages');
      }

      setMessages(
        data.map((msg) => ({
          role: toChatRole(msg.role),
          content: msg.content,
        }))
      );
    } catch (error) {
      console.error('Failed to load session messages:', error);
    }
  }, []);

  const sendMessage = useCallback(
    async (content: string, stream = false) => {
      if (!content.trim()) {
        return;
      }

      const userMessage: SlabChatMessage = { role: 'user', content };
      setMessages((prev) => [...prev, userMessage]);
      setIsRequesting(true);

      try {
        const sid = currentSessionId;

        if (stream) {
          const response = await fetch(resolveApiUrl('/v1/chat/completions'), {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              Accept: 'text/event-stream',
            },
            body: JSON.stringify({
              model: 'llama',
              messages: [userMessage],
              stream: true,
              id: sid,
              max_tokens: 512,
              temperature: 0.7,
            }),
          });

          if (!response.ok) {
            throw new Error(`${response.status} ${response.statusText}`);
          }
          if (!response.body) {
            throw new Error('No streaming response body');
          }

          const reader = response.body.getReader();
          const decoder = new TextDecoder();
          let assistantMessage = '';

          while (true) {
            const { done, value } = await reader.read();
            if (done) {
              break;
            }

            const chunk = decoder.decode(value, { stream: true });
            const lines = chunk.split('\n');

            for (const line of lines) {
              if (!line.startsWith('data: ')) {
                continue;
              }

              const raw = line.slice(6).trim();
              if (!raw || raw === '[DONE]') {
                continue;
              }

              try {
                const parsed = JSON.parse(raw) as unknown;
                const parsedObj =
                  typeof parsed === 'object' && parsed !== null
                    ? (parsed as Record<string, unknown>)
                    : null;

                const directDelta =
                  parsedObj && typeof parsedObj.delta === 'string' ? parsedObj.delta : '';

                const openAiDelta =
                  parsedObj &&
                  Array.isArray(parsedObj.choices) &&
                  parsedObj.choices[0] &&
                  typeof parsedObj.choices[0] === 'object' &&
                  parsedObj.choices[0] !== null &&
                  'delta' in parsedObj.choices[0] &&
                  typeof (parsedObj.choices[0] as { delta?: unknown }).delta === 'object' &&
                  (parsedObj.choices[0] as { delta?: { content?: unknown } }).delta &&
                  typeof (parsedObj.choices[0] as { delta?: { content?: unknown } }).delta
                    ?.content === 'string'
                    ? ((parsedObj.choices[0] as { delta?: { content?: string } }).delta?.content ??
                      '')
                    : '';

                const delta = directDelta || openAiDelta;

                if (delta) {
                  assistantMessage += delta;
                  setMessages((prev) => {
                    const next = [...prev];
                    const lastIdx = next.length - 1;
                    if (next[lastIdx]?.role === 'assistant') {
                      next[lastIdx] = { role: 'assistant', content: assistantMessage };
                    } else {
                      next.push({ role: 'assistant', content: assistantMessage });
                    }
                    return next;
                  });
                }

                if (parsedObj?.error) {
                  throw new Error(String(parsedObj.error));
                }
              } catch (parseError) {
                console.error('Failed to parse SSE data:', parseError);
              }
            }
          }
        } else {
          const { data, error } = await apiFetch.POST('/v1/chat/completions', {
            body: {
              model: 'llama',
              messages: [userMessage],
              stream: false,
              id: sid,
              max_tokens: 512,
              temperature: 0.7,
            },
          });

          if (error || !data) {
            throw new Error('Chat request failed');
          }

          const assistantContent = data.choices[0]?.message?.content ?? '';
          setMessages((prev) => [...prev, { role: 'assistant', content: assistantContent }]);
        }
      } catch (error) {
        console.error('Chat request failed:', error);
        setMessages((prev) => [
          ...prev,
          { role: 'assistant', content: `Error: ${toErrorMessage(error, 'Unknown error')}` },
        ]);
      } finally {
        setIsRequesting(false);
      }
    },
    [currentSessionId]
  );

  const createSession = useCallback(async (name?: string) => {
    try {
      const { data, error } = await apiFetch.POST('/v1/sessions', {
        body: name ? { name } : {},
      });

      if (error || !data) {
        throw new Error('Failed to create session');
      }

      const session: SlabChatSession = {
        id: data.id,
        name: data.name,
        created_at: data.created_at,
        updated_at: data.updated_at,
      };

      setCurrentSessionId(session.id);
      setMessages([]);
      return session;
    } catch (error) {
      console.error('Failed to create session:', error);
      throw error;
    }
  }, []);

  const listSessions = useCallback(async (): Promise<SlabChatSession[]> => {
    try {
      const { data, error } = await apiFetch.GET('/v1/sessions');
      if (error || !data) {
        throw new Error('Failed to list sessions');
      }

      return data.map((session) => ({
        id: session.id,
        name: session.name,
        created_at: session.created_at,
        updated_at: session.updated_at,
      }));
    } catch (error) {
      console.error('Failed to list sessions:', error);
      return [];
    }
  }, []);

  const deleteSession = useCallback(
    async (sid: string) => {
      try {
        const { error } = await apiFetch.DELETE('/v1/sessions/{id}', {
          params: { path: { id: sid } },
        });

        if (error) {
          throw new Error('Failed to delete session');
        }

        if (currentSessionId === sid) {
          setCurrentSessionId(undefined);
          setMessages([]);
        }
      } catch (error) {
        console.error('Failed to delete session:', error);
        throw error;
      }
    },
    [currentSessionId]
  );

  const clearMessages = useCallback(() => {
    setMessages([]);
  }, []);

  return {
    messages,
    isRequesting,
    sessionId: currentSessionId,
    setSessionId: setCurrentSessionId,
    sendMessage,
    loadSessionMessages,
    createSession,
    listSessions,
    deleteSession,
    clearMessages,
  };
};
