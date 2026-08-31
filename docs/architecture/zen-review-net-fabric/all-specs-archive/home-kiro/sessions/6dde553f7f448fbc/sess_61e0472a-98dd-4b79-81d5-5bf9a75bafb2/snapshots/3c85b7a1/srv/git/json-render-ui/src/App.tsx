import { useState, useEffect, useCallback, useRef, type ChangeEvent, type FormEvent } from "react";
import { grpcServerStream, resolveIdentity } from "@/stream/grpc-transport";
import { decodeChatFrame, encodeSendRequest } from "@/stream/chat-codec";

interface Message {
  id: string;
  senderName: string;
  text: string;
  timestamp: string;
  isCurrentUser: boolean;
  personIndex: number; // 0 or 1
}

interface ModelRoute {
  id: string;
  name: string;
  available: boolean;
  provider: string;
  upstream_provider: string;
}

interface Person {
  name: string;
  model: string;
  provider: string;
}

function makeId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
}

export function App() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [activePerson, setActivePerson] = useState(0); // 0 = Person 1, 1 = Person 2
  const [sending, setSending] = useState(false);
  const [models, setModels] = useState<ModelRoute[]>([]);
  const [modelsLoading, setModelsLoading] = useState(true);
  const [persons, setPersons] = useState<Person[]>([
    { name: "Person 1", model: "", provider: "tched" },
    { name: "Person 2", model: "", provider: "tched" },
  ]);
  const abortRef = useRef<(() => void) | null>(null);
  const conversationId = useRef(makeId("group"));

  // Load models on mount
  useEffect(() => {
    fetch("/api/llm/models")
      .then((res) => res.json())
      .then((data) => {
        const modelList: ModelRoute[] = data.models || [];
        setModels(modelList);
        // Default both persons to first two available models
        const available = modelList.filter((m) => m.available);
        if (available.length >= 2) {
          setPersons([
            { name: "Person 1", model: available[0].id, provider: available[0].provider || "tched" },
            { name: "Person 2", model: available[1].id, provider: available[1].provider || "tched" },
          ]);
        } else if (available.length === 1) {
          setPersons([
            { name: "Person 1", model: available[0].id, provider: available[0].provider || "tched" },
            { name: "Person 2", model: available[0].id, provider: available[0].provider || "tched" },
          ]);
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

  const updatePerson = (index: number, field: keyof Person, value: string) => {
    setPersons((prev) =>
      prev.map((p, i) => (i === index ? { ...p, [field]: value } : p))
    );
  };

  const handleInputChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setInput(event.target.value);
  }, []);

  const handleSubmit = useCallback(
    async (event: FormEvent) => {
      event.preventDefault();
      const text = input.trim();
      if (!text || sending) return;

      const person = persons[activePerson];
      const userMsg: Message = {
        id: makeId("user"),
        senderName: person.name,
        text,
        timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
        isCurrentUser: true,
        personIndex: activePerson,
      };

      // Agent response placeholder
      const agentId = makeId("agent");
      const modelName = person.model || "model";
      const agentMsg: Message = {
        id: agentId,
        senderName: `${person.name} (${modelName})`,
        text: "",
        timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
        isCurrentUser: false,
        personIndex: activePerson,
      };

      setInput("");
      setSending(true);
      setMessages((prev) => [...prev, userMsg, agentMsg]);
      abortRef.current?.();

      // Build conversation history for context
      const historyMessages = messages.map((m) => ({
        role: m.isCurrentUser ? "user" : "assistant",
        content: `[${m.senderName}]: ${m.text}`,
      }));
      
      const payloadMessages = [
        {
          role: "system",
          content: `You are assisting in a 2-person group chat. You are responding as ${person.name}'s assistant using model ${modelName}. Keep responses conversational and concise.`,
        },
        ...historyMessages,
        { role: "user", content: text },
      ];

      const uiMessages = new TextEncoder().encode(JSON.stringify(payloadMessages));
      const payload = encodeSendRequest({
        conversationId: conversationId.current,
        uiMessages,
        provider: person.provider,
        model: person.model,
      });

      const { stream, abort } = grpcServerStream(
        "op_chat.chat.ChatService",
        "Send",
        payload,
      );
      console.log(`[group-chat] person=${person.name} model=${person.model} conversationId=${conversationId.current}`);
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
                  ? { ...msg, text: `${msg.text}${frame.text}` }
                  : msg
              )
            );
          } else if (frame.kind === "error") {
            setMessages((prev) =>
              prev.map((msg) =>
                msg.id === agentId
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
              msg.id === agentId ? { ...msg, text: `Error: ${message}` } : msg
            )
          );
        }
      } finally {
        setSending(false);
        abortRef.current = null;
      }
    },
    [input, sending, activePerson, persons, messages]
  );

  // Cleanup on unmount
  useEffect(() => () => abortRef.current?.(), []);

  const personColors = [
    { bg: "bg-blue-600", border: "border-blue-500", text: "text-blue-100" },
    { bg: "bg-purple-600", border: "border-purple-500", text: "text-purple-100" },
  ];

  return (
    <div className="min-h-screen bg-neutral-950 text-neutral-100 flex flex-col">
      {/* Header */}
      <div className="bg-neutral-900 border-b border-neutral-800 p-4">
        <div className="flex justify-between items-center mb-4">
          <div>
            <h1 className="text-xl font-bold">Antigravity Group Chat</h1>
            <p className="text-sm text-neutral-400">
              2-person chat • {models.filter((m) => m.available).length} models available
            </p>
          </div>
        </div>

        {/* Person configuration */}
        <div className="grid grid-cols-2 gap-4">
          {persons.map((person, idx) => (
            <div
              key={idx}
              className={`p-3 rounded-lg border ${
                activePerson === idx
                  ? `${personColors[idx].border} bg-neutral-800`
                  : "border-neutral-700 bg-neutral-900"
              }`}
            >
              <div className="flex items-center gap-2 mb-2">
                <button
                  onClick={() => setActivePerson(idx)}
                  className={`w-4 h-4 rounded-full ${
                    activePerson === idx ? personColors[idx].bg : "bg-neutral-600"
                  }`}
                />
                <input
                  type="text"
                  value={person.name}
                  onChange={(e) => updatePerson(idx, "name", e.target.value)}
                  className="bg-transparent border-none text-sm font-medium focus:outline-none"
                />
              </div>
              <select
                value={person.model}
                onChange={(e) => {
                  const model = models.find((m) => m.id === e.target.value);
                  updatePerson(idx, "model", e.target.value);
                  if (model?.provider) updatePerson(idx, "provider", model.provider);
                }}
                disabled={modelsLoading}
                className="w-full bg-neutral-800 border border-neutral-700 rounded px-2 py-1 text-xs text-neutral-100 focus:outline-none focus:border-blue-600"
              >
                {modelsLoading ? (
                  <option>Loading models...</option>
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
          ))}
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.length === 0 && (
          <div className="text-center text-neutral-500 py-8">
            Select a person and start chatting. Each person can use a different model.
          </div>
        )}
        {messages.map((msg) => {
          const colors = personColors[msg.personIndex] || personColors[0];
          return (
            <div
              key={msg.id}
              className={`flex flex-col ${msg.isCurrentUser ? "items-end" : "items-start"}`}
            >
              <span className="text-xs font-semibold text-neutral-400 mb-1">
                {msg.senderName}
              </span>
              <div
                className={`max-w-md px-4 py-2 rounded-lg ${
                  msg.isCurrentUser
                    ? `${colors.bg} ${colors.text}`
                    : "bg-neutral-800 text-neutral-100"
                }`}
              >
                <p className="text-sm whitespace-pre-wrap">{msg.text || "…"}</p>
              </div>
              <span className="text-[10px] text-neutral-500 mt-1">{msg.timestamp}</span>
            </div>
          );
        })}

        {sending && (
          <div className="flex items-center gap-2 text-neutral-400 text-sm">
            <span>{persons[activePerson].name} is receiving response</span>
            <span className="flex gap-1">
              <span className="w-2 h-2 bg-neutral-400 rounded-full animate-bounce"></span>
              <span className="w-2 h-2 bg-neutral-400 rounded-full animate-bounce" style={{ animationDelay: "0.2s" }}></span>
              <span className="w-2 h-2 bg-neutral-400 rounded-full animate-bounce" style={{ animationDelay: "0.4s" }}></span>
            </span>
          </div>
        )}
      </div>

      {/* Input */}
      <div className="border-t border-neutral-800 p-4 bg-neutral-900">
        <div className="flex items-center gap-2 mb-2">
          <span className="text-xs text-neutral-400">Speaking as:</span>
          <div className="flex gap-2">
            {persons.map((person, idx) => (
              <button
                key={idx}
                onClick={() => setActivePerson(idx)}
                className={`px-3 py-1 text-xs rounded-full ${
                  activePerson === idx
                    ? `${personColors[idx].bg} text-white`
                    : "bg-neutral-700 text-neutral-300"
                }`}
              >
                {person.name}
              </button>
            ))}
          </div>
        </div>
        <form onSubmit={handleSubmit} className="flex gap-2">
          <input
            type="text"
            value={input}
            onChange={handleInputChange}
            onKeyDown={(e) => e.key === "Enter" && !e.shiftKey && handleSubmit(e)}
            placeholder={`Message as ${persons[activePerson].name}...`}
            disabled={sending}
            className="flex-1 bg-neutral-800 border border-neutral-700 rounded px-3 py-2 text-neutral-100 placeholder-neutral-500 focus:outline-none focus:border-blue-600 disabled:opacity-50"
          />
          <button
            type="submit"
            disabled={!input.trim() || sending}
            className={`${personColors[activePerson].bg} hover:opacity-90 disabled:bg-neutral-700 text-white px-4 py-2 rounded font-medium`}
          >
            Send
          </button>
        </form>
      </div>
    </div>
  );
}
