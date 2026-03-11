import { Outlet } from "react-router-dom";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { AppSidebar } from "@/layouts/sidebar"
import { CSSProperties } from "react";
import Header from "@/layouts/header";
import FooterStatusBar from "@/layouts/footer-status-bar";

export default function Layout() {
    return (
        <div className="flex h-screen w-full flex-col overflow-hidden">
            <SidebarProvider
                defaultOpen={false}
                className="min-h-0 flex-1"
                style={
                    {
                        "--sidebar-width": "12rem",
                        "--sidebar-width-icon": "3.25rem",
                        "--sidebar-width-mobile": "16rem",
                    } as CSSProperties
                }
            >
                <div className="flex min-h-0 flex-1 w-full overflow-hidden">
                    <AppSidebar />
                    <SidebarInset className="flex h-full min-w-0 flex-col">
                        <Header />
                        <main className="flex-1 overflow-hidden">
                            <Outlet />
                        </main>
                    </SidebarInset>
                </div>
            </SidebarProvider>
            <FooterStatusBar />
        </div>
    )
}
