/**
 * PluginChatView - Chat interface with json-render UI extraction
 *
 * Uses AI SDK's useChat pattern combined with json-render's streaming
 * to extract and render UI specs from assistant messages.
 */
import React, { useState, FormEvent, useRef, useEffect, useMemo } from 'react';
import { createSpecStreamCompiler, type Spec } from '@json-render/core';
import { Renderer, JSONUIProvider } from '@json-render/react';
import { registry } from './registry';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import { Send, Loader2, RefreshCw, Bot, User, Sparkles } from 'lucide-react';
import { streamChat } from '@/api/client';

// Plugin catalog for quick selection
const PLUGIN_CATALOG = [
  'adc', 'agentConfig', 'antigravity', 'snowball', 'btrfs',
  'cognitiveMcp', 'compactMcp', 'config', 'cron', 'ctlPlaneChatbot',
  'datastore', 'dnsresolver', 'embeddingModel', 'emqx', 'endpoint',
  'factory', 'fail2ban', 'freedesktop', 'fullSystem', 'gcloudAdc',
  'gemmaBrain', 'ghostbridge', 'hardware', 'hostRuntime', 'humanPrincipal',
  'identitySled', 'incus', 'jsonRender', 'keypair', 'keyring',
  'largeLanguageModel', 'login1', 'mailServer', 'mcp', 'memory',
  'net', 'netmaker', 'notebooklm', 'oci', 'openflow',
  'openflowObfuscation', 'oscalSubidRegistry', 'ovsdbBridge', 'packagekit', 'pcidecl',
  'persona', 'privacyRoutes', 'procfs', 'proxyServer', 'qdrant',
  'rovsCommands', 'rtnetlink', 'schemaRenderer', 'service', 'sessDecl',
  'sharedUnixSocket', 'software', 'tchedRouter', 'unixSocket', 'users',
  'webUi', 'wgOpdbus', 'wgcf', 'wireguard', 'workflows', 'xray',
] as const;

interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
}

interface ParsedMessage extends ChatMessage {
  spec: Spec | null;
  textContent: string;
}

/**
 * Extract json-render spec from message content.
 * Tolerates prose preamble, code fences, and partial JSON.
 */
function extractSpec(content: string): { spec: Spec | null; textContent: string } {
  if (!content) return { spec: null, textContent: '' };

  // Try to find JSON in code fences first
  const fenceMatch = content.match(/```(?:json)?\s*([\s\S]*?)```/);
  let jsonStr = fenceMatch ? fenceMatch[1].trim() : null;
  let textContent = content;

  // If no code fence, try to find raw JSON object
  if (!jsonStr) {
    const jsonMatch = content.match(/\{[\s\S]*"root"[\s\S]*"elements"[\s\S]*\}/);
    if (jsonMatch) {
      jsonStr = jsonMatch[0];
      // Extract text before the JSON
      const jsonIndex = content.indexOf(jsonStr);
      textContent = content.slice(0, jsonIndex).trim();
    }
  } else {
    // Extract text before the code fence
    const fenceIndex = content.indexOf('```');
    textContent = content.slice(0, fenceIndex).trim();
  }

  if (!jsonStr) {
    return { spec: null, textContent: content };
  }

  try {
    const parsed = JSON.parse(jsonStr);
    // Validate it looks like a spec
    if (parsed && typeof parsed === 'object' && 'root' in parsed && 'elements' in parsed) {
      return { spec: parsed as Spec, textContent };
    }
  } catch {
    // Try partial/streaming parse with json-render compiler
    try {
      const compiler = createSpecStreamCompiler<Spec>();
      const { result } = compiler.push(jsonStr);
      if (result && 'root' in result && 'elements' in result) {
        return { spec: result, textContent };
      }
    } catch {
      // Fall through
    }
  }

  return { spec: null, textContent: content };
}

interface PluginChatViewProps {
  /** Initial plugin to focus on */
  initialPlugin?: string;
  /** Custom class name */
  className?: string;
}

