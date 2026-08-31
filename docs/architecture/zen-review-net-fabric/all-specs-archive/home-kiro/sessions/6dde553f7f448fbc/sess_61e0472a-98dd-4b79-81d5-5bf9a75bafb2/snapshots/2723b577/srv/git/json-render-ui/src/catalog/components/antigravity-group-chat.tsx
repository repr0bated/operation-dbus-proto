/**
 * Antigravity Group Chat - User vs Chatbot
 * User selects which side they speak as, chatbot is designated.
 * Each side has its own model selector and identity for the tunnel connection.
 */
import { useCallback, useEffect, useRef, useState, type ChangeEvent, type FormEvent } from "react";
import { grpcServerStream, resolveIdentity } from "@/stream/grpc-transport";
import { decodeChatFrame, encodeSendRequest } from "@/stream/chat-codec";
import { uiStore } from "@/store/ui-store";
import type { El } from "./types";

type ChatRole = "user" | "agent" | "system";
type Side = "left" | "right";

interface GroupMessage {
  id: string;
  side: Side;
  role: ChatRole;
  content: string;
  model?: string;
  isBot: boolean;
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

/** Side panel - shows messages for one side */
function SidePanel({
  side,
  label,
  isBot,
  color,
  messages,
  input,
  sending,
  onInputChange,
  onSubmit,
  selectedModel,
  onModelChange,
  models,
  isSpeakingSide,
  onSelectSpeakingSide,
}: {
  side: Side;
  label: string;
  isBot: boolean;
  color: string;
  messages: GroupMessage[];
  input: string;
  sending: boolean;
  onInputChange: (e: ChangeEvent<HTMLInputElement>) => void;
  onSubmit: (e: FormEvent) => void;
  selectedModel: string;
  onModelChange: (model: string, provider: string) => void;
  models: ModelRoute[];
  isSpeakingSide: boolean;
  onSelectSpeakingSide: () => void;
}) {
  const sideMessages = messages.filter((m) => m.side === side);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [sideMessages]);

