import { useMemo, useState } from "react"
import {
    useTranslation,
} from "@slab/i18n"
function useGreeting() {
    const { t } = useTranslation()
    // Read the clock once per mount in a lazy state initializer: initializers
    // run a single time, so render stays pure (react(purity)) while the
    // greeting still reflects the mount time.
    const [hour] = useState(() => new Date().getHours())
    const greeting = useMemo(() => {
        if (hour < 12) {
            return t("pages.assistant.greeting.morning")
        }

        if (hour < 18) {
            return t("pages.assistant.greeting.afternoon")
        }

        return t("pages.assistant.greeting.evening")
    }, [t, hour])
    return greeting
}

export { useGreeting }
