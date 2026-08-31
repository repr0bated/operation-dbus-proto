/**
 * Antigravity Group Chat - 2-person chat with separate identities
 * Each person has their own model selector and identity for the tunnel connection.
 */
import { useCallback, useEffect, useRef, useState, type ChangeEvent, type FormEvent } from "react";
import { grpcServerStream, resolveIdentity } from "@/stream/grpc-transport";
import { decodeChatFrame, encodeSendRequest } from "@/stream/chat-codec";
import { uiStore } from "@/store/ui-store";
import type { El } from "./types";

type ChatRole = "user" | "agent" | "system";

interface GroupMessage {
  id: string;
  personId: "person1" | "person2";
  role: ChatRole;
  content: string;
  model?: string;
}

interface ModelRoute {
  provider: string;
  model: string;
  display_name?: string;
}

function makeId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
}

function mapRole(role: string): ChatRole {
  if (role === "user") return "user";
  if (role === "system") return "system";
  return "agent";
}

/** Hook for a single person's chat identity */
function usePersonChat(
  personId: "person1" | "person2",
  streamId: string,
  selectedModel: string,
  selectedProvider: string,
  onMessage: (msg: GroupMessage) => void,
  onUpdate: (id: string, content: string, role: ChatRole) => void,
) {
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const abortRef = useRef<(() => void) | null>(null);
  const conversationId = useRef(makeId(`${streamId}-${personId}`));

  const handleInputChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setInput(e.target.value);
  }, []);

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      const text = input.trim();
      if (!text || sending) return;

      const userMsg: GroupMessage = {
        id: makeId("user"),
        personId,
        role: "user",
        content: text,
        model: selectedModel,
      };
      const agentId = makeId("agent");
      const agentMsg: GroupMessage = {
        id: agentId,
        personId,
        role: "agent",
        content: "",
        model: selectedModel,
      };

      setInput("");
      setSending(true);
      onMessage(userMsg);
      onMessage(agentMsg);
      abortRef.current?.();

      await resolveIdentity();

      const payloadMessages = [{ role: "user", content: text }];
      const uiMessages = new TextEncoder().encode(JSON.stringify(payloadMessages));
      const payload = encodeSendRequest({
        conversationId: conversationId.current,
        uiMessages,
        provider: selectedProvider,
        model: selectedModel,
      });

      const { stream, abort } = grpcServerStream(
        "op_chat.chat.ChatService",
        "Send",
        payload,
      );
      console.log(`[group-chat] ${personId} send: model=${selectedModel} provider=${selectedProvider}`);
      abortRef.current = abort;

      try {
        const reader = stream.getReader();
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          const frame = decodeChatFrame(value);
          if (frame.kind === "part" && frame.text) {
            onUpdate(agentId, frame.text, mapRole(frame.role));
          } else if (frame.kind === "error") {
            onUpdate(agentId, frame.message, "system");
          }
        }
      } catch (err) {
        const message = (err as Error).message;
        if (message !== "AbortError") {
          onUpdate(agentId, message, "system");
        }
      } finally {
        setSending(false);
        abortRef.current = null;
      }
    },
    [input, sending, personId, selectedModel, selectedProvider, onMessage, onUpdate],
  );

  useEffect(() => () => abortRef.current?.(), []);

  return { input, handleInputChange, handleSubmit, sending };
}

/** Model selector dropdown */
function ModelSelector({
  value,
  onChange,
  models,
  label,
}: {
  value: string;
  onChange: (model: string, provider: string) => void;
  models: ModelRoute[];
  label: string;
}) {
  return (
    <div className="flex items-center gap-2">
      <span className="text-xs text-neutral-400">{label}:</span>
      <select
        value={value}
        onChange={(e) => {
          const selected = models.find((m) => m.model === e.target.value);
          if (selected) onChange(selected.model, selected.provider);
        }}
        className="bg-zinc-900 border border-zinc-700 text-white text-xs rounded px-2 py-1 focus:outline-none focus:ring-1 focus:ring-sky-500"
      >
        {models.map((m) => (
          <option key={`${m.provider}/${m.model}`} value={m.model}>
            {m.display_name || m.model}
          </option>
        ))}
      </select>
    </div>
  );
}

/** Person's chat panel */
function PersonPanel({
  personId,
  label,
  color,
  messages,
  input,
  sending,
  onInputChange,
  onSubmit,
  selectedModel,
  selectedProvider,
  onModelChange,
  models,
}: {
  personId: "person1" | "person2";
  label: string;
  color: string;
  messages: GroupMessage[];
  input: string;
  sending: boolean;
  onInputChange: (e: ChangeEvent<HTMLInputElement>) => void;
  onSubmit: (e: FormEvent) => void;
  selectedModel: string;
  selectedProvider: string;
  onModelChange: (model: string, provider: string) => void;
  models: ModelRoute[];
}) {
  const personMessages = messages.filter((m) => m.personId === personId);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [personMessages]);

  return (
    <div className={`flex flex-col flex-1 min-w-0 rounded-lg border ${color} bg-neutral-950/30 p-3`}>
      <div className="flex items-center justify-between mb-2 shrink-0">
        <h3 className="text-sm font-medium text-neutral-200">{label}</h3>
        <ModelSelector
          value={selectedModel}
          onChange={onModelChange}
          models={models}
          label="Model"
        />
      </div>
      
      <div
        ref={scrollRef}
        className="flex-1 flex flex-col gap-2 overflow-y-auto min-h-0 rounded-md border border-neutral-800 bg-neutral-950/40 p-3"
      >
        {personMessages.map((msg) => {
          const align =
            msg.role === "user"
              ? "self-end bg-sky-950/50 border-sky-800"
              : "self-start bg-neutral-900 border-neutral-800";
          const roleLabel = msg.role === "agent" ? "assistant" : msg.role;
          return (
            <div key={msg.id} className={`max-w-[85%] rounded-lg border px-3 py-2 ${align}`}>
              <div className="flex items-center gap-2 mb-1">
                <span className="text-[10px] uppercase tracking-wide text-neutral-500">{roleLabel}</span>
                {msg.model && (
                  <span className="text-[9px] text-neutral-600">{msg.model}</span>
                )}
              </div>
              <div className="text-sm text-neutral-100 whitespace-pre-wrap break-words">
                {msg.content || (msg.role === "agent" ? "…" : "")}
              </div>
            </div>
          );
        })}
      </div>
      
      <form onSubmit={onSubmit} className="flex gap-2 mt-2">
        <input
          value={input}
          onChange={onInputChange}
          disabled={sending}
          placeholder={`${label} message…`}
          className="flex-1 bg-zinc-900 border border-zinc-800 text-white rounded p-2 text-sm focus:outline-none focus:ring-1 focus:ring-sky-500"
        />
        <button
          type="submit"
          disabled={sending || !input.trim()}
          className="bg-sky-700 px-4 py-2 text-sm text-white rounded disabled:opacity-50"
        >
          {sending ? "…" : "Send"}
        </button>
      </form>
    </div>
  );
}

