import { useState } from "react";

interface Message {
  id: string;
  role: "user" | "assistant";
  content: string;
}

export function App() {
  const [messages, setMessages] = useState<Message[]>([
    {
      id: "1",
      role: "assistant",
      content: "Hi! I'm a chat interface connected to the antigravity SDK. Ask me about the system, plugins, or operations.",
    },
  ]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);

  const handleSend = async () => {
    if (!input.trim()) return;

    const userMessage: Message = {
      id: Date.now().toString(),
      role: "user",
      content: input,
    };

    setMessages((prev) => [...prev, userMessage]);
    setInput("");
    setLoading(true);

    // TODO: Send to actual chat service
    setTimeout(() => {
      const assistantMessage: Message = {
        id: (Date.now() + 1).toString(),
        role: "assistant",
        content: `You said: "${input}". Chat service not yet connected.`,
      };
      setMessages((prev) => [...prev, assistantMessage]);
      setLoading(false);
    }, 500);
  };

  return (
    <div className="min-h-screen bg-neutral-950 text-neutral-100 flex flex-col">
      <div className="bg-neutral-900 border-b border-neutral-800 p-4">
        <h1 className="text-xl font-bold">Antigravity Chat</h1>
        <p className="text-sm text-neutral-400">Connected to operation-dbus SDK</p>
      </div>

      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.map((msg) => (
          <div key={msg.id} className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}>
            <div
              className={`max-w-md px-4 py-2 rounded-lg ${
                msg.role === "user"
                  ? "bg-blue-900 text-blue-100"
                  : "bg-neutral-800 text-neutral-100"
              }`}
            >
              {msg.content}
            </div>
          </div>
        ))}
        {loading && (
          <div className="flex justify-start">
            <div className="bg-neutral-800 text-neutral-100 px-4 py-2 rounded-lg">
              Thinking...
            </div>
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
            placeholder="Ask about plugins, operations, or the system..."
            className="flex-1 bg-neutral-800 border border-neutral-700 rounded px-3 py-2 text-neutral-100 placeholder-neutral-500 focus:outline-none focus:border-blue-600"
          />
          <button
            onClick={handleSend}
            disabled={loading || !input.trim()}
            className="bg-blue-600 hover:bg-blue-700 disabled:bg-neutral-700 text-white px-4 py-2 rounded font-medium"
          >
            Send
          </button>
        </div>
      </div>
    </div>
  );
}
