import { useState, useCallback } from 'react';
import api from '@/lib/api';

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

export const useSlabChat = (sessionId?: string) => {
  const [messages, setMessages] = useState<SlabChatMessage[]>([]);
  const [isRequesting, setIsRequesting] = useState(false);
  const [currentSessionId, setCurrentSessionId] = useState<string | undefined>(sessionId);

  // Load session messages
  const loadSessionMessages = useCallback(async (sid: string) => {
    try {
      const response = await api.get('/v1/sessions/{id}/messages', {
        params: { path: { id: sid } }
      });
      const data = await response.json();
      setMessages(data.map((msg: any) => ({
        role: msg.role,
        content: msg.content
      })));
    } catch (error) {
      console.error('Failed to load session messages:', error);
    }
  }, []);

  // Send message to backend
  const sendMessage = useCallback(async (content: string, stream = false) => {
    if (!content.trim()) return;

    const userMessage: SlabChatMessage = { role: 'user', content };
    setMessages(prev => [...prev, userMessage]);
    setIsRequesting(true);

    try {
      // Use existing session or create implicit one
      const sid = currentSessionId;

      if (stream) {
        // Streaming response
        const response = await api.post('/v1/chat/completions', {
          body: {
            model: 'llama',
            messages: [userMessage],
            stream: true,
            id: sid,
            max_tokens: 512,
            temperature: 0.7
          },
          headers: {
            'Accept': 'text/event-stream'
          }
        });

        // Handle SSE stream
        const reader = response.body?.getReader();
        const decoder = new TextDecoder();
        let assistantMessage = '';

        if (reader) {
          while (true) {
            const { done, value } = await reader.read();
            if (done) break;

            const chunk = decoder.decode(value);
            const lines = chunk.split('\n');

            for (const line of lines) {
              if (line.startsWith('data: ')) {
                const data = line.slice(6);
                try {
                  const parsed = JSON.parse(data);
                  if (parsed.delta) {
                    assistantMessage += parsed.delta;
                    setMessages(prev => {
                      const newMessages = [...prev];
                      const lastIdx = newMessages.length - 1;
                      if (newMessages[lastIdx]?.role === 'assistant') {
                        newMessages[lastIdx] = {
                          role: 'assistant',
                          content: assistantMessage
                        };
                      } else {
                        newMessages.push({
                          role: 'assistant',
                          content: assistantMessage
                        });
                      }
                      return newMessages;
                    });
                  }
                  if (parsed.error) {
                    throw new Error(parsed.error);
                  }
                } catch (e) {
                  console.error('Failed to parse SSE data:', e);
                }
              }
            }
          }
        }
      } else {
        // Non-streaming response
        const response = await api.post('/v1/chat/completions', {
          body: {
            model: 'llama',
            messages: [userMessage],
            stream: false,
            id: sid,
            max_tokens: 512,
            temperature: 0.7
          }
        });

        const data = await response.json();
        const assistantContent = data.choices[0]?.message?.content || '';

        setMessages(prev => [...prev, {
          role: 'assistant',
          content: assistantContent
        }]);
      }

      setIsRequesting(false);
    } catch (error) {
      console.error('Chat request failed:', error);
      setMessages(prev => [...prev, {
        role: 'assistant',
        content: `Error: ${error instanceof Error ? error.message : 'Unknown error'}`
      }]);
      setIsRequesting(false);
    }
  }, [currentSessionId]);

  // Create new session
  const createSession = useCallback(async (name?: string) => {
    try {
      const response = await api.post('/v1/sessions', {
        body: { name }
      });
      const session: SlabChatSession = await response.json();
      setCurrentSessionId(session.id);
      setMessages([]);
      return session;
    } catch (error) {
      console.error('Failed to create session:', error);
      throw error;
    }
  }, []);

  // List sessions
  const listSessions = useCallback(async (): Promise<SlabChatSession[]> => {
    try {
      const response = await api.get('/v1/sessions');
      const data = await response.json();
      return data;
    } catch (error) {
      console.error('Failed to list sessions:', error);
      return [];
    }
  }, []);

  // Delete session
  const deleteSession = useCallback(async (sid: string) => {
    try {
      await api.delete('/v1/sessions/{id}', {
        params: { path: { id: sid } }
      });
      if (currentSessionId === sid) {
        setCurrentSessionId(undefined);
        setMessages([]);
      }
    } catch (error) {
      console.error('Failed to delete session:', error);
      throw error;
    }
  }, [currentSessionId]);

  // Clear current messages
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
    clearMessages
  };
};
