import { useState, useEffect } from 'react';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Spinner } from '@/components/ui/spinner';
import { toast } from 'sonner';
import { Send, Plus, Trash2 } from 'lucide-react';
import { useSlabChat, type SlabChatSession } from './hooks/use-slab-chat';

interface SlabChatProps {
  initialSessionId?: string;
}

export default function SlabChat({ initialSessionId }: SlabChatProps) {
  const [input, setInput] = useState('');
  const [sessions, setSessions] = useState<SlabChatSession[]>([]);

  const {
    messages,
    isRequesting,
    sessionId,
    setSessionId,
    sendMessage,
    loadSessionMessages,
    createSession,
    listSessions,
    deleteSession,
    clearMessages
  } = useSlabChat(initialSessionId);

  // Load sessions on mount
  useEffect(() => {
    loadSessions();
  }, []);

  // Load messages when session changes
  useEffect(() => {
    if (sessionId) {
      loadSessionMessages(sessionId);
    } else {
      clearMessages();
    }
  }, [sessionId, loadSessionMessages, clearMessages]);

  const loadSessions = async () => {
    try {
      const sessionList = await listSessions();
      setSessions(sessionList);
    } catch (error) {
      toast.error('Failed to load sessions');
    }
  };

  const handleSendMessage = async () => {
    if (!input.trim() || isRequesting) return;

    const userMessage = input;
    setInput('');

    // Create session if needed
    let currentSessionId = sessionId;
    if (!currentSessionId) {
      try {
        const newSession = await createSession('New Chat');
        currentSessionId = newSession.id;
        await loadSessions();
      } catch (error) {
        toast.error('Failed to create session');
        return;
      }
    }

    await sendMessage(userMessage, false);
  };

  const handleNewChat = async () => {
    try {
      await createSession('New Chat');
      await loadSessions();
      toast.success('New chat created');
    } catch (error) {
      toast.error('Failed to create new chat');
    }
  };

  const handleDeleteSession = async (sid: string) => {
    try {
      await deleteSession(sid);
      await loadSessions();
      toast.success('Session deleted');
    } catch (error) {
      toast.error('Failed to delete session');
    }
  };

  return (
    <div className="h-screen flex">
      {/* Sessions Sidebar */}
      <Card className="w-64 m-4 p-4 flex flex-col">
        <div className="flex items-center justify-between mb-4">
          <h2 className="font-semibold">Chats</h2>
          <Button
            size="sm"
            variant="ghost"
            onClick={handleNewChat}
          >
            <Plus className="h-4 w-4" />
          </Button>
        </div>

        <ScrollArea className="flex-1">
          <div className="space-y-2">
            {sessions.map((session) => (
              <div
                key={session.id}
                className={`p-2 rounded cursor-pointer hover:bg-muted transition-colors ${
                  sessionId === session.id ? 'bg-muted' : ''
                }`}
                onClick={() => setSessionId(session.id)}
              >
                <div className="flex items-center justify-between">
                  <span className="text-sm truncate flex-1">
                    {session.name || 'New Chat'}
                  </span>
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-6 w-6 p-0"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleDeleteSession(session.id);
                    }}
                  >
                    <Trash2 className="h-3 w-3" />
                  </Button>
                </div>
                <p className="text-xs text-muted-foreground">
                  {new Date(session.created_at).toLocaleDateString()}
                </p>
              </div>
            ))}
            {sessions.length === 0 && (
              <p className="text-sm text-muted-foreground text-center py-4">
                No chats yet
              </p>
            )}
          </div>
        </ScrollArea>
      </Card>

      {/* Chat Area */}
      <Card className="flex-1 m-4 ml-0 flex flex-col">
        {/* Messages */}
        <ScrollArea className="flex-1 p-4">
          <div className="space-y-4">
            {messages.length === 0 && (
              <div className="h-full flex items-center justify-center">
                <div className="text-center text-muted-foreground">
                  <p className="text-lg mb-2">Start a conversation</p>
                  <p className="text-sm">Type a message below to begin</p>
                </div>
              </div>
            )}

            {messages.map((message, index) => (
              <div
                key={index}
                className={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
              >
                <div
                  className={`max-w-[80%] rounded-lg p-3 ${
                    message.role === 'user'
                      ? 'bg-primary text-primary-foreground'
                      : 'bg-muted'
                  }`}
                >
                  <p className="text-sm whitespace-pre-wrap">{message.content}</p>
                </div>
              </div>
            ))}

            {isRequesting && (
              <div className="flex justify-start">
                <div className="bg-muted rounded-lg p-3">
                  <Spinner className="h-4 w-4" />
                </div>
              </div>
            )}
          </div>
        </ScrollArea>

        {/* Input */}
        <div className="border-t p-4">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              handleSendMessage();
            }}
            className="flex gap-2"
          >
            <Input
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="Type your message..."
              disabled={isRequesting}
            />
            <Button
              type="submit"
              disabled={!input.trim() || isRequesting}
            >
              {isRequesting ? (
                <Spinner className="h-4 w-4" />
              ) : (
                <Send className="h-4 w-4" />
              )}
            </Button>
          </form>
          <p className="text-xs text-muted-foreground mt-2">
            {sessionId ? `Session: ${sessionId}` : 'No active session'}
          </p>
        </div>
      </Card>
    </div>
  );
}
