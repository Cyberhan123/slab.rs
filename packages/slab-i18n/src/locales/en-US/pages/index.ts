import { audio } from './audio';
import { chat } from './chat';
import { hub } from './hub';
import { image } from './image';
import { plugins } from './plugins';
import { settings } from './settings';
import { task } from './task';
import { video } from './video';

export const pages = {
  audio,
  chat,
  hub,
  image,
  plugins,
  settings,
  task,
  video,
} as const;
