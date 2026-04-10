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

export type HeaderControl = HeaderSelectControl;

export const HEADER_SELECT_KEYS = {
  audioModel: 'audio:model',
  chatModel: 'chat:model',
  imageModel: 'image:model',
  videoModel: 'video:model',
} as const;
