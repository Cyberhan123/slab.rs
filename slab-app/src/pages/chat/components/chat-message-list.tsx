import { Bubble, BubbleListProps, Think } from '@ant-design/x';
import { BubbleListRef } from '@ant-design/x/es/bubble';
import { useRef, useState } from 'react';
import XMarkdown from '@ant-design/x-markdown';
import { Footer } from './footer';
import locale from '../local';
import { useStyle } from '../hooks/use-style';

interface ChatMessageListProps {
  messages: any[];
  className: string;
  onReload: (id: string | number, requestParams: any, opts?: any) => void;
}

type ParsedThinkingContent = {
  thinking: string | null;
  answer: string;
  thinkingLoading: boolean;
};

function parseThinkingContent(rawContent: string): ParsedThinkingContent {
  const openTagIndex = rawContent.indexOf('<think');
  if (openTagIndex < 0) {
    return { thinking: null, answer: rawContent, thinkingLoading: false };
  }

  const openTagEnd = rawContent.indexOf('>', openTagIndex);
  if (openTagEnd < 0) {
    return { thinking: null, answer: rawContent, thinkingLoading: false };
  }

  const openTag = rawContent.slice(openTagIndex, openTagEnd + 1);
  const thinkingMarkedDone = /\bstatus\s*=\s*["']?done["']?/i.test(openTag);
  const closeTag = '</think>';
  const closeTagIndex = rawContent.indexOf(closeTag, openTagEnd + 1);

  if (closeTagIndex < 0) {
    const thinking = rawContent.slice(openTagEnd + 1).trimStart();

    return {
      thinking: thinking || null,
      answer: rawContent.slice(0, openTagIndex).trimEnd(),
      thinkingLoading: !thinkingMarkedDone,
    };
  }

  const thinking = rawContent.slice(openTagEnd + 1, closeTagIndex).trim();
  const before = rawContent.slice(0, openTagIndex);
  const after = rawContent.slice(closeTagIndex + closeTag.length);

  return {
    thinking: thinking || null,
    answer: `${before}${after}`.trimStart(),
    thinkingLoading: false,
  };
}

export const ChatMessageList = ({ messages, className }: ChatMessageListProps) => {
  const listRef = useRef<BubbleListRef>(null);
  const styles = useStyle();
  const [thinkingExpandedByMessage, setThinkingExpandedByMessage] = useState<
    Record<string, boolean>
  >({});

  const isThinkingExpanded = (messageKey: string) =>
    Boolean(thinkingExpandedByMessage[messageKey]);

  const setThinkingExpanded = (messageKey: string, expanded: boolean) => {
    setThinkingExpandedByMessage((prev) => ({
      ...prev,
      [messageKey]: expanded,
    }));
  };

  const getRole = (className: string): BubbleListProps['role'] => ({
    assistant: {
      placement: 'start',
      footer: (content: any, info: any) => (
        <Footer content={content} status={info.status || ''} id={info.key || ''} />
      ),
      contentRender: (content: any, info: any) => {
        const messageKey = String(info.key ?? '');
        const thinkingExpanded = isThinkingExpanded(messageKey);
        const rawContent = String(content ?? '');
        const { thinking, answer, thinkingLoading } = parseThinkingContent(rawContent);
        const isWaitingForResponse = info.status === 'loading' || info.status === 'updating';
        const hasNextChunk = info.status === 'updating';
        const answerMarkdown = answer.replace(/\n\n/g, '<br/><br/>');
        const thinkingMarkdown = (thinking ?? '').replace(/\n\n/g, '<br/><br/>');

        return (
          <div className="space-y-3">
            {thinking ? (
              <Think
                title={
                  thinkingLoading && isWaitingForResponse
                    ? locale.deepThinking
                    : locale.completeThinking
                }
                loading={thinkingLoading && isWaitingForResponse}
                blink={thinkingLoading && isWaitingForResponse}
                expanded={thinkingExpanded}
                onExpand={(expanded) => setThinkingExpanded(messageKey, expanded)}
                className="max-w-full"
              >
                {thinkingExpanded ? (
                  <XMarkdown
                    paragraphTag="div"
                    className={className}
                    streaming={{
                      hasNextChunk,
                      enableAnimation: true,
                    }}
                  >
                    {thinkingMarkdown}
                  </XMarkdown>
                ) : null}
              </Think>
            ) : null}

            {answerMarkdown ? (
              <XMarkdown
                paragraphTag="div"
                className={className}
                streaming={{
                  hasNextChunk,
                  enableAnimation: true,
                }}
              >
                {answerMarkdown}
              </XMarkdown>
            ) : null}
          </div>
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
