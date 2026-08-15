"use client"

import { Button } from "@slab/components/button"
import { Badge } from "@slab/components/badge"
import { Spinner } from "@slab/components/spinner"
import { useTranslation } from "@slab/i18n"
import {
  CheckIcon,
  FilePenIcon,
  ListChecksIcon,
  ShieldAlertIcon,
  TerminalIcon,
  XIcon,
} from "lucide-react"
import { useState } from "react"

import type { ApprovalScope } from "@slab/core/harness"
import type { ApprovalRequest } from "@slab/core/harness"
import { PlanCardBody } from "./message/message-tool-plan-part"

const changeTypeVariant: Record<string, "default" | "secondary" | "destructive"> = {
  add: "secondary",
  edit: "default",
  delete: "destructive",
}

type ScopeChoice = {
  scope: ApprovalScope
  approved: boolean
  label: string
  icon: typeof CheckIcon
  variant: "default" | "outline"
}

const ALL_SCOPES: ScopeChoice[] = [
  { scope: "run_once", approved: true, label: "pages.assistant.approval.runOnce", icon: CheckIcon, variant: "default" },
  { scope: "always_in_workspace", approved: true, label: "pages.assistant.approval.alwaysInWorkspace", icon: CheckIcon, variant: "outline" },
  { scope: "always", approved: true, label: "pages.assistant.approval.always", icon: CheckIcon, variant: "outline" },
  { scope: "deny", approved: false, label: "pages.assistant.actions.reject", icon: XIcon, variant: "outline" },
]

export function ApprovalCard({
  approval,
  onResolve,
}: {
  approval: ApprovalRequest
  onResolve: (itemId: string, approved: boolean, scope: ApprovalScope) => Promise<void> | void
}) {
  const { t } = useTranslation()
  const [pendingAction, setPendingAction] = useState<string | null>(null)

  const handle = async (approved: boolean, scope: ApprovalScope, label: string) => {
    setPendingAction(label)
    try {
      await onResolve(approval.itemId, approved, scope)
    } finally {
      setPendingAction(null)
    }
  }

  // Prefer the server-advertised scopes; fall back to a simple approve/reject
  // (approve = run-once, reject = deny) for older servers.
  const choices: ScopeChoice[] =
    approval.allowedScopes && approval.allowedScopes.length > 0
      ? ALL_SCOPES.filter((choice) => approval.allowedScopes!.includes(choice.scope))
      : [
          { scope: "run_once", approved: true, label: "pages.assistant.actions.approve", icon: CheckIcon, variant: "default" },
          { scope: "deny", approved: false, label: "pages.assistant.actions.reject", icon: XIcon, variant: "outline" },
        ]

  const isCommand = approval.kind === "command"
  const isPlan = approval.kind === "plan"

  return (
    <div
      className="rounded-md border border-yellow-500/40 bg-yellow-500/5 p-3"
      data-testid={isPlan ? "assistant-approval-plan" : undefined}
    >
      <div className="flex items-center gap-2 text-sm font-medium">
        <ShieldAlertIcon className="size-4 text-yellow-600" />
        <span>{t("pages.assistant.approval.title")}</span>
        <Badge variant="secondary" className="gap-1">
          {isPlan ? (
            <ListChecksIcon className="size-3" />
          ) : isCommand ? (
            <TerminalIcon className="size-3" />
          ) : (
            <FilePenIcon className="size-3" />
          )}
          {isPlan
            ? t("pages.assistant.approval.plan")
            : isCommand
              ? t("pages.assistant.approval.command")
              : t("pages.assistant.approval.fileChange")}
        </Badge>
      </div>

      {approval.reason ? (
        <p className="mt-2 text-muted-foreground text-xs">{approval.reason}</p>
      ) : null}

      <div className="mt-2 space-y-2">
        {isPlan && approval.planSnapshot ? (
          <PlanCardBody plan={approval.planSnapshot} />
        ) : isCommand ? (
          <pre className="overflow-x-auto rounded-md bg-muted/60 p-2 font-mono text-xs">
            <span className="text-muted-foreground">$ cd {approval.cwd ?? "."}</span>
            {"\n"}
            <span>{approval.command ?? "(shell)"}</span>
          </pre>
        ) : (
          <ul className="space-y-1">
            {(approval.changes ?? []).map((change) => (
              <li key={`${change.type}:${change.path}`} className="rounded-md bg-muted/60 p-2 text-xs">
                <div className="flex items-center gap-2">
                  <Badge variant={changeTypeVariant[change.type] ?? "secondary"}>
                    {change.type}
                  </Badge>
                  <code className="font-mono">{change.path}</code>
                </div>
                {change.diff ? (
                  <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap font-mono text-[11px] text-muted-foreground">
                    {change.diff}
                  </pre>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-2">
        {choices.map((choice) => {
          const Icon = choice.icon
          return (
            <Button
              key={choice.scope}
              data-testid={`assistant-approval-${choice.scope}`}
              size="sm"
              variant={choice.variant}
              disabled={pendingAction !== null}
              onClick={() => {
                void handle(choice.approved, choice.scope, choice.label)
              }}
            >
              {pendingAction === choice.label ? (
                <Spinner className="size-3.5" />
              ) : (
                <Icon className="size-3.5" />
              )}
              {t(choice.label)}
            </Button>
          )
        })}
      </div>
    </div>
  )
}
