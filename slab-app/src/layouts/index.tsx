import { Outlet } from "react-router-dom";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { AppSidebar } from "@/layouts/sidebar"
import { CSSProperties } from "react";
import Header from "@/layouts/header";

export default function Layout() {
    return (
        <SidebarProvider
            style={
                {
                    "--sidebar-width": "8rem",
                    "--sidebar-width-mobile": "16rem",
                } as CSSProperties
            }
        >
            <div className="flex h-screen w-full overflow-hidden">
                <AppSidebar />
                <SidebarInset className="flex flex-col h-full">
                    <Header />
                    <main className="flex-1 overflow-hidden">
                        <Outlet />
                    </main>
                </SidebarInset>
            </div>
        </SidebarProvider>
    )
}