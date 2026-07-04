import { BotMessageSquare } from "lucide-react"
import {
    Avatar,
    AvatarFallback,
    AvatarImage,
} from "@slab/components/avatar"

function AgentAvatar({ name, url }: { name?: string, url?: string }) {
    if (!url) {
        return <Avatar>
            <AvatarFallback><BotMessageSquare /></AvatarFallback>
        </Avatar>
    }
    return <Avatar>
        <AvatarImage src={url} alt={`@${name}`} />
        <AvatarFallback>{name}</AvatarFallback>
    </Avatar>
}

export default AgentAvatar