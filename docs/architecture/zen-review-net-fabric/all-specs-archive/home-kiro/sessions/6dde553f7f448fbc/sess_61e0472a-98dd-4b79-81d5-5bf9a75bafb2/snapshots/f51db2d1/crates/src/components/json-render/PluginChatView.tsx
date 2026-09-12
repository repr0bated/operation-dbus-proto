/**
 * PluginChatView - Chat interface with json-render UI extraction
 *
 * Uses useJsonRenderMessage from @json-render/react to extract specs
 * from AI SDK message parts, combined with ai-elements for chat UI.
 */
import React, { useState, FormEvent, useRef, useEffect, useCallback } from 'react';
import { useJsonRenderMessage, Renderer, JSONUIProvider, type DataPart } from '@json-render/react';
import { createSpecStreamCompiler, type Spec } from '@json-render/core';
import { registry } from './registry';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import { Send, Loader2, RefreshCw, Sparkles, Code, Copy, Check, ThumbsUp, ThumbsDown } from 'lucide-react';
import { streamChat } from '@/api/client';

// ai-elements components
import {
  Message,
  MessageContent as AIMessageContent,
  MessageResponse,
  MessageActions,
  MessageAction,
  MessageToolbar,
} from '@/components/ai-elements/message';
import {
  PromptInput,
  PromptInputTextarea,
  PromptInputFooter,
  PromptInputTools,
  PromptInputActions,
  PromptInputSubmitButton,
} from '@/components/ai-elements/prompt-input';

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

/**
 * Message with AI SDK compatible parts array
 */
interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  /** AI SDK compatible parts array for useJsonRenderMessage */
  parts: DataPart[];
}

/**
 * Convert raw text content to DataPart array.
 * Attempts to extract json-render spec from code fences or raw JSON.
 */
function contentToParts(content: string): DataPart[] {
  const parts: DataPart[] = [];
  
  if (!content) return parts;

  // Try to find JSON spec in code fences
  const fenceMatch = content.match(/```(?:json)?\s*([\s\S]*?)```/);
  let specJson: string | null = null;
  let textContent = content;

  if (fenceMatch) {
    specJson = fenceMatch[1].trim();
    const fenceIndex = content.indexOf('```');
    textContent = content.slice(0, fenceIndex).trim();
  } else {
    // Try to find raw JSON object with spec structure
    const jsonMatch = content.match(/\{[\s\S]*"root"[\s\S]*"elements"[\s\S]*\}/);
    if (jsonMatch) {
      specJson = jsonMatch[0];
      const jsonIndex = content.indexOf(specJson);
      textContent = content.slice(0, jsonIndex).trim();
    }
  }

  // Add text part if present
  if (textContent) {
    parts.push({ type: 'text', text: textContent });
  }

  // Try to parse and add data part for spec
  if (specJson) {
    try {
      const parsed = JSON.parse(specJson);
      if (parsed && typeof parsed === 'object' && 'root' in parsed && 'elements' in parsed) {
        parts.push({ type: 'data', data: parsed });
      }
    } catch {
      // Try streaming compiler for partial JSON
      try {
        const compiler = createSpecStreamCompiler<Spec>();
        const { result } = compiler.push(specJson);
        if (result && 'root' in result && 'elements' in result) {
          parts.push({ type: 'data', data: result });
        }
      } catch {
        // Fall through - no valid spec
      }
    }
  }

  return parts;
}

/**
 * Component to render a single message with json-render extraction
 */
function MessageContent({ message }: { message: ChatMessage }) {
  // Use json-render's hook to extract spec from parts
  const { spec, text, hasSpec } = useJsonRenderMessage(message.parts);
  const [showRaw, setShowRaw] = useState(false);

  const handleAction = useCallback((actionName: string, params?: Record<string, unknown>) => {
    console.log('[PluginChatView] Action:', actionName, params);
  }, []);

  return (
    <div className="flex-1 space-y-3 min-w-0">
      {/* Text content */}
      {text && (
        <p className="text-sm whitespace-pre-wrap">{text}</p>
      )}

      {/* Rendered spec */}
      {hasSpec && spec && (
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Badge variant="outline" className="text-xs">
              <Sparkles className="w-3 h-3 mr-1" />
              Generated UI
            </Badge>
            <Button
              variant="ghost"
              size="sm"
              className="h-6 px-2 text-xs"
              onClick={() => setShowRaw(!showRaw)}
            >
              <Code className="w-3 h-3 mr-1" />
              {showRaw ? 'Hide' : 'Show'} JSON
            </Button>
          </div>

          {showRaw ? (
            <pre className="p-3 bg-muted rounded-lg text-xs overflow-auto max-h-64">
              {JSON.stringify(spec, null, 2)}
            </pre>
          ) : (
            <div className="border rounded-lg p-4 bg-card">
              <JSONUIProvider
                registry={registry}
                handlers={{
                  dispatch: async (params) => handleAction('dispatch', params),
                }}
              >
                <Renderer spec={spec} />
              </JSONUIProvider>
            </div>
          )}
        </div>
      )}

      {/* Show raw content if no spec extracted */}
      {!hasSpec && !text && message.content && (
        <p className="text-sm whitespace-pre-wrap text-muted-foreground">
          {message.content}
        </p>
      )}
    </div>
  );
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
      parts: [{ type: 'text', text: input }],
    };

    setMessages((prev) => [...prev, userMessage]);
    setInput('');
    setIsLoading(true);
    setError(null);

    // Create assistant message placeholder
    const assistantId = `assistant-${Date.now()}`;
    setMessages((prev) => [
      ...prev,
      { id: assistantId, role: 'assistant', content: '', parts: [] },
    ]);

    // Stream the response
    let fullContent = '';
    abortRef.current = streamChat(
      input,
      undefined,
      (chunk) => {
        fullContent += chunk;
        // Update message with accumulated content and parsed parts
        const parts = contentToParts(fullContent);
        setMessages((prev) =>
          prev.map((msg) =>
            msg.id === assistantId
              ? { ...msg, content: fullContent, parts }
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
    
    // Build prompt using json-render's buildUserPrompt for proper catalog context
    const prompt = `Generate a json-render UI spec for the "${plugin}" plugin.

The spec should include:
- A PluginCard with the plugin ID and status
- A MetricGrid showing key metrics
- StatusBadge for current state
- ActionGroup with common operations

Use the available components: Card, Badge, Button, Text, Row, Column, Metric, Progress, Alert, PluginCard, MetricGrid, StatusBadge, ActionGroup.

Return a valid JSON spec with { root, elements } structure where:
- root: string key of the root element
- elements: Record<string, { type, props, children? }> mapping`;
    
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
                Uses <code className="text-xs bg-muted px-1 rounded">useJsonRenderMessage</code> for spec extraction.
              </p>
            </div>
          ) : (
            <div className="divide-y">
              {messages.map((message) => (
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
                  
                  {message.role === 'user' ? (
                    <div className="flex-1">
                      <p className="text-sm whitespace-pre-wrap">{message.content}</p>
                    </div>
                  ) : (
                    <MessageContent message={message} />
                  )}

                  {/* Show streaming indicator */}
                  {message.role === 'assistant' &&
                    !message.content &&
                    isLoading && (
                      <div className="flex items-center gap-2 text-muted-foreground">
                        <Loader2 className="w-4 h-4 animate-spin" />
                        <span className="text-sm">Generating...</span>
                      </div>
                    )}
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
