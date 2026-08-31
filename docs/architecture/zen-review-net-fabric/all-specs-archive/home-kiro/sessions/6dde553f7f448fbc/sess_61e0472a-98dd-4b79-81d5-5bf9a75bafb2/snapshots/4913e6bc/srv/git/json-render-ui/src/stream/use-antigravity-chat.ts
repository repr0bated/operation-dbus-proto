import { useCallback, useEffect, useRef, useState, type ChangeEvent, type FormEvent } from "react";
import { grpcServerStream, grpcUnary, resolveIdentity } from "./grpc-transport";
import { decodeChatFrame, decodeRepeatedStrings, encodeSendRequest } from "./chat-codec";
import { uiStore } from "@/store/ui-store";

export type ChatRole = "user" | "agent" | "system";
export type ChatMode = "live" | "accountability";

export interface StreamChatMessage {
  id: string;
  role: ChatRole;
  content: string;
}

function messagesPath(streamId: string): string {
  return `/streams/${streamId}/messages`;
}

function makeId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
}

function readMessages(streamId: string): StreamChatMessage[] {
  return (uiStore.get(messagesPath(streamId)) as StreamChatMessage[] | undefined) ?? [];
}

function writeMessages(streamId: string, messages: StreamChatMessage[]): void {
  uiStore.set(messagesPath(streamId), messages);
}

function mapRole(role: string): ChatRole {
  if (role === "user") return "user";
  if (role === "system") return "system";
  return "agent";
}

async function resolveRoute(): Promise<{ provider: string; model: string }> {
  const router = (uiStore.get("/plugins/tched_router") as Record<string, unknown> | undefined) ?? {};
  let provider = typeof router.selected_provider === "string" ? router.selected_provider : "";
  let model = typeof router.selected_model === "string" ? router.selected_model : "";
  if (provider && model) return { provider, model };
  try {
    const res = await fetch("/api/llm/status");
    if (res.ok) {
      const status = (await res.json()) as { provider?: string; model?: string };
      if (typeof status.provider === "string") provider = provider || status.provider;
      if (typeof status.model === "string") model = model || status.model;
    }
  } catch {
    /* empty provider/model → bridge uses live selected_* */
  }
  return { provider, model };
}

async function loadEpisodeContext(): Promise<string[]> {
  const empty = new Uint8Array(0);
  const cap = { capability: "chatbot.read" };
  const [episodesBuf, contextBuf] = await Promise.all([
    grpcUnary(
      "operation.method.ctl_plane_chatbot.list_episodes.ListEpisodesService",
      "ListEpisodes",
      empty,
      cap,
    ),
    grpcUnary(
      "operation.method.ctl_plane_chatbot.query_context.QueryContextService",
      "QueryContext",
      empty,
      cap,
    ),
  ]);
  return [
    ...decodeRepeatedStrings(episodesBuf, 1),
    ...decodeRepeatedStrings(contextBuf, 1),
  ].filter((line) => line.length > 0);
}

/**
 * Same shape as the pasted Vercel useChat wiring: messages + input + submit,
 * with every delta written to /streams/<streamId>/messages.
 * Transport is op_chat.chat.ChatService on :8090 (proxied via :8080), not OpenAI Edge.
 * Accountability mode prepends ctl_plane_chatbot ListEpisodes / QueryContext.
 */
export function useAntigravityChat(streamId: string, mode: ChatMode = "live", systemPrompt?: string) {
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [messages, setMessages] = useState<StreamChatMessage[]>(() => readMessages(streamId));
  const abortRef = useRef<(() => void) | null>(null);
  const conversationId = useRef(makeId(streamId));
  const seededRef = useRef(false);

  useEffect(() => {
    writeMessages(streamId, messages);
  }, [streamId, messages]);

  useEffect(() => {
    if (mode !== "accountability" || seededRef.current) return;
    if (readMessages(streamId).length > 0) {
      seededRef.current = true;
      return;
    }
    let cancelled = false;
    (async () => {
      await resolveIdentity();
      let content =
        "Accountability Assistant. Ask about chatbot decisions; answers use tched_router plus ctl_plane_chatbot episodes.";
      try {
        const lines = await loadEpisodeContext();
        content = lines.length
          ? `Accountability Assistant. Loaded ${lines.length} episode/context line(s) from ctl_plane_chatbot.`
          : "Accountability Assistant. ctl_plane_chatbot returned no episodes yet.";
      } catch (err) {
        content = `Accountability Assistant. Episode load failed: ${(err as Error).message}`;
      }
      if (cancelled) return;
      seededRef.current = true;
      setMessages((prev) =>
        prev.length > 0 ? prev : [{ id: "sys-1", role: "system", content }],
      );
    })();
    return () => {
      cancelled = true;
    };
  }, [mode, streamId]);

  const handleInputChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setInput(event.target.value);
  }, []);

  const handleSubmit = useCallback(
    async (event: FormEvent) => {
      event.preventDefault();
      const text = input.trim();
      if (!text || sending) return;

      const userMsg: StreamChatMessage = { id: makeId("user"), role: "user", content: text };
      const agentId = makeId("agent");
      const agentMsg: StreamChatMessage = { id: agentId, role: "agent", content: "" };

      setInput("");
      setSending(true);
      setMessages((prev) => [...prev, userMsg, agentMsg]);
      abortRef.current?.();
      await resolveIdentity();

      const route = await resolveRoute();
      const payloadMessages: Array<{ role: string; content: string }> = [];
      if (mode === "accountability") {
        let episodeBlock = "";
        try {
          const lines = await loadEpisodeContext();
          if (lines.length) episodeBlock = `\n\nEpisodes:\n${lines.join("\n")}`;
        } catch {
          /* send without episode block */
        }
        payloadMessages.push({
          role: "system",
          content:
            "You are the Accountability Assistant. Answer from reasoning episodes and tched_router decisions. Be concise." +
            episodeBlock,
        });
      } else if (systemPrompt) {
        payloadMessages.push({
          role: "system",
          content: systemPrompt,
        });
      }
      payloadMessages.push({ role: "user", content: text });

      const uiMessages = new TextEncoder().encode(JSON.stringify(payloadMessages));
      const payload = encodeSendRequest({
        conversationId: conversationId.current,
        uiMessages,
        provider: route.provider,
        model: route.model,
      });

      const { stream, abort } = grpcServerStream(
        "op_chat.chat.ChatService",
        "Send",
        payload,
      );
      console.log(`[antigravity-send] streamId=${streamId} mode=${mode} conversationId=${conversationId.current}`);
      abortRef.current = abort;

      try {
        const reader = stream.getReader();
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          const frame = decodeChatFrame(value);
          if (frame.kind === "part" && frame.text) {
            setMessages((prev) =>
              prev.map((msg) =>
                msg.id === agentId
                  ? { ...msg, role: mapRole(frame.role), content: `${msg.content}${frame.text}` }
                  : msg,
              ),
            );
          } else if (frame.kind === "error") {
            setMessages((prev) =>
              prev.map((msg) =>
                msg.id === agentId ? { ...msg, role: "system", content: frame.message } : msg,
              ),
            );
          }
        }
      } catch (err) {
        const message = (err as Error).message;
        if (message !== "AbortError") {
          setMessages((prev) =>
            prev.map((msg) =>
              msg.id === agentId ? { ...msg, role: "system", content: message } : msg,
            ),
          );
        }
      } finally {
        setSending(false);
        abortRef.current = null;
      }
    },
    [input, sending, mode, streamId],
  );

  useEffect(() => () => abortRef.current?.(), []);

  return { messages, input, handleInputChange, handleSubmit, sending };
}
