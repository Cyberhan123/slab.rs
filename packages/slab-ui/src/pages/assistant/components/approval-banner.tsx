"use client"

import { Badge } from "@slab/components/badge"
import { Button } from "@slab/components/button"
import { CodeBlock, CodeBlockContent } from "@slab/components/code-block"
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemFooter,
  ItemHeader,
  ItemMedia,
  ItemTitle,
} from "@slab/components/item"
import { Spinner } from "@slab/components/spinner"
import {
  Questionnaire,
  QuestionnaireChoice,
  QuestionnaireChoices,
  QuestionnaireItem,
} from "@slab/components/questionnaire"
import { useTranslation } from "@slab/i18n"
import { motion, useReducedMotion } from "motion/react"
import {
  FilePenIcon,
  ListChecksIcon,
  ShieldAlertIcon,
  TerminalIcon,
  XIcon,
} from "lucide-react"
import { useState } from "react"

import type { ApprovalScope } from "@slab/api/harness"
import type { ApprovalRequest } from "@slab/core/harness"
import { PatchDiffView } from "./patch-diff-view"
import { PlanCardBody } from "./message/message-tool-plan-part"

const changeTypeVariant: Record<string, "default" | "secondary" | "destructive"> = {
  add: "secondary",
  edit: "default",
  delete: "destructive",
}

/** Approve scopes offered as questionnaire choices, in display order. */
const APPROVE_SCOPES: ReadonlyArray<{ scope: ApprovalScope; label: string }> = [
  { scope: "run_once", label: "pages.assistant.approval.runOnce" },
  { scope: "always_in_workspace", label: "pages.assistant.approval.alwaysInWorkspace" },
  { scope: "always", label: "pages.assistant.approval.always" },
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
  const shouldReduceMotion = useReducedMotion()

  const handle = async (approved: boolean, scope: ApprovalScope, action: string) => {
    setPendingAction(action)
    try {
      await onResolve(approval.itemId, approved, scope)
    } finally {
      setPendingAction(null)
    }
  }

  // Approve scopes render as a questionnaire radio list — picking a choice
  // resolves immediately (no extra confirm step). Prefer the server-advertised
  // scopes; fall back to a single approve choice (= run-once) for older
  // servers. "deny" is the dedicated reject button instead of a scope choice.
  const approveChoices =
    approval.allowedScopes && approval.allowedScopes.length > 0
      ? APPROVE_SCOPES.filter((choice) => approval.allowedScopes!.includes(choice.scope))
      : [{ scope: "run_once" as ApprovalScope, label: "pages.assistant.actions.approve" }]

  const isCommand = approval.kind === "command"
  const isPlan = approval.kind === "plan"

  // The ai-elements Confirmation shape: an outlined Item whose header names the
  // decision, content carries the payload (plan / command / file changes), and
  // the footer holds the scope choices plus the deny action. The card springs
  // in from its anchor above the composer (skipped under reduced motion); the
  // resolve protocol and every e2e testid are unchanged.
  return (
    <motion.div
      initial={shouldReduceMotion ? false : { opacity: 0, y: 8, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ type: "spring", stiffness: 480, damping: 42, mass: 1 }}
    >
      <Item variant="outline" data-testid={isPlan ? "assistant-approval-plan" : undefined}>
        <ItemHeader>
          <ItemMedia variant="icon">
            <ShieldAlertIcon className="text-yellow-600" />
          </ItemMedia>
          <ItemContent>
            <ItemTitle className="gap-2">
              {t("pages.assistant.approval.title")}
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
            </ItemTitle>
            {approval.reason ? (
              <ItemDescription>{approval.reason}</ItemDescription>
            ) : null}
          </ItemContent>
        </ItemHeader>

        <ItemContent>
          {isPlan && approval.planSnapshot ? (
            <PlanCardBody plan={approval.planSnapshot} />
          ) : isCommand ? (
            <CodeBlock size="sm">
              <CodeBlockContent wrap>
                {`$ cd ${approval.cwd ?? "."}\n${approval.command ?? "(shell)"}`}
              </CodeBlockContent>
            </CodeBlock>
          ) : (
            <ul className="flex flex-col gap-1">
              {(approval.changes ?? []).map((change) => (
                <li key={`${change.type}:${change.path}`} className="rounded-md bg-muted/60 p-2 text-xs">
                  <div className="flex items-center gap-2">
                    <Badge variant={changeTypeVariant[change.type] ?? "secondary"}>
                      {change.type}
                    </Badge>
                    <code className="font-mono">{change.path}</code>
                  </div>
                  {change.diff ? <PatchDiffView diff={change.diff} /> : null}
                </li>
              ))}
            </ul>
          )}
        </ItemContent>

        <ItemFooter>
          {/* Actions stack VERTICALLY: one approve scope per row, deny below —
              side-by-side placement squeezed the scope labels on narrow cards. */}
          {approveChoices.length > 0 ? (
            // The questionnaire Root renders a <form>; the card lives outside
            // the composer form, and submit is swallowed (choices resolve on
            // change, so there is nothing to submit).
            <Questionnaire noValidate onSubmit={(event) => event.preventDefault()}>
              <QuestionnaireItem name="scope">
                <QuestionnaireChoices className="grid-cols-1 gap-1.5">
                  {approveChoices.map((choice) => (
                    <QuestionnaireChoice
                      key={choice.scope}
                      value={choice.scope}
                      disabled={pendingAction !== null}
                      data-testid={`assistant-approval-${choice.scope}`}
                      className="press-scale-soft gap-3 rounded-md border border-border/60 px-3 py-2 text-sm transition-colors hover:bg-accent/50 data-checked:border-primary/60 data-checked:bg-accent"
                      onChange={() => void handle(true, choice.scope, `approve:${choice.scope}`)}
                    >
                      {t(choice.label)}
                      {pendingAction === `approve:${choice.scope}` ? (
                        <Spinner className="ms-auto size-3.5" />
                      ) : null}
                    </QuestionnaireChoice>
                  ))}
                </QuestionnaireChoices>
              </QuestionnaireItem>
            </Questionnaire>
          ) : null}
          <ItemActions>
            <Button
              variant="outline"
              size="sm"
              data-testid="assistant-approval-deny"
              disabled={pendingAction !== null}
              className="press-scale"
              onClick={() => void handle(false, "deny", "deny")}
            >
              {pendingAction === "deny" ? (
                <Spinner className="size-3.5" />
              ) : (
                <XIcon className="size-3.5" />
              )}
              {t("pages.assistant.actions.reject")}
            </Button>
          </ItemActions>
        </ItemFooter>
      </Item>
    </motion.div>
  )
}