/** Main Group Chat Container */
export const AntigravityGroupChatEl: El<"antigravityGroupChat"> = ({ props }) => {
  const streamId = props.streamId || "group_chat_01";
  const [messages, setMessages] = useState<GroupMessage[]>([]);
  const [models, setModels] = useState<ModelRoute[]>([]);
  
  // Person 1 state
  const [person1Model, setPerson1Model] = useState("");
  const [person1Provider, setPerson1Provider] = useState("");
  
  // Person 2 state
  const [person2Model, setPerson2Model] = useState("");
  const [person2Provider, setPerson2Provider] = useState("");

  // Load models from tched_router
  useEffect(() => {
    const loadModels = async () => {
      try {
        const res = await fetch("/api/llm/models");
        if (res.ok) {
          const data = (await res.json()) as { models: ModelRoute[] };
          if (data.models?.length) {
            setModels(data.models);
            // Default to first two different models if available
            if (!person1Model && data.models[0]) {
              setPerson1Model(data.models[0].model);
              setPerson1Provider(data.models[0].provider);
            }
            if (!person2Model && data.models[1]) {
              setPerson2Model(data.models[1].model);
              setPerson2Provider(data.models[1].provider);
            } else if (!person2Model && data.models[0]) {
              setPerson2Model(data.models[0].model);
              setPerson2Provider(data.models[0].provider);
            }
          }
        }
      } catch (err) {
        console.error("[group-chat] Failed to load models:", err);
      }
    };
    loadModels();
  }, []);

  // Also try tched_router plugin state
  useEffect(() => {
    const router = uiStore.get("/plugins/tched_router") as Record<string, unknown> | undefined;
    if (router?.model_routes && Array.isArray(router.model_routes) && models.length === 0) {
      const routes = router.model_routes as ModelRoute[];
      setModels(routes);
      if (routes[0] && !person1Model) {
        setPerson1Model(routes[0].model);
        setPerson1Provider(routes[0].provider);
      }
      if (routes[1] && !person2Model) {
        setPerson2Model(routes[1].model);
        setPerson2Provider(routes[1].provider);
      }
    }
  }, [models.length, person1Model, person2Model]);

  const handleMessage = useCallback((msg: GroupMessage) => {
    setMessages((prev) => [...prev, msg]);
  }, []);

  const handleUpdate = useCallback((id: string, content: string, role: ChatRole) => {
    setMessages((prev) =>
      prev.map((msg) =>
        msg.id === id
          ? { ...msg, content: msg.content + content, role }
          : msg,
      ),
    );
  }, []);

  const person1 = usePersonChat(
    "person1",
    streamId,
    person1Model,
    person1Provider,
    handleMessage,
    handleUpdate,
  );

  const person2 = usePersonChat(
    "person2",
    streamId,
    person2Model,
    person2Provider,
    handleMessage,
    handleUpdate,
  );

  // Persist messages to uiStore
  useEffect(() => {
    uiStore.set(`/streams/${streamId}/group_messages`, messages);
  }, [streamId, messages]);

  return (
    <div className="flex flex-col gap-4 h-full min-h-[400px]">
      {props.title && (
        <h2 className="text-lg font-medium text-neutral-200">{props.title}</h2>
      )}
      
      <div className="flex gap-4 flex-1 min-h-0">
        <PersonPanel
          personId="person1"
          label={props.person1Label || "Person 1"}
          color="border-sky-800"
          messages={messages}
          input={person1.input}
          sending={person1.sending}
          onInputChange={person1.handleInputChange}
          onSubmit={person1.handleSubmit}
          selectedModel={person1Model}
          selectedProvider={person1Provider}
          onModelChange={(m, p) => {
            setPerson1Model(m);
            setPerson1Provider(p);
          }}
          models={models}
        />
        
        <PersonPanel
          personId="person2"
          label={props.person2Label || "Person 2"}
          color="border-purple-800"
          messages={messages}
          input={person2.input}
          sending={person2.sending}
          onInputChange={person2.handleInputChange}
          onSubmit={person2.handleSubmit}
          selectedModel={person2Model}
          selectedProvider={person2Provider}
          onModelChange={(m, p) => {
            setPerson2Model(m);
            setPerson2Provider(p);
          }}
          models={models}
        />
      </div>
    </div>
  );
};
