import {
  BotMessageSquare,
  ClipboardList,
  Film,
  Info,
  Mic,
  Package,
  Palette,
  Puzzle,
  Settings,
  Sparkles,
} from "lucide-react";
import type { ComponentType } from "react";

export type HeaderIcon = ComponentType<{
  className?: string;
}>;

export type HeaderMeta = {
  title: string;
  subtitle: string;
  icon: HeaderIcon;
};

export type HeaderMetaOverride = Partial<HeaderMeta>;

export const DEFAULT_HEADER_META: HeaderMeta = {
  title: "Slab",
  subtitle: "ML Inference Platform",
  icon: BotMessageSquare,
};

export const PAGE_HEADER_META = {
  about: {
    title: "About",
    subtitle: "Project and runtime information",
    icon: Info,
  },
  audio: {
    title: "Audio",
    subtitle: "Transcribe and process audio files",
    icon: Mic,
  },
  chat: {
    title: "Chat",
    subtitle: "Talk with AI models in one workspace",
    icon: BotMessageSquare,
  },
  hub: {
    title: "Hub",
    subtitle: "Models Repository",
    icon: Package,
  },
  image: {
    title: "Image",
    subtitle: "Generate and manage AI images",
    icon: Sparkles,
  },
  plugins: {
    title: "Plugins",
    subtitle: "Run workspace plugins with Extism runtime",
    icon: Puzzle,
  },
  settings: {
    title: "Settings",
    subtitle: "Configure app and backend options",
    icon: Settings,
  },
  setup: {
    title: "Setup",
    subtitle: "Initialize local runtime dependencies",
    icon: Package,
  },
  task: {
    title: "Tasks",
    subtitle: "Track and manage system tasks",
    icon: ClipboardList,
  },
  themePreview: {
    title: "Theme Preview",
    subtitle: "Preview UI components and design tokens",
    icon: Palette,
  },
  video: {
    title: "Video",
    subtitle: "Video tooling and processing",
    icon: Film,
  },
} satisfies Record<string, HeaderMeta>;
