import {
  Box,
  Boxes,
  Braces,
  Code2,
  PlugZap,
  TerminalSquare,
  type LucideIcon,
} from 'lucide-react';

import type { components } from '@/lib/api/v1.d.ts';

export type PluginTone = 'teal' | 'gold' | 'slate' | 'blue';
export type PluginStatusKey = 'working' | 'invalid' | 'running' | 'idle' | 'disabled';
export type PluginSummaryMessage = {
  key: string;
  options?: Record<string, unknown>;
  raw?: string;
};
export type PluginRecord = components['schemas']['PluginResponse'];

export const PLUGIN_TONES: PluginTone[] = ['gold', 'teal', 'slate', 'blue'];
export const PLUGIN_ICONS: LucideIcon[] = [Braces, Code2, Boxes, TerminalSquare, PlugZap, Box];
export const INSTALLED_SKELETON_KEYS = [
  'plugin-skeleton-one',
  'plugin-skeleton-two',
  'plugin-skeleton-three',
  'plugin-skeleton-four',
] as const;

export function isPluginRunning(plugin: PluginRecord) {
  return plugin.runtimeStatus.toLowerCase() === 'running';
}

export function pluginSummaryMessage(plugin: PluginRecord): PluginSummaryMessage {
  if (!plugin.valid) {
    return plugin.error
      ? { key: 'pages.plugins.summary.invalidManifest', raw: plugin.error }
      : { key: 'pages.plugins.summary.invalidManifest' };
  }
  if (isPluginRunning(plugin)) return { key: 'pages.plugins.summary.runtimeActive' };
  if (!plugin.enabled) return { key: 'pages.plugins.summary.disabled' };
  if (plugin.updateAvailable) {
    return { key: 'pages.plugins.summary.updateReady', options: { version: plugin.version } };
  }
  if (plugin.hasWasm && plugin.uiEntry) return { key: 'pages.plugins.summary.webviewRuntime' };
  if (plugin.hasWasm) return { key: 'pages.plugins.summary.runtimeHooks' };
  if (plugin.uiEntry) return { key: 'pages.plugins.summary.uiEntry' };
  return {
    key: 'pages.plugins.summary.sourceVersion',
    options: { sourceKind: plugin.sourceKind, version: plugin.version },
  };
}

export function pluginSearchText(plugin: PluginRecord) {
  return [
    plugin.name,
    plugin.id,
    plugin.version,
    plugin.error,
    plugin.lastError,
    plugin.sourceKind,
    plugin.sourceRef,
    plugin.runtimeStatus,
    plugin.availableVersion,
    plugin.installedVersion,
    plugin.hasWasm ? 'wasm' : null,
    plugin.uiEntry ? 'ui' : null,
    ...plugin.allowHosts,
  ]
    .filter(Boolean)
    .join(' ')
    .toLowerCase();
}

export function toneSurfaceClassName(tone: PluginTone) {
  switch (tone) {
    case 'teal':
      return 'bg-[color-mix(in_oklab,var(--brand-teal)_12%,var(--shell-card))]';
    case 'gold':
      return 'bg-[color-mix(in_oklab,var(--brand-gold)_12%,var(--shell-card))]';
    case 'blue':
      return 'bg-[color-mix(in_oklab,var(--chart-2)_10%,var(--shell-card))]';
    case 'slate':
    default:
      return 'bg-[color-mix(in_oklab,var(--shell-rail-label)_12%,var(--shell-card))]';
  }
}

export function toneTextClassName(tone: PluginTone) {
  switch (tone) {
    case 'teal':
      return 'text-[var(--brand-teal)]';
    case 'gold':
      return 'text-[var(--brand-gold)]';
    case 'blue':
      return 'text-[var(--chart-2)]';
    case 'slate':
    default:
      return 'text-muted-foreground';
  }
}
