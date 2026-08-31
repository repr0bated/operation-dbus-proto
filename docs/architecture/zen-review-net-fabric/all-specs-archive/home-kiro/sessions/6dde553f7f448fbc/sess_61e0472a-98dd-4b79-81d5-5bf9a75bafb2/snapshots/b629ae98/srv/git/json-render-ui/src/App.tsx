import { useState, useEffect, useCallback, useRef, type ChangeEvent, type FormEvent } from "react";
import { grpcServerStream, resolveIdentity } from "@/stream/grpc-transport";
import { decodeChatFrame, encodeSendRequest } from "@/stream/chat-codec";

interface Message {
  id: string;
  role: "user" | "assistant";
  text: string;
  timestamp: string;
}

interface ModelRoute {
  id: string;
  name: string;
  available: boolean;
  provider: string;
  upstream_provider: string;
}

function makeId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
}

export function App() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [models, setModels] = useState<ModelRoute[]>([]);
  const [modelsLoading, setModelsLoading] = useState(true);
  const [selectedModel, setSelectedModel] = useState("");
  const [selectedProvider, setSelectedProvider] = useState("tched");
  const abortRef = useRef<(() => void) | null>(null);
  const conversationId = useRef(makeId("chat"));
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Load models on mount
  useEffect(() => {
    fetch("/api/llm/models")
      .then((res) => res.json())
      .then((data) => {
        const modelList: ModelRoute[] = data.models || [];
        setModels(modelList);
        const available = modelList.filter((m) => m.available);
        if (available.length > 0) {
          setSelectedModel(available[0].id);
          setSelectedProvider(available[0].provider || "tched");
        }
        setModelsLoading(false);
      })
      .catch((err) => {
        console.error("Failed to load models:", err);
        setModelsLoading(false);
      });
  }, []);

  // Resolve identity on mount
  useEffect(() => {
    resolveIdentity();
  }, []);

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleInputChange = useCallback((event: ChangeEvent<HTMLTextAreaElement>) => {
    setInput(event.target.value);
  }, []);

  const handleSubmit = useCallback(
    async (event: FormEvent) => {
      event.preventDefault();
      const text = input.trim();
      if (!text || sending) return;

      const userMsg: Message = {
        id: makeId("user"),
        role: "user",
        text,
        timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
      };

      const assistantId = makeId("assistant");
      const assistantMsg: Message = {
        id: assistantId,
        role: "assistant",
        text: "",
        timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
      };

      setInput("");
      setSending(true);
      setMessages((prev) => [...prev, userMsg, assistantMsg]);
      abortRef.current?.();

      // Build conversation history
      const historyMessages = messages.map((m) => ({
        role: m.role,
        content: m.text,
      }));

      const payloadMessages = [
        {
          role: "system",
          content: "You are a helpful AI assistant. Be concise and helpful.",
        },
        ...historyMessages,
        { role: "user", content: text },
      ];

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
      console.log(`[chat] model=${selectedModel} provider=${selectedProvider} conversationId=${conversationId.current}`);
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
                msg.id === assistantId
                  ? { ...msg, text: `${msg.text}${frame.text}` }
                  : msg
              )
            );
          } else if (frame.kind === "error") {
            setMessages((prev) =>
              prev.map((msg) =>
                msg.id === assistantId
                  ? { ...msg, text: `Error: ${frame.message}` }
                  : msg
              )
            );
          }
        }
      } catch (err) {
        const message = (err as Error).message;
        if (message !== "AbortError") {
          setMessages((prev) =>
            prev.map((msg) =>
              msg.id === assistantId ? { ...msg, text: `Error: ${message}` } : msg
            )
          );
        }
      } finally {
        setSending(false);
        abortRef.current = null;
      }
    },
    [input, sending, selectedModel, selectedProvider, messages]
  );

  // Cleanup on unmount
  useEffect(() => () => abortRef.current?.(), []);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit(e as unknown as FormEvent);
    }
  };

  return (
    <div className="min-h-screen bg-neutral-950 text-neutral-100 flex flex-col">
      {/* Header */}
      <div className="bg-neutral-900 border-b border-neutral-800 p-4">
        <div className="flex justify-between items-center">
          <div>
            <h1 className="text-xl font-bold">Antigravity Chat</h1>
            <p className="text-sm text-neutral-400">
              {models.filter((m) => m.available).length} models available
            </p>
          </div>
          <select
            value={selectedModel}
            onChange={(e) => {
              const model = models.find((m) => m.id === e.target.value);
              setSelectedModel(e.target.value);
              if (model?.provider) setSelectedProvider(model.provider);
            }}
            disabled={modelsLoading}
            className="bg-neutral-800 border border-neutral-700 rounded px-3 py-2 text-sm text-neutral-100 focus:outline-none focus:border-sky-600"
          >
            {modelsLoading ? (
              <option>Loading...</option>
            ) : (
              models
                .filter((m) => m.available)
                .map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.id}
                  </option>
                ))
            )}
          </select>
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.length === 0 && (
          <div className="text-center text-neutral-500 py-8">
            Start a conversation with the AI assistant.
          </div>
        )}
        {messages.map((msg) => (
          <div
            key={msg.id}
            className={`flex flex-col ${msg.role === "user" ? "items-end" : "items-start"}`}
          >
            <span className="text-xs font-semibold text-neutral-400 mb-1">
              {msg.role === "user" ? "You" : "Assistant"}
            </span>
            <div
              className={`max-w-2xl px-4 py-2 rounded-lg ${
                msg.role === "user"
                  ? "bg-sky-600 text-white"
                  : "bg-neutral-800 text-neutral-100"
              }`}
            >
              <p className="text-sm whitespace-pre-wrap">{msg.text || "…"}</p>
            </div>
            <span className="text-[10px] text-neutral-500 mt-1">{msg.timestamp}</span>
          </div>
        ))}

        {sending && (
          <div className="flex items-center gap-2 text-neutral-400 text-sm">
            <span>Assistant is typing</span>
            <span className="flex gap-1">
              <span className="w-2 h-2 bg-neutral-400 rounded-full animate-bounce"></span>
              <span className="w-2 h-2 bg-neutral-400 rounded-full animate-bounce" style={{ animationDelay: "0.2s" }}></span>
              <span className="w-2 h-2 bg-neutral-400 rounded-full animate-bounce" style={{ animationDelay: "0.4s" }}></span>
            </span>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="border-t border-neutral-800 p-4 bg-neutral-900">
        <form onSubmit={handleSubmit} className="flex gap-2">
          <textarea
            value={input}
            onChange={handleInputChange}
            onKeyDown={handleKeyDown}
            placeholder="Type a message..."
            disabled={sending}
            rows={1}
            className="flex-1 bg-neutral-800 border border-neutral-700 rounded px-3 py-2 text-neutral-100 placeholder-neutral-500 focus:outline-none focus:border-sky-600 disabled:opacity-50 resize-none"
          />
          <button
            type="submit"
            disabled={!input.trim() || sending}
            className="bg-sky-600 hover:bg-sky-500 disabled:bg-neutral-700 text-white px-4 py-2 rounded font-medium"
          >
            Send
          </button>
        </form>
      </div>
    </div>
  );
}
