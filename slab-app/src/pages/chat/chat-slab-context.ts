import { createContext, useContext } from 'react';
import { useSlabChat } from './hooks/use-slab-chat';

export const SlabChatContext = createContext<ReturnType<typeof useSlabChat> | null>(null);

export const useSlabChatContext = () => {
  const context = useContext(SlabChatContext);
  if (!context) {
    throw new Error('useSlabChatContext must be used within SlabChatProvider');
  }
  return context;
};

export const DEFAULT_CONVERSATIONS_ITEMS = [
  {
    key: 'new',
    label: 'New Chat',
    group: 'Today',
  },
];
