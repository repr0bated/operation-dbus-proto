import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

/**
 * Live Log belongs on /logs (LogsPage), not Chat or Assistant.
 * Root cause of tab-vs-Logs: Chat tab mounted LiveLogPanel (EventChain gRPC);
 * LogsPage used REST api.logs.list snapshots and never subscribed.
 */

vi.mock("@/components/chat/LiveLogPanel", () => ({
  LiveLogPanel: () => <div data-testid="live-log-panel">LiveLogPanel</div>,
}));

vi.mock("@/components/chat/SystemPromptEditor", () => ({
  SystemPromptEditor: () => <div data-testid="system-prompt-editor">SystemPromptEditor</div>,
}));

vi.mock("@/components/shell/ProviderSelect", () => ({
  ProviderSelect: () => <div data-testid="provider-select" />,
}));

vi.mock("@/stores/event-store", () => ({
  useEventStore: () => ({ connected: true, logs: [], setLogs: () => undefined }),
}));

vi.mock("@/grpc/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/grpc/client")>();
  return {
    ...actual,
    chatService: {
      send: () => ({
        stream: new ReadableStream({ start(c) { c.close(); } }),
        abort: () => undefined,
      }),
    },
  };
});

vi.mock("@/grpc/chatbot-cognitive", () => ({
  chatbotCognitive: {
    listSouls: async () => [],
    listNamespaces: async () => [],
    updateSoul: async () => undefined,
    deleteSoul: async () => undefined,
  },
}));

vi.mock("@/components/ui-model/PluginSchemaPanel", () => ({
  PluginSchemaPanel: () => <div data-testid="schema-panel" />,
}));

vi.mock("@/lib/plugin-ui-surfaces", () => ({
  resolveZeroclawChatSelection: async () => ({ provider: "opencode", model: "test" }),
}));

import ChatPage from "@/pages/ChatPage";
import AssistantPage from "@/pages/AssistantPage";
import LogsPage from "@/pages/LogsPage";

describe("Live Log placement on Logs page", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    Element.prototype.scrollTo = vi.fn() as unknown as typeof Element.prototype.scrollTo;
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("keeps Main Chat chat-only (no Live Log)", () => {
    render(
      <MemoryRouter>
        <ChatPage />
      </MemoryRouter>,
    );

    expect(screen.getByRole("heading", { name: "Chat" })).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/Message/)).toBeInTheDocument();
    expect(screen.queryByTestId("live-log-panel")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Live Log/i })).not.toBeInTheDocument();
  });

  it("keeps Assistant config-only (system prompt, no Live Log)", async () => {
    render(
      <MemoryRouter>
        <AssistantPage />
      </MemoryRouter>,
    );

    expect(await screen.findByTestId("system-prompt-editor")).toBeInTheDocument();
    expect(screen.queryByTestId("live-log-panel")).not.toBeInTheDocument();
  });

  it("hosts LiveLogPanel on LogsPage", () => {
    render(
      <MemoryRouter>
        <LogsPage />
      </MemoryRouter>,
    );

    expect(screen.getByRole("heading", { name: "Logs" })).toBeInTheDocument();
    expect(screen.getByTestId("live-log-panel")).toBeInTheDocument();
  });
});
