import { createBrowserRouter } from "react-router-dom";

import App from "./App";

export function createDesktopRoutes() {
  return [
    {
      path: "*",
      element: <App />,
    },
  ];
}

export function createDesktopBrowserRouter() {
  return createBrowserRouter(createDesktopRoutes());
}
