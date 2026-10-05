import { userEvent } from "vitest/browser"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"
import type { ReactNode } from "react"

import { type QuestionnaireAnswerPayload, QuestionnaireCard } from "../questionnaire-card"
import type { QuestionnaireRequest } from "@slab/core/harness"

vi.mock("@slab/i18n", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

vi.mock("@slab/components/button", () => ({
  // Forward remaining props (data-testid, className, …) so testid queries work.
  Button: ({
    children,
    onClick,
    disabled,
    ...props
  }: {
    children: ReactNode
    onClick?: () => void
    disabled?: boolean
  }) => (
    <button type="button" onClick={onClick} disabled={disabled} {...props}>
      {children}
    </button>
  ),
  buttonVariants: () => "",
}))

vi.mock("@slab/components/spinner", () => ({
  Spinner: (props: { className?: string }) => <span data-testid="spinner" {...props} />,
}))

function request(overrides: Partial<QuestionnaireRequest> = {}): QuestionnaireRequest {
  return {
    itemId: "call-q1",
    threadId: "hthread-1",
    question: "Which database engine?",
    choices: [
      { label: "SQLite", value: "sqlite" },
      { label: "Postgres", value: "postgres" },
    ],
    allowMultiple: false,
    allowCustomInput: false,
    required: true,
    status: "pending",
    ...overrides,
  }
}

describe("QuestionnaireCard", () => {
  it("renders the question and choices", async () => {
    const screen = await render(<QuestionnaireCard request={request()} onResolve={vi.fn<(itemId: string, answers: QuestionnaireAnswerPayload) => void>()} />)
    await expect.element(screen.getByText("Which database engine?")).toBeInTheDocument()
    await expect
      .element(screen.getByRole("radio", { name: "SQLite" }))
      .toBeInTheDocument()
    await expect.element(screen.getByTestId("assistant-questionnaire-submit")).toBeInTheDocument()
  })

  it("submits a single selection on Submit", async () => {
    const onResolve = vi.fn<(itemId: string, answers: QuestionnaireAnswerPayload) => void>()
    const screen = await render(<QuestionnaireCard request={request()} onResolve={onResolve} />)

    await screen.getByRole("radio", { name: "Postgres" }).click()
    await screen.getByTestId("assistant-questionnaire-submit").click()

    expect(onResolve).toHaveBeenCalledWith("call-q1", { selected: ["postgres"], custom: null })
  })

  it("supports multiple selection when allowMultiple", async () => {
    const onResolve = vi.fn<(itemId: string, answers: QuestionnaireAnswerPayload) => void>()
    const screen = await render(
      <QuestionnaireCard
        request={request({ allowMultiple: true })}
        onResolve={onResolve}
      />,
    )

    // Checkboxes, not radios.
    await expect.element(screen.getByRole("checkbox", { name: "SQLite" })).toBeInTheDocument()
    await screen.getByRole("checkbox", { name: "SQLite" }).click()
    await screen.getByRole("checkbox", { name: "Postgres" }).click()
    await screen.getByTestId("assistant-questionnaire-submit").click()

    expect(onResolve).toHaveBeenCalledWith("call-q1", {
      selected: ["sqlite", "postgres"],
      custom: null,
    })
  })

  it("submits the custom answer text when offered", async () => {
    const onResolve = vi.fn<(itemId: string, answers: QuestionnaireAnswerPayload) => void>()
    const screen = await render(
      <QuestionnaireCard
        request={request({ allowCustomInput: true })}
        onResolve={onResolve}
      />,
    )

    // The custom input is the only textbox in the card (radios/checkboxes
    // don't take the textbox role); its placeholder comes from the i18n mock.
    await userEvent.type(screen.getByRole("textbox"), "MySQL")
    await screen.getByTestId("assistant-questionnaire-submit").click()

    expect(onResolve).toHaveBeenCalledWith("call-q1", { selected: [], custom: "MySQL" })
  })

  it("blocks empty submits when required and shows the error", async () => {
    const onResolve = vi.fn<(itemId: string, answers: QuestionnaireAnswerPayload) => void>()
    const screen = await render(<QuestionnaireCard request={request()} onResolve={onResolve} />)

    await screen.getByTestId("assistant-questionnaire-submit").click()

    expect(onResolve).not.toHaveBeenCalled()
    await expect
      .element(screen.getByText("pages.assistant.questionnaire.required"))
      .toBeInTheDocument()
  })

  it("renders a Skip button and resolves skipped when the question is optional", async () => {
    const onResolve = vi.fn<(itemId: string, answers: QuestionnaireAnswerPayload) => void>()
    const screen = await render(
      <QuestionnaireCard request={request({ required: false })} onResolve={onResolve} />,
    )

    await screen.getByTestId("assistant-questionnaire-skip").click()

    expect(onResolve).toHaveBeenCalledWith("call-q1", {
      selected: [],
      custom: null,
      skipped: true,
    })
  })

  it("disables everything (with a spinner) while a resolution is pending", async () => {
    let resolvePromise: (() => void) | undefined
    const onResolve = vi.fn<(itemId: string, answers: QuestionnaireAnswerPayload) => void>(() =>
        new Promise<void>((r) => {
          resolvePromise = () => r()
        }),
    )
    const screen = await render(<QuestionnaireCard request={request()} onResolve={onResolve} />)

    await screen.getByRole("radio", { name: "SQLite" }).click()
    await screen.getByTestId("assistant-questionnaire-submit").click()

    await expect.element(screen.getByTestId("spinner")).toBeInTheDocument()
    await expect.element(screen.getByRole("radio", { name: "SQLite" })).toBeDisabled()
    await expect.element(screen.getByTestId("assistant-questionnaire-submit")).toBeDisabled()

    resolvePromise?.()
    await expect.element(screen.getByRole("radio", { name: "SQLite" })).toBeEnabled()
  })
})
