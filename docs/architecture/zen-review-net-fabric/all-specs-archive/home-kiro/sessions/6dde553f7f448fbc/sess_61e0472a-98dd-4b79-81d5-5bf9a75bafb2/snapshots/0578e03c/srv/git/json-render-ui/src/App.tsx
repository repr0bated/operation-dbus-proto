import { useState, useEffect } from "react";

interface Message {
  id: string;
  senderName: string;
  text: string;
  timestamp: string;
  isCurrentUser: boolean;
}

interface ModelRoute {
  id: string;
  name: string;
  available: boolean;
  provider: string;
  upstream_provider: string;
}

type InterfaceMode = "local-echo" | "zeroclaw" | "chat-service";

export function App() {
  const [messages, setMessages] = useState<Message[]>([
    {
      id: "1",
      senderName: "3tched Router",
      text: "Connected to tched_router D-Bus pipeline. Select a model and start chatting.",
      timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
      isCurrentUser: false,
    },
  ]);
  const [input, setInput] = useState("");
  const [typing, setTyping] = useState<string[]>([]);
  const [mode, setMode] = useState<InterfaceMode>("chat-service");
  const [models, setModels] = useState<ModelRoute[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [modelsLoading, setModelsLoading] = useState(true);

  // Load models on mount
  useEffect(() => {
    fetch("/api/llm/models")
      .then((res) => res.json())
      .then((data) => {
        const modelList = data.models || [];
        setModels(modelList);
        // Default to first available model
        const firstAvailable = modelList.find((m: ModelRoute) => m.available);
        if (firstAvailable) {
          setSelectedModel(firstAvailable.id);
        }
        setModelsLoading(false);
      })
      .catch((err) => {
        console.error("Failed to load models:", err);
        setModelsLoading(false);
      });
  }, []);

  const handleSend = () => {
    if (!input.trim()) return;

    const newMessage: Message = {
      id: Date.now().toString(),
      senderName: "You",
      text: input,
      timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
      isCurrentUser: true,
    };

    setMessages((prev) => [...prev, newMessage]);
    setInput("");

    // Route based on selected mode
    if (mode === "local-echo") {
      // Local echo demo
      setTyping(["User 1"]);
      setTimeout(() => {
        const response: Message = {
          id: (Date.now() + 1).toString(),
          senderName: "User 1",
          text: `You said: "${input}"`,
          timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
          isCurrentUser: false,
        };
        setMessages((prev) => [...prev, response]);
        setTyping([]);
      }, 1000);
    } else if (mode === "zeroclaw") {
      // Call zeroclaw A2A JSON-RPC endpoint
      setTyping(["Zeroclaw"]);
      const userMessage = input;
      fetch("http://127.0.0.1:8082/a2a/dashboard", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          jsonrpc: "2.0",
          id: `chat-${Date.now()}`,
          method: "message/send",
          params: {
            message: {
              role: "user",
              parts: [{ kind: "text", text: userMessage }],
            },
          },
        }),
      })
        .then((res) => res.json())
        .then((data) => {
          // Extract text from A2A response
          let responseText = "No response";
          if (data.result?.artifacts?.[0]?.parts?.[0]?.text) {
            responseText = data.result.artifacts[0].parts[0].text;
          } else if (data.error) {
            responseText = `Error: ${data.error.message || JSON.stringify(data.error)}`;
          }
          const response: Message = {
            id: (Date.now() + 1).toString(),
            senderName: "Zeroclaw",
            text: responseText,
            timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
            isCurrentUser: false,
          };
          setMessages((prev) => [...prev, response]);
          setTyping([]);
        })
        .catch((err) => {
          console.error("Zeroclaw error:", err);
          const response: Message = {
            id: (Date.now() + 1).toString(),
            senderName: "Zeroclaw",
            text: `Error: ${err.message}`,
            timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
            isCurrentUser: false,
          };
          setMessages((prev) => [...prev, response]);
          setTyping([]);
        });
    } else if (mode === "chat-service") {
      // Call op-web /api/chat endpoint with selected model
      setTyping(["3tched Router"]);
      const userMessage = input;
      fetch("/api/chat", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          message: userMessage,
          model: selectedModel || undefined,
        }),
      })
        .then((res) => res.json())
        .then((data) => {
          // Parse the response - may contain tool_code wrapper
          let responseText = data.content || data.message || "No response";
          // Try to extract respond_to_user content if present
          const match = responseText.match(/respond_to_user\(\{[^}]*"message"\s*:\s*"([^"]+)"/);
          if (match) {
            responseText = match[1].replace(/\\n/g, "\n");
          }
          const response: Message = {
            id: (Date.now() + 1).toString(),
            senderName: `3tched Router (${data.model || selectedModel})`,
            text: responseText,
            timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
            isCurrentUser: false,
          };
          setMessages((prev) => [...prev, response]);
          setTyping([]);
        })
        .catch((err) => {
          console.error("Chat service error:", err);
          const response: Message = {
            id: (Date.now() + 1).toString(),
            senderName: "3tched Router",
            text: `Error: ${err.message}`,
            timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
            isCurrentUser: false,
          };
          setMessages((prev) => [...prev, response]);
          setTyping([]);
        });
    }
  };

  return (
    <div className="min-h-screen bg-neutral-950 text-neutral-100 flex flex-col">
      <div className="bg-neutral-900 border-b border-neutral-800 p-4">
        <div className="flex justify-between items-center mb-3">
          <div>
            <h1 className="text-xl font-bold">3tched Router Chat</h1>
            <p className="text-sm text-neutral-400">
              {models.length} models available via D-Bus pipeline
            </p>
          </div>
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2">
              <label htmlFor="mode-select" className="text-sm text-neutral-400">
                Mode:
              </label>
              <select
                id="mode-select"
                value={mode}
                onChange={(e) => setMode(e.target.value as InterfaceMode)}
                className="bg-neutral-800 border border-neutral-700 rounded px-2 py-1 text-sm text-neutral-100 focus:outline-none focus:border-blue-600"
              >
                <option value="chat-service">3tched Router (D-Bus)</option>
                <option value="zeroclaw">Zeroclaw A2A</option>
                <option value="local-echo">Local Echo (Demo)</option>
              </select>
            </div>
            {mode === "chat-service" && (
              <div className="flex items-center gap-2">
                <label htmlFor="model-select" className="text-sm text-neutral-400">
                  Model:
                </label>
                <select
                  id="model-select"
                  value={selectedModel}
                  onChange={(e) => setSelectedModel(e.target.value)}
                  disabled={modelsLoading}
                  className="bg-neutral-800 border border-neutral-700 rounded px-2 py-1 text-sm text-neutral-100 focus:outline-none focus:border-blue-600 max-w-[200px]"
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
            )}
          </div>
        </div>
        <div className="text-xs text-neutral-500">
          {mode === "local-echo" && "Messages echo locally"}
          {mode === "zeroclaw" && "A2A JSON-RPC → 127.0.0.1:8082/a2a/dashboard"}
          {mode === "chat-service" && `D-Bus → /org/opdbus/v1/plugins/tched_router • ${selectedModel}`}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.map((msg) => (
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
                  ? "bg-blue-600 text-blue-100"
                  : "bg-neutral-800 text-neutral-100"
              }`}
            >
              <p className="text-sm">{msg.text}</p>
            </div>
            <span className="text-[10px] text-neutral-500 mt-1">{msg.timestamp}</span>
          </div>
        ))}

        {typing.length > 0 && (
          <div className="flex items-center gap-2 text-neutral-400 text-sm">
            <span>{typing.join(", ")} is typing</span>
            <span className="flex gap-1">
              <span className="w-2 h-2 bg-neutral-400 rounded-full animate-bounce"></span>
              <span className="w-2 h-2 bg-neutral-400 rounded-full animate-bounce" style={{ animationDelay: "0.2s" }}></span>
              <span className="w-2 h-2 bg-neutral-400 rounded-full animate-bounce" style={{ animationDelay: "0.4s" }}></span>
            </span>
          </div>
        )}
      </div>

      <div className="border-t border-neutral-800 p-4 bg-neutral-900">
        <div className="flex gap-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSend()}
            placeholder="Type a message..."
            className="flex-1 bg-neutral-800 border border-neutral-700 rounded px-3 py-2 text-neutral-100 placeholder-neutral-500 focus:outline-none focus:border-blue-600"
          />
          <button
            onClick={handleSend}
            disabled={!input.trim()}
            className="bg-blue-600 hover:bg-blue-700 disabled:bg-neutral-700 text-white px-4 py-2 rounded font-medium"
          >
            Send
          </button>
        </div>
      </div>
    </div>
  );
}
