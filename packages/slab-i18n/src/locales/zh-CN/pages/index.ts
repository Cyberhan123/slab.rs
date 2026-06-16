import { audio } from './audio';
import { assistant } from './assistant';
import { hub } from './hub';
import { image } from './image';
import { plugins } from './plugins';
import { settings } from './settings';
import { setup } from './setup';
import { task } from './task';
import { video } from './video';
import { workspace } from './workspace';

export const pages = {
  audio,
  assistant,
  hub,
  image,
  plugins,
  settings,
  setup,
  task,
  video,
  workspace,
} as const;
