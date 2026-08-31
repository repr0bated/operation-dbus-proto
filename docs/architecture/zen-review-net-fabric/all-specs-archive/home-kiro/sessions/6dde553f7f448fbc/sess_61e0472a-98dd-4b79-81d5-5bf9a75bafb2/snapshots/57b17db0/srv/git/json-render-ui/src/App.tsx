import { useEffect } from "react";
import { useEventStream } from "./stream/use-event-stream";
import { ChatbotLayout } from "./layouts/ChatbotLayout";
import { NAVIGATION_SYSTEM_PROMPT } from "./prompts/navigation-system-prompt";

export function App() {
  useEventStream();

  useEffect(() => {
    window.addEventListener("popstate", () => {
      // Could add navigation handling here if needed
    });
    return () => window.removeEventListener("popstate", () => {});
  }, []);

  return (
    <ChatbotLayout
      streamId="stream_antigravity_01"
      mode="live"
      systemPrompt={NAVIGATION_SYSTEM_PROMPT}
    />
  );
}
