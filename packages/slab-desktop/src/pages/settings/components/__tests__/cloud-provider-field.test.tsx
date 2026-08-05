import { beforeEach, describe, expect, it, vi } from "vitest";
import { render } from "vitest-browser-react";

import { CloudProviderField } from "../cloud-provider-field";

vi.mock("@slab/i18n", () => ({
  useTranslation: vi.fn<
    () => { t: (key: string, options?: { count?: number }) => string }
  >(() => ({
    t: (key, options) => (options?.count !== undefined ? `${key}:${options.count}` : key),
  })),
}));

const OPENAI_ENTRY = {
  id: "openai-main",
  family: "openai",
  display_name: "OpenAI",
  api_base: "https://api.openai.com/v1",
  auth: { api_key: "sk-test", api_key_env: null },
};

const ANTHROPIC_ENTRY = {
  id: "anthropic-main",
  family: "anthropic",
  display_name: "Anthropic",
  api_base: "https://api.anthropic.com/v1",
  auth: { api_key: null, api_key_env: "ANTHROPIC_API_KEY" },
};

beforeEach(() => {
  vi.clearAllMocks();
});

describe("CloudProviderField", () => {
  it("renders the empty state when no providers are configured", async () => {
    const screen = await render(<CloudProviderField value={[]} onChange={vi.fn()} />);

    await expect.element(screen.getByText("pages.settings.providerRegistry.empty")).toBeInTheDocument();
    await expect.element(screen.getByText("pages.settings.providerRegistry.addProvider")).toBeInTheDocument();
  });

  it("lists configured providers with their display name and api base", async () => {
    const screen = await render(
      <CloudProviderField value={[OPENAI_ENTRY, ANTHROPIC_ENTRY]} onChange={vi.fn()} />,
    );

    // Display name appears in both the title and the family badge.
    expect(screen.getByText("OpenAI").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Anthropic").length).toBeGreaterThanOrEqual(1);
    await expect.element(screen.getByText("https://api.openai.com/v1")).toBeInTheDocument();
    await expect.element(screen.getByText("https://api.anthropic.com/v1")).toBeInTheDocument();
    // configured count uses the plural form (count !== 1)
    await expect.element(
      screen.getByText("pages.settings.providerRegistry.configuredProviders:2"),
    ).toBeInTheDocument();
  });

  it("deletes a provider and emits the remaining registry via onChange", async () => {
    const onChange = vi.fn();
    const screen = await render(
      <CloudProviderField value={[OPENAI_ENTRY, ANTHROPIC_ENTRY]} onChange={onChange} />,
    );

    // First remove button belongs to OpenAI (rendered first).
    const removeButtons = screen.getByLabelText("Remove provider").all();
    await removeButtons[0]!.click();

    expect(onChange).toHaveBeenCalledTimes(1);
    const emitted = onChange.mock.calls[0]![0] as Array<{ id: string; family: string }>;
    expect(emitted).toHaveLength(1);
    expect(emitted[0]!.id).toBe("anthropic-main");
    expect(emitted[0]!.family).toBe("anthropic");
  });

  it("preserves the auth shape (api_key / api_key_env) when emitting entries", async () => {
    const onChange = vi.fn();
    const screen = await render(<CloudProviderField value={[ANTHROPIC_ENTRY]} onChange={onChange} />);

    const removeButtons = screen.getByLabelText("Remove provider").all();
    await removeButtons[0]!.click();

    expect(onChange).toHaveBeenCalledTimes(1);
    // Removing the last entry emits an empty array (activation will clean up its models).
    expect(onChange.mock.calls[0]![0]).toEqual([]);
  });

  it("renders a custom provider without crashing when family is unknown", async () => {
    const customEntry = { ...OPENAI_ENTRY, family: "openai_compatible", display_name: "My Local" };
    const screen = await render(<CloudProviderField value={[customEntry]} onChange={vi.fn()} />);

    await expect.element(screen.getByText("My Local")).toBeInTheDocument();
  });
});
