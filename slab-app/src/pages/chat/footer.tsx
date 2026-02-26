import { useContext } from 'react';
import { Actions } from '@ant-design/x';
import { SyncOutlined } from '@ant-design/icons';
import locale from './local';
import { ChatContext } from './chat-context';

type FooterProps = {
    id?: string;
    content: string;
    status?: string;
}

export const Footer: React.FC<FooterProps> = (props: FooterProps) => {
    const { id, content, status } = props;
    const context = useContext(ChatContext);
    const Items = [
        {
            key: 'retry',
            label: locale.retry,
            icon: <SyncOutlined />,
            onItemClick: () => {
                if (id) {
                    context?.onReload?.(id, {
                        userAction: 'retry',
                    });
                }
            },
        },
        {
            key: 'copy',
            actionRender: <Actions.Copy text={content} />,
        },
    ];
    return status !== 'updating' && status !== 'loading' ? (
        <div style={{ display: 'flex' }}>{id && <Actions items={Items} />}</div>
    ) : null;
};