import { DeleteOutlined, OpenAIOutlined } from '@ant-design/icons';
import {

  Bubble,
  BubbleListProps,
  Conversations,
  Sender,
  SenderProps,
  XProvider,
} from '@ant-design/x';
import XMarkdown from '@ant-design/x-markdown';
import {
  useXChat,
  useXConversations,
} from '@ant-design/x-sdk';
import { Flex, GetRef, message } from 'antd';

import { clsx } from 'clsx';
import dayjs from 'dayjs';
import { useEffect, useRef, useState } from 'react';
import '@ant-design/x-markdown/themes/light.css';
import '@ant-design/x-markdown/themes/dark.css';
import { BubbleListRef } from '@ant-design/x/es/bubble';
import { useMarkdownTheme } from './use-markdowm-theme';
import locale from './local';
import { ChatContext, DEFAULT_CONVERSATIONS_ITEMS, historyMessageFactory, providerFactory } from './chat-context';
import { Footer } from './footer';
import { useStyle } from './use-style';


const slotConfig: SenderProps['slotConfig'] = [
  { type: 'text', value: locale.slotTextStart },
  {
    type: 'select',
    key: 'destination',
    props: {
      defaultValue: 'X SDK',
      options: ['X SDK', 'X Markdown', 'Bubble'],
    },
  },
  { type: 'text', value: locale.slotTextEnd },
];
function Chat() {
  const [className] = useMarkdownTheme();
  const senderRef = useRef<GetRef<typeof Sender>>(null);
  const { styles } = useStyle();
  const [messageApi, contextHolder] = message.useMessage();
  const [deepThink, setDeepThink] = useState<boolean>(true);
  const { conversations, addConversation, setConversations } = useXConversations({
    defaultConversations: DEFAULT_CONVERSATIONS_ITEMS,
  });
  const [curConversation, setCurConversation] = useState<string>(
    DEFAULT_CONVERSATIONS_ITEMS[0].key,
  );
  const listRef = useRef<BubbleListRef>(null);
  const { onRequest, messages, isRequesting, abort, onReload } = useXChat({
    provider: providerFactory(curConversation), // every conversation has its own provider
    conversationKey: curConversation,
    defaultMessages: historyMessageFactory(curConversation),
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
  const [activeConversation, setActiveConversation] = useState<string>();
  const getRole = (className: string): BubbleListProps['role'] => ({
    assistant: {
      placement: 'start',
      footer: (content, { status, key }) => (
        <Footer content={content} status={status} id={key as string} />
      ),
      contentRender: (content: any, { status }) => {
        const newContent = content.replace(/\n\n/g, '<br/><br/>');
        return (
          <XMarkdown
            paragraphTag="div"
            className={className}
            streaming={{
              hasNextChunk: status === 'updating',
              enableAnimation: true,
            }}
          >
            {newContent}
          </XMarkdown>
        );
      },
    },
    user: { placement: 'end' },
  });
  useEffect(() => {
    senderRef.current!.focus({
      cursor: 'end',
    });
  }, [senderRef.current]);
  return (
    <XProvider locale={locale}>
      {contextHolder}
      <ChatContext.Provider value={{ onReload }}>
        <div className={styles.layout}>
          <div className={styles.side}>
            <Conversations
              creation={{
                onClick: () => {
                  if (messages.length === 0) {
                    messageApi.error(locale.itIsNowANewConversation);
                    return;
                  }
                  const now = dayjs().valueOf().toString();
                  addConversation({
                    key: now,
                    label: `${locale.newConversation} ${conversations.length + 1}`,
                    group: locale.today,
                  });
                  setCurConversation(now);
                },
              }}
              items={conversations
                .map(({ key, label, ...other }) => ({
                  key,
                  label: key === activeConversation ? `[${locale.curConversation}]${label}` : label,
                  ...other,
                }))
                .sort(({ key }) => (key === activeConversation ? -1 : 0))}
              className={styles.conversations}
              activeKey={curConversation}
              onActiveChange={async (val) => {
                setCurConversation(val);
              }}
              groupable
              styles={{ item: { padding: '0 8px' } }}
              menu={(conversation) => ({
                items: [
                  {
                    label: locale.delete,
                    key: 'delete',
                    icon: <DeleteOutlined />,
                    danger: true,
                    onClick: () => {
                      const newList = conversations.filter((item) => item.key !== conversation.key);
                      const newKey = newList?.[0]?.key;
                      setConversations(newList);
                      if (conversation.key === curConversation) {
                        setCurConversation(newKey);
                      }
                    },
                  },
                ],
              })}
            />
          </div>
          <div className={styles.chat}>
            <div className={styles.chatList}>
              {messages?.length !== 0 && (
                /* 🌟 消息列表 */
                <Bubble.List
                  ref={listRef}
                  styles={{
                    root: {
                      maxWidth: 940,
                      height: 'calc(100% - 160px)',
                      marginBlockEnd: 24,
                    },
                  }}
                  items={messages?.map((i) => ({
                    ...i.message,
                    key: i.id,
                    status: i.status,
                    loading: i.status === 'loading',
                    extraInfo: i.message.extraInfo,
                  }))}
                  role={getRole(className)}
                />
              )}
              <div
                style={{ width: '100%', maxWidth: 840 }}
                className={clsx({ [styles.startPage]: messages.length === 0 })}
              >
                {messages.length === 0 && (
                  <div className={styles.agentName}>{locale.agentName}</div>
                )}
                <Sender
                  suffix={false}
                  ref={senderRef}
                  key={curConversation}
                  slotConfig={slotConfig}
                  loading={isRequesting}
                  onSubmit={(val) => {
                    if (!val) return;
                    onRequest({
                      messages: [{ role: 'user', content: val }],
                      thinking: {
                        type: 'disabled',
                      },
                    });
                    listRef.current?.scrollTo({ top: 'bottom' });
                    setActiveConversation(curConversation);
                    senderRef.current?.clear?.();
                  }}
                  onCancel={() => {
                    abort();
                  }}
                  placeholder={locale.placeholder}
                  footer={(actionNode) => {
                    return (
                      <Flex justify="space-between" align="center">
                        <Flex gap="small" align="center">
                          <Sender.Switch
                            value={deepThink}
                            onChange={(checked: boolean) => {
                              setDeepThink(checked);
                            }}
                            icon={<OpenAIOutlined />}
                          >
                            {locale.deepThink}
                          </Sender.Switch>
                        </Flex>
                        <Flex align="center">{actionNode}</Flex>
                      </Flex>
                    );
                  }}
                  autoSize={{ minRows: 3, maxRows: 6 }}
                />
              </div>
            </div>
          </div>
        </div>
      </ChatContext.Provider>
    </XProvider>
  );
}

export default Chat;