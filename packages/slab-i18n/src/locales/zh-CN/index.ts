import type { LocaleSchema } from '../en-US';
import { common } from './common';
import { layouts } from './layouts';
import { pages } from './pages';
import { zhCNServer as server } from '../server';

export const zhCN = {
  common,
  layouts,
  pages,
  server,
} satisfies LocaleSchema;
