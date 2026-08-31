import { useState, useRef, useMemo } from "react";
import { JSONUIProvider, Renderer, type SetState, type Spec } from "@json-render/react";
import { useAntigravityChat, type ChatMode, type StreamChatMessage } from "@/stream/use-antigravity-chat";
import { uiStore } from "@/store/ui-store";
import { registry, handlers as catalogHandlers } from "@/catalog/registry";
import { parseContent } from "@/utils/parse-content";

interface ChatMessage extends StreamChatMessage {
  spec?: Spec | null;
  text?: string;
}

interface ChatbotLayoutProps {
  streamId?: string;
  mode?: ChatMode;
  systemPrompt?: string;
}

export function ChatbotLayout({
  streamId = "stream_antigravity_01",
  mode = "live",
  systemPrompt,
}: ChatbotLayoutProps) {
  const { messages, input, handleInputChange, handleSubmit, sending } = useAntigravityChat(
    streamId,
    mode,
    systemPrompt,
  );

  // Track the currently active spec from the latest message
  const [currentSpec, setCurrentSpec] = useState<Spec | null>(null);

  // Parse specs from messages as they arrive
  const enrichedMessages = useMemo(
    () =>
      messages.map((msg) => {
        const { spec, text } = parseContent(msg.content);
        return { ...msg, spec, text };
      }),
    [messages],
  );

  // Update currentSpec from the latest assistant message that has a spec
  useMemo(() => {
    const latestWithSpec = [...enrichedMessages].reverse().find((msg) => msg.role === "assistant" && msg.spec);
    if (latestWithSpec?.spec) {
      setCurrentSpec(latestWithSpec.spec);
    }
  }, [enrichedMessages]);

  const snapshotRef = useRef(uiStore.getSnapshot);
  snapshotRef.current = uiStore.getSnapshot;

  const setState = useRef<SetState>((updater) => {
    const next = updater(snapshotRef.current());
    const updates: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(next)) {
      updates[`/${key}`] = value;
    }
    uiStore.update(updates);
  });

  const handlers = useMemo(
    () => ({
      ...catalogHandlers(() => setState.current, () => snapshotRef.current()),
      // Add custom handler for sending messages from UI interactions
      sendChatMessage: async (params: Record<string, unknown>) => {
        const message = params?.message;
        if (typeof message === "string" && message.trim()) {
          // This would be called by UI elements that need to send chat messages
          // For now, just log it - the user would need to type in the input
          console.log("[UI interaction] Would send message:", message);
        }
      },
    }),
    [],
  );

  return (
    <JSONUIProvider registry={registry} store={uiStore} handlers={handlers}>
      <div className="flex h-screen w-screen gap-0 bg-neutral-950">
        {/* Main content area */}
        <div className="flex-1 flex flex-col overflow-hidden bg-neutral-950">
          {currentSpec ? (
            <Renderer spec={currentSpec} registry={registry} />
          ) : (
            <div className="flex items-center justify-center h-full text-neutral-400">
              <div className="text-center">
                <p className="text-lg mb-2">Waiting for UI from chatbot…</p>
                <p className="text-sm text-neutral-500">Ask the chatbot to draw the interface</p>
              </div>
            </div>
          )}
        </div>

        {/* Fixed chat panel */}
        <div className="w-80 flex flex-col bg-neutral-900 border-l border-neutral-800">
          {/* Chat messages */}
          <div className="flex-1 overflow-y-auto p-3 space-y-3 min-h-0">
            {enrichedMessages.length === 0 ? (
              <div className="text-sm text-neutral-500 text-center py-8">
                <p>Start a conversation</p>
              </div>
            ) : (
              enrichedMessages.map((msg) => (
                <div
                  key={msg.id}
                  className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}
                >
                  <div
                    className={`max-w-[85%] rounded-lg px-3 py-2 text-sm ${
                      msg.role === "user"
                        ? "bg-sky-700 text-white"
                        : "bg-neutral-800 text-neutral-100"
                    }`}
                  >
                    {/* Show text content */}
                    {msg.text && (
                      <div className="whitespace-pre-wrap break-words">{msg.text}</div>
                    )}
                    {/* Show indicator if spec was extracted */}
                    {msg.spec && (
                      <div className="text-xs text-neutral-400 mt-1 italic">
                        [UI spec rendered]
                      </div>
                    )}
                    {!msg.text && !msg.spec && (
                      <div className="text-neutral-500 italic">
                        {msg.role === "user" ? "[message]" : "[response]"}
                      </div>
                    )}
                  </div>
                </div>
              ))
            )}
          </div>

          {/* Chat input */}
          <div className="border-t border-neutral-800 p-3 shrink-0">
            <form onSubmit={handleSubmit} className="flex gap-2">
              <input
                value={input}
                onChange={handleInputChange}
                disabled={sending}
                placeholder="Ask chatbot…"
                className="flex-1 bg-neutral-800 border border-neutral-700 text-white rounded px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-sky-500 placeholder-neutral-500"
              />
              <button
                type="submit"
                disabled={sending || !input.trim()}
                className="bg-sky-700 hover:bg-sky-600 px-3 py-2 text-sm text-white rounded disabled:opacity-50 disabled:cursor-not-allowed shrink-0"
              >
                {sending ? "…" : "Send"}
              </button>
            </form>
          </div>
        </div>
      </div>
    </JSONUIProvider>
  );
}
