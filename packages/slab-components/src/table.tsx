"use client"

import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "./lib/utils"

const tableVariants = cva("w-full caption-bottom text-sm", {
  variants: {
    variant: {
      default: "",
      roomy: "[&_td]:py-4 [&_th]:h-12 [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-[0.14em]",
    },
  },
  defaultVariants: {
    variant: "default",
  },
})

function Table({
  className,
  variant = "default",
  ...props
}: React.ComponentProps<"table"> & VariantProps<typeof tableVariants>) {
  return (
    <div
      data-slot="table-container"
      className="relative w-full overflow-x-auto"
    >
      <table
        data-slot="table"
        data-variant={variant}
        className={cn(tableVariants({ variant }), className)}
        {...props}
      />
    </div>
  )
}

function TableHeader({
  className,
  variant = "default",
  ...props
}: React.ComponentProps<"thead"> & {
  variant?: "default" | "soft-header"
}) {
  return (
    <thead
      data-slot="table-header"
      data-variant={variant}
      className={cn(
        "[&_tr]:border-b",
        variant === "soft-header" &&
          "[&_tr]:border-border/60 [&_tr]:bg-[var(--surface-soft)]",
        className
      )}
      {...props}
    />
  )
}

function TableBody({ className, ...props }: React.ComponentProps<"tbody">) {
  return (
    <tbody
      data-slot="table-body"
      className={cn("[&_tr:last-child]:border-0", className)}
      {...props}
    />
  )
}

function TableFooter({ className, ...props }: React.ComponentProps<"tfoot">) {
  return (
    <tfoot
      data-slot="table-footer"
      className={cn(
        "bg-muted/50 border-t font-medium [&>tr]:last:border-b-0",
        className
      )}
      {...props}
    />
  )
}

function TableRow({ className, ...props }: React.ComponentProps<"tr">) {
  return (
    <tr
      data-slot="table-row"
      className={cn(
        "hover:bg-muted/50 data-[state=selected]:bg-muted border-b transition-colors",
        className
      )}
      {...props}
    />
  )
}

function TableHead({
  className,
  variant = "default",
  ...props
}: React.ComponentProps<"th"> & {
  variant?: "default" | "soft-header"
}) {
  return (
    <th
      data-slot="table-head"
      data-variant={variant}
      className={cn(
        "text-foreground h-10 px-2 text-left align-middle font-medium whitespace-nowrap [&:has([role=checkbox])]:pr-0 [&>[role=checkbox]]:translate-y-[2px]",
        variant === "soft-header" && "px-4 text-[11px] font-semibold tracking-[0.14em] text-muted-foreground",
        className
      )}
      {...props}
    />
  )
}

function TableCell({
  className,
  variant = "default",
  ...props
}: React.ComponentProps<"td"> & {
  variant?: "default" | "roomy" | "sticky-actions"
}) {
  return (
    <td
      data-slot="table-cell"
      data-variant={variant}
      className={cn(
        "p-2 align-middle whitespace-nowrap [&:has([role=checkbox])]:pr-0 [&>[role=checkbox]]:translate-y-[2px]",
        variant === "roomy" && "px-4 py-4",
        variant === "sticky-actions" &&
          "sticky right-0 z-10 bg-[var(--surface-1)] text-right shadow-[-1px_0_0_color-mix(in_oklab,var(--border)_85%,transparent)]",
        className
      )}
      {...props}
    />
  )
}

function TableCaption({
  className,
  ...props
}: React.ComponentProps<"caption">) {
  return (
    <caption
      data-slot="table-caption"
      className={cn("text-muted-foreground mt-4 text-sm", className)}
      {...props}
    />
  )
}

export {
  Table,
  TableHeader,
  TableBody,
  TableFooter,
  TableHead,
  TableRow,
  TableCell,
  TableCaption,
  tableVariants,
}
