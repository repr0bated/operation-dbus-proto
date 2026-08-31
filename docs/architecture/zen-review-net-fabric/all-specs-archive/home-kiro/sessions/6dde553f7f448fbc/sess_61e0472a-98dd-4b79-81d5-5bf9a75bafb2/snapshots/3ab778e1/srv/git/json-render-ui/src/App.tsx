import { Renderer } from "@json-render/react";
import { useStateStore } from "./store/state-store";
import { useEventStream } from "./stream/use-event-stream";
import { defineRegistry } from "@json-render/react";
import { defineCatalog } from "@json-render/core";
import { z } from "zod";
import { useState } from "react";

// Define catalog for group chat components
const chatCatalog = defineCatalog(
  { root: z.string(), elements: z.record(z.any()) },
  {
    components: {
      chatContainer: { slots: ["default"] },
      chatHeader: {
        props: z.object({
          title: z.string(),
          memberCount: z.number(),
        }),
      },
      chatMessage: {
        props: z.object({
          id: z.string(),
          senderName: z.string(),
          text: z.string(),
          timestamp: z.string(),
          isCurrentUser: z.boolean(),
        }),
      },
      chatMessages: { slots: ["default"] },
      chatInput: {
        props: z.object({
          placeholder: z.string(),
        }),
      },
      activeTyping: {
        props: z.object({
          users: z.array(z.string()),
        }),
      },
    },
  }
);

// Register components
const chatRegistry = defineRegistry(chatCatalog, {
  components: {
    chatContainer: ({ slots }) => (
      <div className="min-h-screen bg-neutral-950 text-neutral-100 flex flex-col">
        {slots?.default}
      </div>
    ),
    chatHeader: ({ props }) => (
      <div className="bg-neutral-900 border-b border-neutral-800 p-4">
        <h1 className="text-xl font-bold">{props.title}</h1>
        <p className="text-sm text-neutral-400">{props.memberCount} participants</p>
      </div>
    ),
    chatMessage: ({ props }) => (
      <div className={`flex flex-col my-2 px-4 py-2 max-w-md ${props.isCurrentUser ? "ml-auto" : "mr-auto"}`}>
        <span className="text-xs font-semibold text-neutral-400">{props.senderName}</span>
        <div
          className={`px-3 py-2 rounded-lg ${
            props.isCurrentUser ? "bg-blue-600 text-blue-100" : "bg-neutral-800 text-neutral-100"
          }`}
        >
          <p className="text-sm">{props.text}</p>
        </div>
        <span className="text-[10px] text-neutral-500 mt-1">{props.timestamp}</span>
      </div>
    ),
    chatMessages: ({ slots }) => (
      <div className="flex-1 overflow-y-auto p-4 space-y-2">{slots?.default}</div>
    ),
    chatInput: ({ props }) => (
      <div className="border-t border-neutral-800 p-4 bg-neutral-900">
        <input
          type="text"
          placeholder={props.placeholder}
          className="w-full bg-neutral-800 border border-neutral-700 rounded px-3 py-2 text-neutral-100 placeholder-neutral-500 focus:outline-none focus:border-blue-600"
        />
      </div>
    ),
    activeTyping: ({ props }) => (
      <div className="px-4 py-2 text-xs text-neutral-500">
        {props.users.length > 0 && `${props.users.join(", ")} is typing...`}
      </div>
    ),
  },
});

const initialSpec = {
  root: "container",
  elements: {
    container: {
      type: "chatContainer",
      slots: { default: ["header", "messages", "typing", "input"] },
    },
    header: {
      type: "chatHeader",
      props: {
        title: "Group Chat",
        memberCount: 2,
      },
    },
    messages: {
      type: "chatMessages",
      slots: {
        default: ["msg1", "msg2"],
      },
    },
    msg1: {
      type: "chatMessage",
      props: {
        id: "msg_1",
        senderName: "User 1",
        text: "Hey! How are you?",
        timestamp: "10:30 AM",
        isCurrentUser: false,
      },
    },
    msg2: {
      type: "chatMessage",
      props: {
        id: "msg_2",
        senderName: "You",
        text: "Good, just working on the dashboard!",
        timestamp: "10:31 AM",
        isCurrentUser: true,
      },
    },
    typing: {
      type: "activeTyping",
      props: {
        users: [],
      },
    },
    input: {
      type: "chatInput",
      props: {
        placeholder: "Type a message...",
      },
    },
  },
};

export function App() {
  useEventStream();
  const connected = useStateStore((s) => s.connected);

  return (
    <div>
      {!connected && (
        <div className="fixed top-0 inset-x-0 z-50 bg-amber-900/80 text-amber-100 text-xs text-center py-1">
          Connecting to event stream...
        </div>
      )}
      <Renderer spec={initialSpec} registry={chatRegistry.registry} />
    </div>
  );
}
