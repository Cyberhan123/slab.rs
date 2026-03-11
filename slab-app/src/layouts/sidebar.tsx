import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarMenuItem,
    SidebarHeader,
    SidebarMenu,
    SidebarMenuButton
} from "@/components/ui/sidebar"
import { Link, useLocation } from "react-router-dom"
import { BotMessageSquare, ImageIcon, Mic, Film, Package, ClipboardList, Settings, Puzzle, type LucideIcon } from "lucide-react"

type SidebarItem = {
    to: string;
    label: string;
    icon: LucideIcon;
    end?: boolean;
};

const primaryItems: SidebarItem[] = [
    { to: "/", label: "Chat", icon: BotMessageSquare, end: true },
    { to: "/image", label: "Image", icon: ImageIcon },
    { to: "/audio", label: "Audio", icon: Mic },
    { to: "/video", label: "Video", icon: Film },
    { to: "/hub", label: "Hub", icon: Package },
    { to: "/plugins", label: "Plugins", icon: Puzzle },
    { to: "/task", label: "Tasks", icon: ClipboardList },
];

const footerItems: SidebarItem[] = [
    { to: "/settings", label: "Settings", icon: Settings },
];

const isPathActive = (pathname: string, to: string, end = false) => {
    if (end) {
        return pathname === to;
    }
    return pathname === to || pathname.startsWith(`${to}/`);
};

export function AppSidebar() {
    const { pathname } = useLocation();

    const renderItem = (item: SidebarItem) => {
        const Icon = item.icon;
        const active = isPathActive(pathname, item.to, item.end);

        return (
            <SidebarMenuItem key={item.to}>
                <SidebarMenuButton
                    asChild
                    isActive={active}
                    tooltip={item.label}
                    className="h-9 rounded-md group-data-[collapsible=icon]:size-9! group-data-[collapsible=icon]:justify-center"
                >
                    <Link to={item.to} aria-current={active ? "page" : undefined}>
                        <Icon />
                        <span>{item.label}</span>
                    </Link>
                </SidebarMenuButton>
            </SidebarMenuItem>
        );
    };

    return (
        <Sidebar
            variant="sidebar"
            collapsible="icon"
            className="border-r border-sidebar-border/70 bg-sidebar/95 backdrop-blur md:bottom-7 md:h-[calc(100svh-1.75rem)]"
        >
            <SidebarHeader className="items-center border-b border-sidebar-border/60 px-2 py-2">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md border border-primary/30 bg-primary/10">
                    <span className="text-[11px] font-semibold tracking-wide text-primary">Slab</span>
                </div>
            </SidebarHeader>
            <SidebarContent className="px-2 py-2">
                <SidebarMenu className="gap-1.5">
                    {primaryItems.map(renderItem)}
                </SidebarMenu>
            </SidebarContent>
            <SidebarFooter className="border-t border-sidebar-border/60 px-2 py-2">
                <SidebarMenu className="gap-1.5">
                    {footerItems.map(renderItem)}
                </SidebarMenu>
            </SidebarFooter>
        </Sidebar>
    )
}
