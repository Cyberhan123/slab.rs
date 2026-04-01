import { DeleteOutlined } from '@ant-design/icons';
import { Conversations } from '@ant-design/x';
import { useXConversations } from '@ant-design/x-sdk';
import { message } from 'antd';
import dayjs from 'dayjs';
import locale from '../local';
import { DEFAULT_CONVERSATIONS_ITEMS } from '../chat-context';

interface ChatSidebarProps {
  curConversation: string;
  setCurConversation: (key: string) => void;
  activeConversation: string | undefined;
  messages: any[];
}

export const ChatSidebar = ({
  curConversation,
  setCurConversation,
  activeConversation,
  messages
}: ChatSidebarProps) => {
  const { conversations, addConversation, setConversations } = useXConversations({
    defaultConversations: DEFAULT_CONVERSATIONS_ITEMS,
  });
  const [messageApi] = message.useMessage();

  const handleCreateConversation = () => {
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
  };

  const handleDeleteConversation = (conversationKey: string) => {
    const newList = conversations.filter((item: any) => item.key !== conversationKey);
    const newKey = newList?.[0]?.key;
    setConversations(newList);
    if (conversationKey === curConversation) {
      setCurConversation(newKey || '');
    }
  };

  return (
    <div className="bg-sidebar w-72 h-full flex flex-col p-3 box-border border-r border-border">
      <Conversations
        creation={{
          onClick: handleCreateConversation,
        }}
        items={conversations
          .map(({ key, label, ...other }: any) => ({
            key,
            label: key === activeConversation ? `[${locale.curConversation}]${label}` : label,
            ...other,
          }))
          .sort(({ key }: any) => (key === activeConversation ? -1 : 0))}
        className="overflow-y-auto mt-3 p-0 flex-1"
        activeKey={curConversation}
        onActiveChange={async (val: string) => {
          setCurConversation(val);
        }}
        groupable
        menu={(conversation: any) => ({
          items: [
            {
              label: locale.delete,
              key: 'delete',
              icon: <DeleteOutlined className="size-4" />,
              danger: true,
              onClick: () => handleDeleteConversation(conversation.key),
            },
          ],
        })}
      />
    </div>
  );
};