export function PluginChatView({
  initialPlugin = 'tchedRouter',
  className,
}: PluginChatViewProps) {
  const [input, setInput] = useState('');
  const [selectedPlugin, setSelectedPlugin] = useState(initialPlugin);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const abortRef = useRef<AbortController | null>(null);

  // Parse specs from messages
  const parsedMessages = useMemo<ParsedMessage[]>(() => {
    return messages.map((msg) => {
      if (msg.role === 'user') {
        return { ...msg, spec: null, textContent: msg.content };
      }
      const { spec, textContent } = extractSpec(msg.content);
      return { ...msg, spec, textContent };
    });
  }, [messages]);

  // Auto-scroll on new messages
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!input.trim() || isLoading) return;

    const userMessage: ChatMessage = {
      id: `user-${Date.now()}`,
      role: 'user',
      content: input,
    };

    setMessages((prev) => [...prev, userMessage]);
    setInput('');
    setIsLoading(true);
    setError(null);

    // Create assistant message placeholder
    const assistantId = `assistant-${Date.now()}`;
    setMessages((prev) => [
      ...prev,
      { id: assistantId, role: 'assistant', content: '' },
    ]);

    // Stream the response
    abortRef.current = streamChat(
      input,
      undefined,
      (chunk) => {
        // Append chunk to assistant message
        setMessages((prev) =>
          prev.map((msg) =>
            msg.id === assistantId
              ? { ...msg, content: msg.content + chunk }
              : msg
          )
        );
      },
      () => {
        setIsLoading(false);
        abortRef.current = null;
      }
    );
  };

  const handlePluginSelect = (plugin: string) => {
    setSelectedPlugin(plugin);
    // Generate a prompt for this plugin
    const prompt = `Generate a json-render UI spec for the "${plugin}" plugin. The spec should include:
- A Card with the plugin name as title
- Status indicator (Badge showing state)
- Key metrics or configuration values
- Action buttons for common operations

Return a JSON object with { root, elements } structure.`;
    
    setInput(prompt);
  };

  const handleStop = () => {
    if (abortRef.current) {
      abortRef.current.abort();
      setIsLoading(false);
    }
  };

  const handleClear = () => {
    setMessages([]);
    setError(null);
  };

  const handleAction = (actionName: string, params?: Record<string, unknown>) => {
    console.log('[PluginChatView] Action:', actionName, params);
  };

  return (
    <div className={cn('flex h-full', className)}>
      {/* Plugin sidebar */}
      <div className="w-48 border-r flex flex-col">
        <div className="p-3 border-b">
          <h3 className="font-semibold text-sm flex items-center gap-2">
            <Sparkles className="w-4 h-4" />
            Plugins
          </h3>
        </div>
        <ScrollArea className="flex-1">
          <div className="p-2 space-y-1">
            {PLUGIN_CATALOG.map((plugin) => (
              <Button
                key={plugin}
                variant={selectedPlugin === plugin ? 'secondary' : 'ghost'}
                size="sm"
                className="w-full justify-start text-xs font-mono h-7"
                onClick={() => handlePluginSelect(plugin)}
              >
                {plugin}
              </Button>
            ))}
          </div>
        </ScrollArea>
      </div>

      {/* Chat area */}
      <div className="flex-1 flex flex-col">
        {/* Header */}
        <div className="p-3 border-b flex items-center justify-between">
          <div className="flex items-center gap-2">
            <h2 className="font-semibold">Plugin UI Generator</h2>
            <Badge variant="outline" className="font-mono">
              {selectedPlugin}
            </Badge>
          </div>
          <Button
            variant="ghost"
            size="sm"
            onClick={handleClear}
            disabled={messages.length === 0}
          >
            <RefreshCw className="w-4 h-4" />
          </Button>
        </div>

        {/* Messages */}
        <ScrollArea ref={scrollRef} className="flex-1">
          {messages.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full p-8 text-muted-foreground">
              <Bot className="w-12 h-12 mb-4" />
              <p className="text-center mb-2">
                Select a plugin from the sidebar to generate a UI spec.
              </p>
              <p className="text-center text-sm">
                Or describe what you want to build.
              </p>
            </div>
          ) : (
            <div className="divide-y">
              {parsedMessages.map((message) => (
                <div
                  key={message.id}
                  className={cn(
                    'flex gap-3 p-4',
                    message.role === 'user' ? 'bg-muted/50' : 'bg-background'
                  )}
                >
                  <div
                    className={cn(
                      'flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center',
                      message.role === 'user'
                        ? 'bg-primary text-primary-foreground'
                        : 'bg-secondary'
                    )}
                  >
                    {message.role === 'user' ? (
                      <User className="w-4 h-4" />
                    ) : (
                      <Bot className="w-4 h-4" />
                    )}
                  </div>
                  <div className="flex-1 space-y-3 min-w-0">
                    {/* Text content */}
                    {message.textContent && (
                      <p className="text-sm whitespace-pre-wrap">
                        {message.textContent}
                      </p>
                    )}

                    {/* Rendered spec */}
                    {message.spec && (
                      <div className="border rounded-lg p-4 bg-card">
                        <JSONUIProvider
                          registry={registry}
                          handlers={{
                            dispatch: async (params) => handleAction('dispatch', params),
                          }}
                        >
                          <Renderer
                            spec={message.spec}
                            registry={registry}
                          />
                        </JSONUIProvider>
                      </div>
                    )}

                    {/* Show streaming indicator for assistant without spec yet */}
                    {message.role === 'assistant' &&
                      !message.spec &&
                      !message.content &&
                      isLoading && (
                        <div className="flex items-center gap-2 text-muted-foreground">
                          <Loader2 className="w-4 h-4 animate-spin" />
                          <span className="text-sm">Generating...</span>
                        </div>
                      )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </ScrollArea>

        {/* Error display */}
        {error && (
          <div className="p-3 bg-destructive/10 text-destructive text-sm">
            Error: {error}
          </div>
        )}

        {/* Input */}
        <form onSubmit={handleSubmit} className="p-3 border-t flex gap-2">
          <Input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder={`Describe the UI you want for ${selectedPlugin}...`}
            disabled={isLoading}
            className="flex-1"
          />
          {isLoading ? (
            <Button type="button" variant="outline" onClick={handleStop}>
              Stop
            </Button>
          ) : (
            <Button type="submit" disabled={!input.trim()}>
              <Send className="w-4 h-4" />
            </Button>
          )}
        </form>
      </div>
    </div>
  );
}

export default PluginChatView;
