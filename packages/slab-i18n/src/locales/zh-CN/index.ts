import type { LocaleSchema } from '../en-US';
import { layouts } from './layouts';
import { pages } from './pages';
import { zhCNServer as server } from '../server';

export const zhCN = {
  layouts,
  pages,
  server,
} satisfies LocaleSchema;
