"use client"

/**
 * Animated text shimmer, re-implemented from the
 * `example-full-message/shimmer.tsx` reference (that directory must not be
 * imported). Used by the reasoning trigger while a thought is streaming.
 */

import { cn } from "@slab/ui/lib/utils"
import { motion } from "motion/react"
import type { CSSProperties, ElementType } from "react"
import { memo, useMemo } from "react"

// Motion components are created at module scope — creating them during render
// would remount the animation on every render (react-compiler lint).
const MOTION_ELEMENTS = {
  p: motion.create("p"),
  span: motion.create("span"),
  div: motion.create("div"),
} as const

type MotionElement = keyof typeof MOTION_ELEMENTS

export interface TextShimmerProps {
  children: string
  as?: ElementType
  className?: string
  duration?: number
  spread?: number
}

const ShimmerComponent = ({
  children,
  as = "p",
  className,
  duration = 2,
  spread = 2,
}: TextShimmerProps) => {
  const MotionComponent =
    (typeof as === "string" && as in MOTION_ELEMENTS
      ? MOTION_ELEMENTS[as as MotionElement]
      : MOTION_ELEMENTS.p)
  const dynamicSpread = useMemo(() => (children?.length ?? 0) * spread, [children, spread])

  return (
    <MotionComponent
      animate={{ backgroundPosition: "0% center" }}
      className={cn(
        "relative inline-block bg-[length:250%_100%,auto] bg-clip-text text-transparent",
        "[--bg:linear-gradient(90deg,#0000_calc(50%-var(--spread)),var(--color-background),#0000_calc(50%+var(--spread)))] [background-repeat:no-repeat,padding-box]",
        className,
      )}
      initial={{ backgroundPosition: "100% center" }}
      style={
        {
          "--spread": `${dynamicSpread}px`,
          backgroundImage:
            "var(--bg), linear-gradient(var(--color-muted-foreground), var(--color-muted-foreground))",
        } as CSSProperties
      }
      transition={{ duration, ease: "linear", repeat: Number.POSITIVE_INFINITY }}
    >
      {children}
    </MotionComponent>
  )
}

export const Shimmer = memo(ShimmerComponent)
