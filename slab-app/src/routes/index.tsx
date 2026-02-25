import { useRoutes } from "react-router-dom";
import Home from "@/pages/Home";
import About from "@/pages/About";
import { ThemePreview } from "@/components/theme-preview";

function AppRoutes() {
  const routes = useRoutes([
    { path: "/", element: <Home /> },
    { path: "/about", element: <About /> },
    { path: "/theme-preview", element: <ThemePreview /> }
  ]);

  return routes;
}

export default AppRoutes;