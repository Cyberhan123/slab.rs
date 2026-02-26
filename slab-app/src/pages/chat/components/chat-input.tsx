import { OpenAIOutlined } from '@ant-design/icons';
import { Sender, SenderProps } from '@ant-design/x';
import { GetRef } from 'antd';
import { useRef } from 'react';
import locale from '../local';
import { useStyle } from '../hooks/use-style';

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

interface ChatInputProps {
  isRequesting: boolean;
  deepThink: boolean;
  setDeepThink: (value: boolean) => void;
  onSubmit: (value: string) => void;
  onCancel: () => void;
  curConversation: string;
}

export const ChatInput = ({
  isRequesting,
  deepThink,
  setDeepThink,
  onSubmit,
  onCancel,
  curConversation
}: ChatInputProps) => {
  const senderRef = useRef<GetRef<typeof Sender>>(null);
  const styles = useStyle();

  return (
    <div className={styles.inputArea}>
      <div className="w-full space-y-4">
        <Sender
          suffix={false}
          ref={senderRef}
          key={curConversation}
          slotConfig={slotConfig}
          loading={isRequesting}
          onSubmit={(val) => {
            if (!val) return;
            onSubmit(val);
            senderRef.current?.clear?.();
          }}
          onCancel={() => {
            onCancel();
          }}
          placeholder={locale.placeholder}
          footer={(actionNode) => {
            return (
              <div className="flex justify-between items-center">
                <div className="flex gap-2 items-center">
                  <Sender.Switch
                    value={deepThink}
                    onChange={(checked: boolean) => {
                      setDeepThink(checked);
                    }}
                    icon={<OpenAIOutlined className="size-4" />}
                  >
                    {locale.deepThink}
                  </Sender.Switch>
                </div>
                <div className="flex items-center">{actionNode}</div>
              </div>
            );
          }}
          autoSize={{ minRows: 3, maxRows: 6 }}
        />
      </div>
    </div>
  );
};
