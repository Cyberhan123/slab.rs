import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { usePluginAuthorizationStore } from "@/store/usePluginAuthorizationStore";
import { usePluginAuthorization } from "../use-plugin-authorization";

vi.mock("@/store/ui-state-storage", () => ({
  createUiStateStorage: () => ({
    getItem: vi.fn<() => Promise<null>>(async () => null),
    removeItem: vi.fn<() => Promise<void>>(async () => {}),
    setItem: vi.fn<() => Promise<void>>(async () => {}),
  }),
}));

vi.mock("@slab/i18n", () => ({
  useTranslation: () => ({
    // Append the params as JSON so tests can assert which request is shown without
    // loading locale data.
    t: (key: string, params?: Record<string, unknown>) =>
      params ? `${key}:${JSON.stringify(params)}` : key,
    i18n: { resolvedLanguage: "en", language: "en" },
  }),
}));

type Authorize = ReturnType<typeof usePluginAuthorization>["authorize"];

function setup() {
  let authorizeRef: Authorize | null = null;
  function Harness() {
    const { authorize, prompt } = usePluginAuthorization("plugin-a", "Plugin A");
    authorizeRef = authorize;
    return <>{prompt}</>;
  }
  render(<Harness />);
  return {
    authorize: (...args: Parameters<Authorize>) => {
      if (!authorizeRef) throw new Error("harness not mounted");
      return authorizeRef(...args);
    },
  };
}

describe("usePluginAuthorization", () => {
  it("serializes concurrent prompts so each request resolves instead of orphaning", async () => {
    usePluginAuthorizationStore.setState({ grants: {} });
    const { authorize } = setup();

    // Two distinct unauthorized permissions requested back-to-back (a plugin
    // commonly fires both on load).
    const first = authorize("chat:complete", {
      method: "POST",
      path: "/v1/chat/completions",
    });
    const second = authorize("models:read", { method: "GET", path: "/v1/models" });

    // The first prompt is shown; the second is queued behind it.
    await screen.findByTestId("plugin-authorization-dialog");
    expect(screen.getByText(/chat\/completions/)).toBeInTheDocument();

    await userEvent.click(screen.getByTestId("plugin-authorization-allow"));
    expect(await first).toBe(true);

    // The queue advances and the second request now has its own prompt.
    expect(screen.getByText(/\/v1\/models/)).toBeInTheDocument();
    await userEvent.click(screen.getByTestId("plugin-authorization-allow"));
    expect(await second).toBe(true);

    const store = usePluginAuthorizationStore.getState();
    expect(store.isAuthorized("plugin-a", "chat:complete")).toBe(true);
    expect(store.isAuthorized("plugin-a", "models:read")).toBe(true);
  });

  it("deny short-circuits without granting and closes the prompt", async () => {
    usePluginAuthorizationStore.setState({ grants: {} });
    const { authorize } = setup();

    const pending = authorize("chat:complete", {
      method: "POST",
      path: "/v1/chat/completions",
    });

    await screen.findByTestId("plugin-authorization-dialog");
    await userEvent.click(screen.getByTestId("plugin-authorization-deny"));

    expect(await pending).toBe(false);
    expect(usePluginAuthorizationStore.getState().isAuthorized("plugin-a", "chat:complete")).toBe(
      false,
    );
    expect(screen.queryByTestId("plugin-authorization-dialog")).not.toBeInTheDocument();
  });
});
