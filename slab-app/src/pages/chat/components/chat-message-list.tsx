import { Bubble, BubbleListProps } from '@ant-design/x';
import { BubbleListRef } from '@ant-design/x/es/bubble';
import { useRef, useState } from 'react';
import XMarkdown from '@ant-design/x-markdown';
import { Footer } from './footer';
import { useStyle } from '../hooks/use-style';

interface ChatMessageListProps {
  messages: any[];
  className: string;
  onReload: (id: string | number, requestParams: any, opts?: any) => void;
}

type ParsedThinkingContent = {
  thinking: string | null;
  answer: string;
};

function parseThinkingContent(rawContent: string): ParsedThinkingContent {
  const openTagIndex = rawContent.indexOf('<think');
  if (openTagIndex < 0) {
    return { thinking: null, answer: rawContent };
  }

  const openTagEnd = rawContent.indexOf('>', openTagIndex);
  if (openTagEnd < 0) {
    return { thinking: null, answer: rawContent };
  }

  const closeTag = '</think>';
  const closeTagIndex = rawContent.indexOf(closeTag, openTagEnd + 1);

  if (closeTagIndex < 0) {
    return {
      thinking: rawContent.slice(openTagEnd + 1).trimStart(),
      answer: rawContent.slice(0, openTagIndex).trimEnd(),
    };
  }

  const thinking = rawContent.slice(openTagEnd + 1, closeTagIndex).trim();
  const before = rawContent.slice(0, openTagIndex);
  const after = rawContent.slice(closeTagIndex + closeTag.length);

  return {
    thinking: thinking || null,
    answer: `${before}${after}`.trimStart(),
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

  const toggleThinking = (messageKey: string) => {
    setThinkingExpandedByMessage((prev) => ({
      ...prev,
      [messageKey]: !prev[messageKey],
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
        const { thinking, answer } = parseThinkingContent(rawContent);
        const answerMarkdown = answer.replace(/\n\n/g, '<br/><br/>');
        const thinkingMarkdown = (thinking ?? '').replace(/\n\n/g, '<br/><br/>');

        return (
          <div className="space-y-3">
            {thinking ? (
              <div className="rounded-md border border-amber-200 bg-amber-50/60 px-3 py-2">
                <div className="mb-1 flex items-center justify-between gap-3">
                  <div className="text-[11px] font-medium tracking-wide text-amber-700">
                    Thinking
                  </div>
                  <button
                    type="button"
                    className="text-[11px] font-medium text-amber-700 underline decoration-dotted underline-offset-2 transition-opacity hover:opacity-80"
                    onClick={() => toggleThinking(messageKey)}
                  >
                    {thinkingExpanded ? 'Hide' : 'Show'}
                  </button>
                </div>
                {thinkingExpanded ? (
                  <XMarkdown
                    paragraphTag="div"
                    className={className}
                    streaming={{
                      hasNextChunk: info.status === 'updating',
                      enableAnimation: true,
                    }}
                  >
                    {thinkingMarkdown}
                  </XMarkdown>
                ) : (
                  <div className="text-xs text-amber-700/80">Reasoning is collapsed</div>
                )}
              </div>
            ) : null}

            {answerMarkdown ? (
              <XMarkdown
                paragraphTag="div"
                className={className}
                streaming={{
                  hasNextChunk: info.status === 'updating',
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
