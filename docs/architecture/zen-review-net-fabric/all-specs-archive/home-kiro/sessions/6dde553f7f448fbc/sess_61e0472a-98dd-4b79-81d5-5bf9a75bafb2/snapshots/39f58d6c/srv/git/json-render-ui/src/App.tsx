import { useState } from "react";

interface Message {
  id: string;
  senderName: string;
  text: string;
  timestamp: string;
  isCurrentUser: boolean;
}

export function App() {
  const [messages, setMessages] = useState<Message[]>([
    {
      id: "1",
      senderName: "User 1",
      text: "Hey! How are you?",
      timestamp: "10:30 AM",
      isCurrentUser: false,
    },
    {
      id: "2",
      senderName: "You",
      text: "Good, just working on the dashboard!",
      timestamp: "10:31 AM",
      isCurrentUser: true,
    },
  ]);
  const [input, setInput] = useState("");
  const [typing, setTyping] = useState<string[]>([]);

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

    // Simulate other user typing
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
  };

  return (
    <div className="min-h-screen bg-neutral-950 text-neutral-100 flex flex-col">
      <div className="bg-neutral-900 border-b border-neutral-800 p-4">
        <h1 className="text-xl font-bold">Group Chat</h1>
        <p className="text-sm text-neutral-400">2 participants</p>
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
