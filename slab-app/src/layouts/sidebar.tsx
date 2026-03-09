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
import { BotMessageSquare, ImageIcon, Mic, Film, Package, ClipboardList, Settings, type LucideIcon } from "lucide-react"

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
                <SidebarMenuButton asChild isActive={active}>
                    <Link to={item.to} aria-current={active ? "page" : undefined}>
                        <Icon />
                        <span>{item.label}</span>
                    </Link>
                </SidebarMenuButton>
            </SidebarMenuItem>
        );
    };

    return (
        <Sidebar variant="inset" collapsible="icon">
            <SidebarHeader className="text-center">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary">
                    <span className="text-sm font-bold text-primary-foreground">Slab</span>
                </div>
            </SidebarHeader>
            <SidebarContent>
                <SidebarMenu>
                    {primaryItems.map(renderItem)}
                </SidebarMenu>
            </SidebarContent>
            <SidebarFooter>
                <SidebarMenu>
                    {footerItems.map(renderItem)}
                </SidebarMenu>
            </SidebarFooter>
        </Sidebar>
    )
}
