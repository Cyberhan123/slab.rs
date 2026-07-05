"use client"

import { useState, type SubmitEvent } from "react"
import {
    InputGroup,
    InputGroupAddon,
    InputGroupButton,
    InputGroupTextarea,
} from "@slab/components/input-group"
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
    DropdownMenuGroup,
    DropdownMenuLabel
} from "@slab/components/dropdown-menu"

import { Spinner } from "@slab/components/spinner"
import {
    ToggleGroup,
    ToggleGroupItem,
} from "@slab/components/toggle-group"

import { Switch } from "@slab/components/switch"

import {
    ArrowUpIcon,
    Sparkle,
    PaperclipIcon,
    PlusIcon,
    Slash,
    File,
    Brain,
    Dot
} from "lucide-react"

type SenderProps = {
    onSubmit: (message: string, event?: SubmitEvent<HTMLFormElement>) => Promise<void> | void
    loading?: boolean
}

function Sender({ onSubmit, loading = false }: SenderProps) {
    const [value, setValue] = useState("")

    return <form
        onSubmit={(e) => {
            e.preventDefault()
            const message = value.trim()

            if (!message || loading) {
                return
            }

            void Promise.resolve(onSubmit(message, e))
                .then(() => {
                    setValue("")
                })
                .catch(() => {})
        }}
        className="w-full"
    >
        <InputGroup>
            <InputGroupTextarea
                aria-label="Message"
                disabled={loading}
                onChange={(event) => setValue(event.target.value)}
                onKeyDown={(event) => {
                    if (event.key === "Enter" && !event.shiftKey) {
                        event.preventDefault()
                        event.currentTarget.form?.requestSubmit()
                    }
                }}
                placeholder="Ask anything"
                rows={3}
                value={value}
            />
            <InputGroupAddon align="block-end" className="pt-1">
                <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                        <InputGroupButton
                            aria-label="Add files"
                            type="button"
                            size="icon-sm"
                            variant="outline"
                        >
                            <PlusIcon />
                        </InputGroupButton>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent
                        align="start"
                        side="top"
                        className="w-44"
                    >
                        <DropdownMenuItem>
                            <PaperclipIcon />
                            Add Files or Dir
                        </DropdownMenuItem>
                        <DropdownMenuSeparator />
                        <DropdownMenuItem>
                            <File />
                            Add context
                        </DropdownMenuItem>
                    </DropdownMenuContent>
                </DropdownMenu>
                <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                        <InputGroupButton
                            aria-label="Add files"
                            type="button"
                            size="icon-sm"
                            variant="outline"
                        >
                            <Slash />
                        </InputGroupButton>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent
                        align="start"
                        side="top"
                        className="w-44"
                    >
                        <DropdownMenuGroup>
                            <DropdownMenuLabel>Model</DropdownMenuLabel>
                            <DropdownMenuItem>
                                <Brain />
                                Effort
                                <ToggleGroup variant="outline" type="single">
                                    <ToggleGroupItem value="low" aria-label="Toggle low">
                                        <Dot /> Low
                                    </ToggleGroupItem>
                                    <ToggleGroupItem value="medium" aria-label="Toggle medium">
                                        <Dot />  Medium
                                    </ToggleGroupItem>
                                    <ToggleGroupItem value="high" aria-label="Toggle high">
                                        <Dot />  High
                                    </ToggleGroupItem>
                                </ToggleGroup>
                            </DropdownMenuItem>
                            <DropdownMenuSeparator />
                            <DropdownMenuItem>
                                <Sparkle />
                                Thinking
                                <Switch id="airplane-mode" />
                            </DropdownMenuItem>

                        </DropdownMenuGroup>
                        <DropdownMenuGroup>
                            <DropdownMenuLabel>Slash Commonds</DropdownMenuLabel>
                            <DropdownMenuItem>
                                /plan
                            </DropdownMenuItem>
                        </DropdownMenuGroup>
                    </DropdownMenuContent>
                </DropdownMenu>
                <InputGroupButton
                    aria-label="Send"
                    type="submit"
                    variant={"default"}
                    size="icon-sm"
                    disabled={loading || value.trim().length === 0}
                    className="ml-auto"
                >
                    {
                        loading ? <Spinner /> : <ArrowUpIcon />
                    }

                    <span className="sr-only">Send</span>
                </InputGroupButton>
            </InputGroupAddon>
        </InputGroup>
    </form>
}

export default Sender
