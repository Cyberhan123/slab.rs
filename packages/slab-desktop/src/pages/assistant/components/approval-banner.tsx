"use client"

import { Button } from "@slab/components/button"
import { Badge } from "@slab/components/badge"
import { Spinner } from "@slab/components/spinner"
import { useTranslation } from "@slab/i18n"
import {
  CheckIcon,
  FilePenIcon,
  ShieldAlertIcon,
  TerminalIcon,
  XIcon,
} from "lucide-react"
import { useState } from "react"

import type { ApprovalRequest } from "../hooks/use-harness-conversation"

const changeTypeVariant: Record<string, "default" | "secondary" | "destructive"> = {
  add: "secondary",
  edit: "default",
  delete: "destructive",
}

export function ApprovalCard({
  approval,
  onResolve,
}: {
  approval: ApprovalRequest
  onResolve: (itemId: string, approved: boolean) => Promise<void> | void
}) {
  const { t } = useTranslation()
  const [pendingAction, setPendingAction] = useState<"approve" | "reject" | null>(null)

  const handle = async (approved: boolean) => {
    setPendingAction(approved ? "approve" : "reject")
    try {
      await onResolve(approval.itemId, approved)
    } finally {
      setPendingAction(null)
    }
  }

  const isCommand = approval.kind === "command"

  return (
    <div className="rounded-md border border-yellow-500/40 bg-yellow-500/5 p-3">
      <div className="flex items-center gap-2 text-sm font-medium">
        <ShieldAlertIcon className="size-4 text-yellow-600" />
        <span>{t("pages.assistant.approval.title")}</span>
        <Badge variant="secondary" className="gap-1">
          {isCommand ? <TerminalIcon className="size-3" /> : <FilePenIcon className="size-3" />}
          {isCommand
            ? t("pages.assistant.approval.command")
            : t("pages.assistant.approval.fileChange")}
        </Badge>
      </div>

      {approval.reason ? (
        <p className="mt-2 text-muted-foreground text-xs">{approval.reason}</p>
      ) : null}

      <div className="mt-2 space-y-2">
        {isCommand ? (
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

      <div className="mt-3 flex items-center gap-2">
        <Button
          size="sm"
          variant="default"
          disabled={pendingAction !== null}
          onClick={() => {
            void handle(true)
          }}
        >
          {pendingAction === "approve" ? (
            <Spinner className="size-3.5" />
          ) : (
            <CheckIcon className="size-3.5" />
          )}
          {t("pages.assistant.actions.approve")}
        </Button>
        <Button
          size="sm"
          variant="outline"
          disabled={pendingAction !== null}
          onClick={() => {
            void handle(false)
          }}
        >
          {pendingAction === "reject" ? (
            <Spinner className="size-3.5" />
          ) : (
            <XIcon className="size-3.5" />
          )}
          {t("pages.assistant.actions.reject")}
        </Button>
      </div>
    </div>
  )
}
