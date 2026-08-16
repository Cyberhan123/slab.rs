import { useMemo } from "react"
import {
    useTranslation,
} from "@slab/i18n"
function useGreeting() {
    const { t } = useTranslation()
    const greeting = useMemo(() => {
        const hour = new Date().getHours()

        if (hour < 12) {
            return t("pages.assistant.greeting.morning")
        }

        if (hour < 18) {
            return t("pages.assistant.greeting.afternoon")
        }

        return t("pages.assistant.greeting.evening")
    }, [t])
    return greeting
}

export { useGreeting }