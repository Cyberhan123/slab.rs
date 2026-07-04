import {
  Film,
  ImageIcon,
  Mic,
} from "lucide-react";

import Audio from "@/pages/audio";
import Image from "@/pages/image";
import Video from "@/pages/video";
import type { DesktopRouteObject } from "../route-meta";

export const mediaRoutes: DesktopRouteObject[] = [
  {
    path: "image",
    meta: {
      title: "Image",
      subtitle: "Generate and manage AI images",
      icon: ImageIcon,
      sidebar: {
        group: "primary",
        labelKey: "layouts.sidebar.items.image",
      },
    },
    element: <Image />,
  },
  {
    path: "video",
    meta: {
      title: "Video",
      subtitle: "Video tooling and processing",
      icon: Film,
      sidebar: {
        group: "primary",
        labelKey: "layouts.sidebar.items.video",
      },
    },
    element: <Video />,
  },
  {
    path: "audio",
    meta: {
      title: "Audio",
      subtitle: "Transcribe and process audio files",
      icon: Mic,
      sidebar: {
        group: "primary",
        labelKey: "layouts.sidebar.items.audio",
      },
    },
    element: <Audio />,
  },
];
