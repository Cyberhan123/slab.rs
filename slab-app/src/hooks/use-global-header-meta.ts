import { useEffect, useMemo } from "react";
import {
  BotMessageSquare,
  ClipboardList,
  Film,
  Info,
  Mic,
  Package,
  Palette,
  Sparkles,
  Settings,
  type LucideIcon,
} from "lucide-react";
import { matchPath, useLocation } from "react-router-dom";

export type HeaderMeta = {
  title: string;
  description: string;
  icon: LucideIcon;
};

const DEFAULT_HEADER_META: HeaderMeta = {
  title: "Slab",
  description: "ML Inference Platform",
  icon: BotMessageSquare,
};

const HEADER_META_BY_ROUTE: Array<{
  path: string;
  end?: boolean;
  meta: HeaderMeta;
}> = [
  {
    path: "/",
    end: true,
    meta: {
      title: "Chat",
      description: "Talk with AI models in one workspace",
      icon: BotMessageSquare,
    },
  },
  {
    path: "/image",
    meta: {
      title: "Image",
      description: "Generate and manage AI images",
      icon: Sparkles,
    },
  },
  {
    path: "/audio",
    meta: {
      title: "Audio",
      description: "Transcribe and process audio files",
      icon: Mic,
    },
  },
  {
    path: "/video",
    meta: {
      title: "Video",
      description: "Video tooling and processing",
      icon: Film,
    },
  },
  {
    path: "/hub",
    meta: {
      title: "Hub",
      description: "Model and backend operations center",
      icon: Package,
    },
  },
  {
    path: "/task",
    meta: {
      title: "Tasks",
      description: "Track and manage system tasks",
      icon: ClipboardList,
    },
  },
  {
    path: "/settings",
    meta: {
      title: "Settings",
      description: "Configure app and backend options",
      icon: Settings,
    },
  },
  {
    path: "/about",
    meta: {
      title: "About",
      description: "Project and runtime information",
      icon: Info,
    },
  },
  {
    path: "/theme-preview",
    meta: {
      title: "Theme Preview",
      description: "Preview UI components and design tokens",
      icon: Palette,
    },
  },
];

function resolveHeaderMeta(pathname: string): HeaderMeta {
  const matched = HEADER_META_BY_ROUTE.find((item) =>
    Boolean(matchPath({ path: item.path, end: item.end ?? true }, pathname)),
  );

  return matched?.meta ?? DEFAULT_HEADER_META;
}

export function useGlobalHeaderMeta(): HeaderMeta {
  const { pathname } = useLocation();

  const meta = useMemo(() => resolveHeaderMeta(pathname), [pathname]);

  useEffect(() => {
    document.title = `${meta.title} | Slab`;
  }, [meta.title]);

  return meta;
}
