import { OpenAIOutlined } from '@ant-design/icons';
import { Sender } from '@ant-design/x';
import { GetRef, Select } from 'antd';
import { useRef } from 'react';
import { CheckCircle2, CloudDownload, Loader2 } from 'lucide-react';
import locale from '../local';
import { useStyle } from '../hooks/use-style';

type ModelOption = {
  id: string;
  label: string;
  downloaded: boolean;
  pending: boolean;
};

interface ChatInputProps {
  isRequesting: boolean;
  deepThink: boolean;
  setDeepThink: (value: boolean) => void;
  onSubmit: (value: string) => void | Promise<void>;
  onCancel: () => void;
  curConversation: string;
  modelOptions: ModelOption[];
  selectedModelId: string;
  onModelChange: (id: string) => void;
  modelLoading?: boolean;
  modelDisabled?: boolean;
}

export const ChatInput = ({
  isRequesting,
  deepThink,
  setDeepThink,
  onSubmit,
  onCancel,
  curConversation,
  modelOptions,
  selectedModelId,
  onModelChange,
  modelLoading = false,
  modelDisabled = false,
}: ChatInputProps) => {
  const senderRef = useRef<GetRef<typeof Sender>>(null);
  const styles = useStyle();

  const renderModelStatusIcon = (option: ModelOption) => {
    if (option.pending) {
      return <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />;
    }
    if (option.downloaded) {
      return <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600" />;
    }
    return <CloudDownload className="h-3.5 w-3.5 text-muted-foreground" />;
  };

  const selectOptions = modelOptions.map((option) => ({
    value: option.id,
    label: (
      <span className="flex min-w-0 items-center gap-2">
        {renderModelStatusIcon(option)}
        <span className="truncate">{option.label}</span>
      </span>
    ),
  }));

  return (
    <div className={styles.inputArea}>
      <div className="w-full space-y-4">
        <Sender
          suffix={false}
          ref={senderRef}
          key={curConversation}
          loading={isRequesting}
          onSubmit={(val) => {
            if (!val) return;
            void onSubmit(val);
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
                  <Select
                    value={selectedModelId || undefined}
                    onChange={onModelChange}
                    disabled={modelDisabled || modelLoading}
                    options={selectOptions}
                    optionLabelProp="label"
                    placeholder={modelLoading ? 'Loading models...' : 'Select model'}
                    notFoundContent="No chat models"
                    size="small"
                    popupMatchSelectWidth={false}
                    className="chat-model-select min-w-[220px]"
                    popupClassName="chat-model-select-dropdown"
                  />
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
