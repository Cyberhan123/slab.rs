import { Bubble, BubbleListProps } from '@ant-design/x';
import { BubbleListRef } from '@ant-design/x/es/bubble';
import { useRef } from 'react';
import XMarkdown from '@ant-design/x-markdown';
import { Footer } from './footer';
import { useStyle } from '../hooks/use-style';

interface ChatMessageListProps {
  messages: any[];
  className: string;
  onReload: (id: string | number, requestParams: any, opts?: any) => void;
}

export const ChatMessageList = ({ messages, className }: ChatMessageListProps) => {
  const listRef = useRef<BubbleListRef>(null);
  const styles = useStyle();

  const getRole = (className: string): BubbleListProps['role'] => ({
    assistant: {
      placement: 'start',
      footer: (content: any, info: any) => (
        <Footer content={content} status={info.status || ''} id={info.key || ''} />
      ),
      contentRender: (content: any, info: any) => {
        const newContent = content.replace(/\n\n/g, '<br/><br/>');
        return (
          <XMarkdown
            paragraphTag="div"
            className={className}
            streaming={{
              hasNextChunk: info.status === 'updating',
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

  return (
    <div className={styles.messageList}>
      <Bubble.List
        ref={listRef}
        items={messages?.map((i) => ({
          ...i.message,
          key: i.id,
          status: i.status,
          loading: i.status === 'loading',
          extraInfo: i.message.extraInfo,
        }))}
        role={getRole(className)}
      />
    </div>
  );
};
