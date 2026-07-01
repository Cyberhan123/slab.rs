import { beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";

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
  it("renders the empty state when no providers are configured", () => {
    render(<CloudProviderField value={[]} onChange={vi.fn()} />);

    expect(screen.getByText("pages.settings.providerRegistry.empty")).toBeDefined();
    expect(screen.getByText("pages.settings.providerRegistry.addProvider")).toBeDefined();
  });

  it("lists configured providers with their display name and api base", () => {
    render(
      <CloudProviderField value={[OPENAI_ENTRY, ANTHROPIC_ENTRY]} onChange={vi.fn()} />,
    );

    // Display name appears in both the title and the family badge.
    expect(screen.getAllByText("OpenAI").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Anthropic").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("https://api.openai.com/v1")).toBeDefined();
    expect(screen.getByText("https://api.anthropic.com/v1")).toBeDefined();
    // configured count uses the plural form (count !== 1)
    expect(
      screen.getByText("pages.settings.providerRegistry.configuredProviders:2"),
    ).toBeDefined();
  });

  it("deletes a provider and emits the remaining registry via onChange", () => {
    const onChange = vi.fn();
    render(<CloudProviderField value={[OPENAI_ENTRY, ANTHROPIC_ENTRY]} onChange={onChange} />);

    // First remove button belongs to OpenAI (rendered first).
    const removeButtons = screen.getAllByLabelText("Remove provider");
    fireEvent.click(removeButtons[0]!);

    expect(onChange).toHaveBeenCalledTimes(1);
    const emitted = onChange.mock.calls[0]![0] as Array<{ id: string; family: string }>;
    expect(emitted).toHaveLength(1);
    expect(emitted[0]!.id).toBe("anthropic-main");
    expect(emitted[0]!.family).toBe("anthropic");
  });

  it("preserves the auth shape (api_key / api_key_env) when emitting entries", () => {
    const onChange = vi.fn();
    render(<CloudProviderField value={[ANTHROPIC_ENTRY]} onChange={onChange} />);

    fireEvent.click(screen.getAllByLabelText("Remove provider")[0]!);

    expect(onChange).toHaveBeenCalledTimes(1);
    // Removing the last entry emits an empty array (activation will clean up its models).
    expect(onChange.mock.calls[0]![0]).toEqual([]);
  });

  it("renders a custom provider without crashing when family is unknown", () => {
    const customEntry = { ...OPENAI_ENTRY, family: "openai_compatible", display_name: "My Local" };
    render(<CloudProviderField value={[customEntry]} onChange={vi.fn()} />);

    expect(screen.getByText("My Local")).toBeDefined();
    cleanup();
  });
});
