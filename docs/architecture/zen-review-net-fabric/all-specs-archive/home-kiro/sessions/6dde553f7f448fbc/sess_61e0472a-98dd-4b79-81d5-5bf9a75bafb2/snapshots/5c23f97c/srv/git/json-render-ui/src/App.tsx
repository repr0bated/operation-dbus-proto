import { useEffect } from "react";
import { useEventStream } from "./stream/use-event-stream";
import { ChatbotLayout } from "./layouts/ChatbotLayout";
import { NAVIGATION_SYSTEM_PROMPT } from "./prompts/navigation-system-prompt";

export function App() {
  useEventStream();

  useEffect(() => {
    const onPop = () => {
      const path = window.location.pathname || "/";
      // Could add navigation handling here if needed
    };
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, []);

  return (
    <ChatbotLayout
      streamId="stream_antigravity_01"
      mode="live"
      systemPrompt={NAVIGATION_SYSTEM_PROMPT}
    />
  );
}
