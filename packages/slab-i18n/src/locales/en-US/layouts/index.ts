import { errorBoundary } from './error-boundary';
import { footerStatusBar } from './footer-status-bar';
import { header } from './header';
import { sidebar } from './sidebar';

export const layouts = {
  errorBoundary,
  footerStatusBar,
  header,
  sidebar,
} as const;
