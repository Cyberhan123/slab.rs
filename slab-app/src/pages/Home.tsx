import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "@/store/useAppStore";

interface Message {
  id: string;
  content: string;
  sender: 'user' | 'ai';
  timestamp: Date;
}

function Home() {
  const [message, setMessage] = useState("");
  const [messages, setMessages] = useState<Message[]>([
    {
      id: "1",
      content: "Hello! I'm Slab AI, your intelligent assistant. How can I help you today?",
      sender: "ai",
      timestamp: new Date(),
    },
  ]);
  const { isLoading, setIsLoading } = useAppStore();
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  useEffect(() => {
    scrollToBottom();
  }, [messages]);

  const sendMessage = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!message.trim()) return;

    // Add user message
    const userMessage: Message = {
      id: Date.now().toString(),
      content: message,
      sender: "user",
      timestamp: new Date(),
    };

    setMessages(prev => [...prev, userMessage]);
    setMessage("");
    setIsLoading(true);

    try {
      const result = await invoke("greet", { name: message });
      // Add AI response
      const aiMessage: Message = {
        id: (Date.now() + 1).toString(),
        content: result as string,
        sender: "ai",
        timestamp: new Date(),
      };
      setMessages(prev => [...prev, aiMessage]);
    } catch (error) {
      console.error("Error calling greet command:", error);
      const errorMessage: Message = {
        id: (Date.now() + 1).toString(),
        content: "An error occurred. Please try again.",
        sender: "ai",
        timestamp: new Date(),
      };
      setMessages(prev => [...prev, errorMessage]);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <main className="max-w-4xl mx-auto p-4 md:p-8 bg-background min-h-screen">
      <div className="bg-white/80 backdrop-blur-sm rounded-2xl shadow-lg overflow-hidden border border-border">
        {/* Chat header */}
        <div className="bg-primary/5 border-b border-border p-4">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 bg-primary rounded-lg flex items-center justify-center">
              <span className="text-white font-bold text-xl">AI</span>
            </div>
            <div>
              <h2 className="font-semibold text-foreground">Slab AI Assistant</h2>
              <p className="text-sm text-muted-foreground">Online • Ready to help</p>
            </div>
          </div>
        </div>

        {/* Messages */}
        <div className="p-6 max-h-[60vh] overflow-y-auto space-y-4">
          {messages.map((msg) => (
            <div key={msg.id} className={`flex ${msg.sender === 'user' ? 'justify-end' : 'justify-start'}`}>
              <div className={`max-w-[80%] ${msg.sender === 'user' ? 'bg-primary text-white rounded-2xl rounded-tr-none' : 'bg-muted rounded-2xl rounded-tl-none'}`}>
                <div className="p-4">
                  <p className={msg.sender === 'user' ? 'text-white' : 'text-foreground'}>
                    {msg.content}
                  </p>
                  <p className={`text-xs mt-2 ${msg.sender === 'user' ? 'text-white/70' : 'text-muted-foreground'}`}>
                    {msg.timestamp.toLocaleTimeString()}
                  </p>
                </div>
              </div>
            </div>
          ))}
          <div ref={messagesEndRef} />
        </div>

        {/* Input area */}
        <div className="p-4 border-t border-border">
          <form onSubmit={sendMessage} className="flex gap-3">
            <input
              value={message}
              onChange={(e) => setMessage(e.currentTarget.value)}
              placeholder="Type your message..."
              disabled={isLoading}
              className="flex-grow px-4 py-3 border border-border rounded-full focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary transition-all bg-white"
            />
            <button
              type="submit"
              disabled={isLoading}
              className="w-12 h-12 bg-primary text-white rounded-full flex items-center justify-center hover:bg-primary/90 transition-colors disabled:bg-muted disabled:cursor-not-allowed"
            >
              {isLoading ? (
                <div className="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin" />
              ) : (
                "➤"
              )}
            </button>
          </form>
          <p className="text-xs text-muted-foreground mt-3 text-center">
            Slab AI is designed to assist with your questions and tasks. Please be respectful.
          </p>
        </div>
      </div>

      {/* Features section */}
      <div className="mt-12 grid grid-cols-1 md:grid-cols-3 gap-6">
        <div className="bg-white/80 backdrop-blur-sm rounded-xl shadow-md p-6 hover:shadow-lg transition-shadow border border-border">
          <div className="w-12 h-12 bg-primary/10 rounded-full flex items-center justify-center mb-4">
            <span className="text-primary font-bold text-xl">🚀</span>
          </div>
          <h3 className="text-lg font-semibold text-foreground mb-2">Fast Performance</h3>
          <p className="text-muted-foreground">Built with Tauri for native performance and React for responsive UI.</p>
        </div>
        <div className="bg-white/80 backdrop-blur-sm rounded-xl shadow-md p-6 hover:shadow-lg transition-shadow border border-border">
          <div className="w-12 h-12 bg-primary/10 rounded-full flex items-center justify-center mb-4">
            <span className="text-primary font-bold text-xl">🎨</span>
          </div>
          <h3 className="text-lg font-semibold text-foreground mb-2">Modern Design</h3>
          <p className="text-muted-foreground">Clean, intuitive interface with an eye-friendly cyan color scheme.</p>
        </div>
        <div className="bg-white/80 backdrop-blur-sm rounded-xl shadow-md p-6 hover:shadow-lg transition-shadow border border-border">
          <div className="w-12 h-12 bg-primary/10 rounded-full flex items-center justify-center mb-4">
            <span className="text-primary font-bold text-xl">🔒</span>
          </div>
          <h3 className="text-lg font-semibold text-foreground mb-2">Secure</h3>
          <p className="text-muted-foreground">Rust-based backend for enhanced security and reliability.</p>
        </div>
      </div>
    </main>
  );
}

export default Home;