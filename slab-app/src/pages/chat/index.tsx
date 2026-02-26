import { XProvider } from '@ant-design/x';
import { useState } from 'react';
import '@ant-design/x-markdown/themes/light.css';
import '@ant-design/x-markdown/themes/dark.css';
import { useMarkdownTheme } from './hooks/use-markdowm-theme';
import locale from './local';
import { ChatContext, DEFAULT_CONVERSATIONS_ITEMS } from './chat-context';
import { useStyle } from './hooks/use-style';
import { ChatSidebar } from './components/chat-sidebar';
import { ChatMessageList } from './components/chat-message-list';
import { ChatInput } from './components/chat-input';
import { useChat } from './hooks/use-chat';

function Chat() {
  const [className] = useMarkdownTheme();
  const styles = useStyle();
  const [deepThink, setDeepThink] = useState<boolean>(true);
  const [curConversation, setCurConversation] = useState<string>(
    DEFAULT_CONVERSATIONS_ITEMS[0].key,
  );
  
  const { 
    messages, 
    isRequesting, 
    abort, 
    onReload, 
    activeConversation, 
    handleSubmit 
  } = useChat(curConversation);

  return (
    <XProvider locale={locale}>
      <ChatContext.Provider value={{ onReload }}>
        <div className={styles.layout}>
          <ChatSidebar 
            curConversation={curConversation}
            setCurConversation={setCurConversation}
            activeConversation={activeConversation}
            messages={messages}
          />
          <div className={styles.chat}>
            <div className={styles.chatList}>
              {messages?.length !== 0 ? (
                <ChatMessageList 
                  messages={messages}
                  className={className}
                  onReload={onReload}
                />
              ) : (
                <div className={styles.startPage}>
                  <div className={styles.agentName}>{locale.agentName}</div>
                  <ChatInput 
                    isRequesting={isRequesting}
                    deepThink={deepThink}
                    setDeepThink={setDeepThink}
                    onSubmit={handleSubmit}
                    onCancel={abort}
                    curConversation={curConversation}
                  />
                </div>
              )}
              
              {messages?.length !== 0 && (
                <ChatInput 
                  isRequesting={isRequesting}
                  deepThink={deepThink}
                  setDeepThink={setDeepThink}
                  onSubmit={handleSubmit}
                  onCancel={abort}
                  curConversation={curConversation}
                />
              )}
            </div>
          </div>
        </div>
      </ChatContext.Provider>
    </XProvider>
  );
}

export default Chat;