import { OpenAIOutlined } from '@ant-design/icons';
import { Sender } from '@ant-design/x';
import { GetRef } from 'antd';
import { useRef } from 'react';
import { CheckCircle2, CloudDownload, Loader2 } from 'lucide-react';
import { Select, SelectContent, SelectItem, SelectTrigger } from '@/components/ui/select';
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

  const selectedModel = modelOptions.find((option) => option.id === selectedModelId);

  const renderModelStatusIcon = (option: ModelOption) => {
    if (option.pending) {
      return <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />;
    }
    if (option.downloaded) {
      return <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600" />;
    }
    return <CloudDownload className="h-3.5 w-3.5 text-muted-foreground" />;
  };

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
                    value={selectedModelId}
                    onValueChange={onModelChange}
                    disabled={modelDisabled || modelLoading}
                  >
                    <SelectTrigger className="h-8 min-w-[220px] rounded-full text-xs">
                      <span className="flex min-w-0 items-center gap-2">
                        {selectedModel ? (
                          renderModelStatusIcon(selectedModel)
                        ) : (
                          <CloudDownload className="h-3.5 w-3.5 text-muted-foreground" />
                        )}
                        <span className="truncate">
                          {selectedModel?.label ?? (modelLoading ? 'Loading models...' : 'Select model')}
                        </span>
                      </span>
                    </SelectTrigger>
                    <SelectContent>
                      {modelOptions.length === 0 ? (
                        <div className="px-2 py-1.5 text-sm text-muted-foreground">No chat models</div>
                      ) : (
                        modelOptions.map((option) => (
                          <SelectItem key={option.id} value={option.id}>
                            <span className="flex min-w-0 items-center gap-2">
                              {renderModelStatusIcon(option)}
                              <span className="truncate">{option.label}</span>
                            </span>
                          </SelectItem>
                        ))
                      )}
                    </SelectContent>
                  </Select>
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
