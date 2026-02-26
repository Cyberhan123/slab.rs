import { useRoutes } from "react-router-dom";
import Chat from "@/pages/chat";
import About from "@/pages/about";
import Settings from "@/pages/settings";
import { ThemePreview } from "@/components/theme-preview";
import Layout from "@/layouts";
import Hub from "@/pages/hub";
import Task from "@/pages/task";
import Audio from "@/pages/audio";
import Video from "@/pages/video";
import Image from "@/pages/image";

function AppRoutes() {
  const routes = useRoutes([
    {
      path: '/',
      element: <Layout />,
      children: [
        { index: true, element: <Chat /> },
        { path: 'image', element: <Image /> },
        { path: 'audio', element: <Audio /> },
        { path: 'video', element: <Video /> },
        { path: 'hub', element: <Hub /> },
        { path: 'task', element: <Task /> },
        { path: 'settings', element: <Settings /> },
        { path: 'about', element: <About /> },
      ],
    },
    { path: "/theme-preview", element: <ThemePreview /> }
  ]);

  return routes;
}

export default AppRoutes;