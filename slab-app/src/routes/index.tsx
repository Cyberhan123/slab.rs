import { useRoutes } from "react-router-dom";
import Chat from "@/pages/chat";
import About from "@/pages/about";
import { ThemePreview } from "@/components/theme-preview";
import Layout from "@/layouts/index";

function AppRoutes() {
  const routes = useRoutes([
    {
      path: '/',
      element: <Layout />, // 全局外壳
      children: [
        { index: true, element: <Chat /> }, // 默认子路由 (/)
        { path: 'about', element: <About /> }, // (/about)
      ],
    },
    { path: "/theme-preview", element: <ThemePreview /> }
  ]);

  return routes;
}

export default AppRoutes;