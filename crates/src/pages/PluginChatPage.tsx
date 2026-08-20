/**
 * PluginChatPage - Chat interface for generating plugin UIs with json-render
 * 
 * Uses useJsonRenderMessage from @json-render/react for spec extraction.
 */
import React from 'react';
import { PluginChatView } from '@/components/json-render';

export default function PluginChatPage() {
  return (
    <div className="h-[calc(100vh-4rem)]">
      <PluginChatView className="h-full" />
    </div>
  );
}
