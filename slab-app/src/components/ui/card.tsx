import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

const cardVariants = cva(
  "text-card-foreground flex flex-col gap-6 rounded-[24px] border border-border/70 py-6",
  {
    variants: {
      variant: {
        default: "bg-card shadow-sm",
        soft: "bg-[var(--surface-1)] shadow-[0_16px_40px_-30px_color-mix(in_oklab,var(--foreground)_32%,transparent)]",
        elevated:
          "bg-[var(--surface-1)] shadow-[0_24px_60px_-38px_color-mix(in_oklab,var(--foreground)_40%,transparent)]",
        metric:
          "bg-[linear-gradient(180deg,color-mix(in_oklab,var(--surface-1)_85%,white)_0%,var(--surface-1)_100%)] shadow-[0_18px_44px_-30px_color-mix(in_oklab,var(--foreground)_32%,transparent)]",
        hero:
          "bg-[linear-gradient(180deg,color-mix(in_oklab,var(--brand-teal)_9%,var(--surface-1))_0%,var(--surface-1)_56%,color-mix(in_oklab,var(--brand-gold)_10%,var(--surface-1))_100%)] shadow-[0_28px_80px_-48px_color-mix(in_oklab,var(--brand-teal)_28%,transparent)]",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

function Card({
  className,
  variant = "default",
  ...props
}: React.ComponentProps<"div"> & VariantProps<typeof cardVariants>) {
  return (
    <div
      data-slot="card"
      data-variant={variant}
      className={cn(
        cardVariants({ variant }),
        className
      )}
      {...props}
    />
  )
}

function CardHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-header"
      className={cn(
        "@container/card-header grid auto-rows-min grid-rows-[auto_auto] items-start gap-2 px-6 has-data-[slot=card-action]:grid-cols-[1fr_auto] [.border-b]:pb-6",
        className
      )}
      {...props}
    />
  )
}

function CardTitle({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-title"
      className={cn("leading-none font-semibold", className)}
      {...props}
    />
  )
}

function CardDescription({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-description"
      className={cn("text-muted-foreground text-sm", className)}
      {...props}
    />
  )
}

function CardAction({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-action"
      className={cn(
        "col-start-2 row-span-2 row-start-1 self-start justify-self-end",
        className
      )}
      {...props}
    />
  )
}

function CardContent({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-content"
      className={cn("px-6", className)}
      {...props}
    />
  )
}

function CardFooter({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-footer"
      className={cn("flex items-center px-6 [.border-t]:pt-6", className)}
      {...props}
    />
  )
}

export {
  Card,
  CardHeader,
  CardFooter,
  CardTitle,
  CardAction,
  CardDescription,
  CardContent,
  cardVariants,
}
