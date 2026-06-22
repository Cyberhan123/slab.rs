"use client"

import * as React from "react"
import { Switch as SwitchPrimitive } from "radix-ui"

import { cn } from "./lib/utils"

function Switch({
  className,
  size = "default",
  variant = "default",
  ...props
}: React.ComponentProps<typeof SwitchPrimitive.Root> & {
  size?: "sm" | "default"
  variant?: "default" | "workspace"
}) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      data-size={size}
      data-variant={variant}
      className={cn(
        "focus-ring peer group/switch inline-flex shrink-0 items-center rounded-full border border-transparent transition-all duration-[var(--dur-180)] ease-out-expo outline-none disabled:cursor-not-allowed disabled:opacity-50 data-[size=default]:h-[1.15rem] data-[size=default]:w-8 data-[size=sm]:h-3.5 data-[size=sm]:w-6",
        "data-[variant=default]:data-[state=checked]:bg-primary data-[variant=default]:data-[state=unchecked]:bg-input data-[variant=default]:shadow-xs dark:data-[variant=default]:data-[state=unchecked]:bg-input/80",
        "data-[variant=workspace]:data-[state=checked]:bg-[var(--brand-teal)] data-[variant=workspace]:data-[state=unchecked]:bg-[var(--surface-selected)] data-[variant=workspace]:shadow-elevation-2",
        className
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className={cn(
          "bg-background dark:data-[state=unchecked]:bg-foreground dark:data-[state=checked]:bg-primary-foreground pointer-events-none block rounded-full ring-0 transition-transform group-data-[size=default]/switch:size-4 group-data-[size=sm]/switch:size-3 data-[state=checked]:translate-x-[calc(100%-2px)] data-[state=unchecked]:translate-x-0"
        )}
      />
    </SwitchPrimitive.Root>
  )
}

export { Switch }
