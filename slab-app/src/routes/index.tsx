import { lazy, Suspense } from "react";
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
import Plugins from "@/pages/plugins";
import { Spinner } from "@/components/ui/spinner";
import { GlobalHeaderProvider } from "@/layouts/global-header-provider";

// Lazy-load the setup page so it doesn't bloat the main bundle.
const SetupPage = lazy(() => import("@/pages/setup"));

function AppRoutes() {
  const routes = useRoutes([
    {
      // The setup wizard lives outside the main Layout so it gets a clean,
      // full-screen canvas.  It navigates to "/" once setup is complete.
      path: '/setup',
      element: (
        <GlobalHeaderProvider>
          <Suspense
            fallback={
              <div className="flex h-screen items-center justify-center">
                <Spinner className="h-8 w-8" />
              </div>
            }
          >
            <SetupPage />
          </Suspense>
        </GlobalHeaderProvider>
      ),
    },
    {
      path: '/',
      element: <Layout />,
      children: [
        { index: true, element: <Chat /> },
        { path: 'image', element: <Image /> },
        { path: 'audio', element: <Audio /> },
        { path: 'video', element: <Video /> },
        { path: 'hub', element: <Hub /> },
        { path: 'plugins', element: <Plugins /> },
        { path: 'task', element: <Task /> },
        { path: 'settings', element: <Settings /> },
        { path: 'about', element: <About /> },
      ],
    },
    { path: "/theme-preview", element: <ThemePreview /> },
  ]);

  return routes;
}

export default AppRoutes;
