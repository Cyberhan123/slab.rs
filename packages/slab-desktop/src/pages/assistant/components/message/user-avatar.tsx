import { UserRound } from "lucide-react"
import {
    Avatar,
    AvatarFallback,
    AvatarImage,
} from "@slab/components/avatar"

function UserAvatar({ name, url }: { name: string, url?: string }) {
    if (!url) {
        return <Avatar>
            <AvatarFallback><UserRound /></AvatarFallback>
        </Avatar>
    }
    return <Avatar>
        <AvatarImage src={url} alt={`@${name}`} />
        <AvatarFallback>{name}</AvatarFallback>
    </Avatar>
}

export default UserAvatar