  return (
    <div className={`flex flex-col flex-1 min-w-0 rounded-lg border ${color} bg-neutral-950/30 p-3`}>
      <div className="flex items-center justify-between mb-2 shrink-0">
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-medium text-neutral-200">{label}</h3>
          {isBot && (
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-purple-900/50 text-purple-300 border border-purple-700">
              CHATBOT
            </span>
          )}
          {!isBot && (
            <button
              onClick={onSelectSpeakingSide}
              className={`text-[10px] px-1.5 py-0.5 rounded border transition-colors ${
                isSpeakingSide
                  ? "bg-sky-900/50 text-sky-300 border-sky-700"
                  : "bg-neutral-800/50 text-neutral-400 border-neutral-700 hover:border-neutral-600"
              }`}
            >
              {isSpeakingSide ? "SPEAKING AS" : "SPEAK AS"}
            </button>
          )}
        </div>
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
        {sideMessages.map((msg) => {
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
      
      {/* Input only shown for the side user is speaking as, OR for bot side */}
      {(isSpeakingSide || isBot) && (
        <form onSubmit={onSubmit} className="flex gap-2 mt-2">
          <input
            value={input}
            onChange={onInputChange}
            disabled={sending || (isBot && !isSpeakingSide)}
            placeholder={isBot ? "Chatbot responds automatically…" : `Type as ${label}…`}
            className="flex-1 bg-zinc-900 border border-zinc-800 text-white rounded p-2 text-sm focus:outline-none focus:ring-1 focus:ring-sky-500 disabled:opacity-50"
            readOnly={isBot}
          />
          {!isBot && (
            <button
              type="submit"
              disabled={sending || !input.trim()}
              className="bg-sky-700 px-4 py-2 text-sm text-white rounded disabled:opacity-50"
            >
              {sending ? "…" : "Send"}
            </button>
          )}
        </form>
      )}
    </div>
  );
}

/** Main Group Chat Container */
export const AntigravityGroupChatEl: El<"antigravityGroupChat"> = ({ props }) => {
  const streamId = props.streamId || "group_chat_01";
  const [messages, setMessages] = useState<GroupMessage[]>([]);
  const [models, setModels] = useState<ModelRoute[]>([]);
  
  // Which side is the chatbot (default: right)
  const botSide: Side = (props.botSide as Side) || "right";
  const userSide: Side = botSide === "right" ? "left" : "right";
  
  // Which side the user is currently speaking as (can only be non-bot side)
  const [speakingSide, setSpeakingSide] = useState<Side>(userSide);
  
  // Left side state
  const [leftModel, setLeftModel] = useState("");
  const [leftProvider, setLeftProvider] = useState("");
  const [leftInput, setLeftInput] = useState("");
  const [leftSending, setLeftSending] = useState(false);
  
  // Right side state
  const [rightModel, setRightModel] = useState("");
  const [rightProvider, setRightProvider] = useState("");
  const [rightInput, setRightInput] = useState("");
  const [rightSending, setRightSending] = useState(false);
  
  const abortRef = useRef<(() => void) | null>(null);
  const conversationIdRef = useRef(makeId(streamId));

  // Load models from tched_router
  useEffect(() => {
    const loadModels = async () => {
      try {
        const res = await fetch("/api/llm/models");
        if (res.ok) {
          const data = (await res.json()) as { models: ModelRoute[] };
          if (data.models?.length) {
            setModels(data.models);
            if (!leftModel && data.models[0]) {
              setLeftModel(data.models[0].model);
              setLeftProvider(data.models[0].provider);
            }
            if (!rightModel) {
              const m = data.models[1] || data.models[0];
              if (m) {
                setRightModel(m.model);
                setRightProvider(m.provider);
              }
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
      if (routes[0] && !leftModel) {
        setLeftModel(routes[0].model);
        setLeftProvider(routes[0].provider);
      }
      if (!rightModel) {
        const m = routes[1] || routes[0];
        if (m) {
          setRightModel(m.model);
          setRightProvider(m.provider);
        }
      }
    }
  }, [models.length, leftModel, rightModel]);

  // Send message and get chatbot response
  const sendMessage = useCallback(
    async (text: string, fromSide: Side) => {
      if (!text.trim()) return;
      
      const isFromBot = fromSide === botSide;
      const model = fromSide === "left" ? leftModel : rightModel;
      const provider = fromSide === "left" ? leftProvider : rightProvider;
      const setSending = fromSide === "left" ? setLeftSending : setRightSending;
      
      // User message
      const userMsg: GroupMessage = {
        id: makeId("user"),
        side: fromSide,
        role: "user",
        content: text,
        model,
        isBot: isFromBot,
      };
      
      // Bot response placeholder (on the OTHER side)
      const botResponseSide = fromSide === "left" ? "right" : "left";
      const botModel = botResponseSide === "left" ? leftModel : rightModel;
      const botProvider = botResponseSide === "left" ? leftProvider : rightProvider;
      const agentId = makeId("agent");
      const agentMsg: GroupMessage = {
        id: agentId,
        side: botResponseSide,
        role: "agent",
        content: "",
        model: botModel,
        isBot: botResponseSide === botSide,
      };

      setSending(true);
      setMessages((prev) => [...prev, userMsg, agentMsg]);
      abortRef.current?.();

      await resolveIdentity();

      const payloadMessages = [{ role: "user", content: text }];
      const uiMessages = new TextEncoder().encode(JSON.stringify(payloadMessages));
      const payload = encodeSendRequest({
        conversationId: conversationIdRef.current,
        uiMessages,
        provider: botProvider,
        model: botModel,
      });

      const { stream, abort } = grpcServerStream(
        "op_chat.chat.ChatService",
        "Send",
        payload,
      );
      console.log(`[group-chat] send from ${fromSide}: model=${botModel} provider=${botProvider}`);
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
                  ? { ...msg, content: msg.content + frame.text, role: mapRole(frame.role) }
                  : msg,
              ),
            );
          } else if (frame.kind === "error") {
            setMessages((prev) =>
              prev.map((msg) =>
                msg.id === agentId ? { ...msg, content: frame.message, role: "system" } : msg,
              ),
            );
          }
        }
      } catch (err) {
        const message = (err as Error).message;
        if (message !== "AbortError") {
          setMessages((prev) =>
            prev.map((msg) =>
              msg.id === agentId ? { ...msg, content: message, role: "system" } : msg,
            ),
          );
        }
      } finally {
        setSending(false);
        abortRef.current = null;
      }
    },
    [botSide, leftModel, leftProvider, rightModel, rightProvider],
  );

  const handleLeftSubmit = useCallback(
    (e: FormEvent) => {
      e.preventDefault();
      if (speakingSide === "left") {
        sendMessage(leftInput, "left");
        setLeftInput("");
      }
    },
    [speakingSide, leftInput, sendMessage],
  );

  const handleRightSubmit = useCallback(
    (e: FormEvent) => {
      e.preventDefault();
      if (speakingSide === "right") {
        sendMessage(rightInput, "right");
        setRightInput("");
      }
    },
    [speakingSide, rightInput, sendMessage],
  );

  // Cleanup on unmount
  useEffect(() => () => abortRef.current?.(), []);

  // Persist messages to uiStore
  useEffect(() => {
    uiStore.set(`/streams/${streamId}/group_messages`, messages);
  }, [streamId, messages]);

  const leftLabel = props.leftLabel || (botSide === "left" ? "Chatbot" : "You");
  const rightLabel = props.rightLabel || (botSide === "right" ? "Chatbot" : "You");

  return (
    <div className="flex flex-col gap-4 h-full min-h-[400px]">
      {props.title && (
        <h2 className="text-lg font-medium text-neutral-200">{props.title}</h2>
      )}
      
      <div className="flex gap-4 flex-1 min-h-0">
        <SidePanel
          side="left"
          label={leftLabel}
          isBot={botSide === "left"}
          color="border-sky-800"
          messages={messages}
          input={leftInput}
          sending={leftSending}
          onInputChange={(e) => setLeftInput(e.target.value)}
          onSubmit={handleLeftSubmit}
          selectedModel={leftModel}
          onModelChange={(m, p) => {
            setLeftModel(m);
            setLeftProvider(p);
          }}
          models={models}
          isSpeakingSide={speakingSide === "left"}
          onSelectSpeakingSide={() => setSpeakingSide("left")}
        />
        
        <SidePanel
          side="right"
          label={rightLabel}
          isBot={botSide === "right"}
          color="border-purple-800"
          messages={messages}
          input={rightInput}
          sending={rightSending}
          onInputChange={(e) => setRightInput(e.target.value)}
          onSubmit={handleRightSubmit}
          selectedModel={rightModel}
          onModelChange={(m, p) => {
            setRightModel(m);
            setRightProvider(p);
          }}
          models={models}
          isSpeakingSide={speakingSide === "right"}
          onSelectSpeakingSide={() => setSpeakingSide("right")}
        />
      </div>
    </div>
  );
};
