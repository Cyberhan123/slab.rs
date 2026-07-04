import { BotMessageSquare } from 'lucide-react';
import type { ComponentType } from 'react';

export type HeaderIcon = ComponentType<{
  className?: string;
}>;

export type HeaderMeta = {
  title: string;
  subtitle: string;
  icon: HeaderIcon;
};

export const DEFAULT_HEADER_META: HeaderMeta = {
  title: 'Slab',
  subtitle: 'ML Inference Platform',
  icon: BotMessageSquare,
};

export type HeaderSelectOption = {
  id: string;
  label: string;
  disabled?: boolean;
};

export type HeaderSelectControl = {
  type: 'select';
  value: string;
  options: HeaderSelectOption[];
  onValueChange: (value: string) => void;
  groupLabel?: string;
  placeholder?: string;
  loading?: boolean;
  disabled?: boolean;
  emptyLabel?: string;
};

export type HeaderSearchControl = {
  type: 'search';
  value: string;
  onValueChange: (value: string) => void;
  placeholder?: string;
  ariaLabel?: string;
  disabled?: boolean;
};

export type HeaderControl = HeaderSelectControl;

export const HEADER_SELECT_KEYS = {
  assistantModel: 'assistant:model',
  audioModel: 'audio:model',
  imageModel: 'image:model',
  videoModel: 'video:model',
} as const;
