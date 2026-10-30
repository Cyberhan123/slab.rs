import * as React from "react"

const MOBILE_BREAKPOINT = 768

export function useIsMobile() {
  // Lazy init from the viewport (external state readable at mount), so the
  // first render already carries the right value and the effect below only
  // subscribes to later changes. `undefined` keeps a non-window environment
  // on the upstream "not mobile" default.
  const [isMobile, setIsMobile] = React.useState<boolean | undefined>(() =>
    typeof window === "undefined" ? undefined : window.innerWidth < MOBILE_BREAKPOINT
  )

  React.useEffect(() => {
    const mql = window.matchMedia(`(max-width: ${MOBILE_BREAKPOINT - 1}px)`)
    const onChange = () => {
      setIsMobile(window.innerWidth < MOBILE_BREAKPOINT)
    }
    mql.addEventListener("change", onChange)
    return () => mql.removeEventListener("change", onChange)
  }, [])

  return !!isMobile
}
