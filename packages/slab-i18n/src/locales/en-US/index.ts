import { layouts } from './layouts';
import { pages } from './pages';
import { enUSServer as server } from '../server';

export const enUS = {
  layouts,
  pages,
  server,
} as const;

type LocaleMessages<T> = {
  [Key in keyof T]:
    T[Key] extends string
      ? string
      : T[Key] extends readonly unknown[]
        ? T[Key]
        : T[Key] extends object
          ? LocaleMessages<T[Key]>
          : T[Key];
};

export type LocaleSchema = LocaleMessages<typeof enUS>;
