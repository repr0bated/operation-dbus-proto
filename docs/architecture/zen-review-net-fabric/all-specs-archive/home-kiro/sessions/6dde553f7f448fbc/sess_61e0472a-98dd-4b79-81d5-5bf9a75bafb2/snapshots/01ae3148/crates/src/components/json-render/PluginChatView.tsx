/**
 * PluginChatView - Chat interface with json-render UI extraction
 *
 * Renders specs directly using JSONUIProvider + Renderer (like JsonRenderView).
 * Uses ai-elements for chat UI components.
 */
import React, { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import { Renderer, JSONUIProvider, type Spec } from '@json-render/react';
import { registry } from './registry';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import { Loader2, RefreshCw, Sparkles, Code, Copy, Check, ThumbsUp, ThumbsDown } from 'lucide-react';
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
  PromptInputSubmit,
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
 * Message with extracted spec and text
 */
interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  /** Extracted json-render spec (if any) */
  spec: Spec | null;
  /** Text content (markdown) separate from spec */
  text: string;
}

/**
 * Parse content to extract spec and text separately.
 * Returns { spec, text } where spec is extracted JSON and text is remaining content.
 */
function parseContent(content: string): { spec: Spec | null; text: string } {
  if (!content) return { spec: null, text: '' };

  let spec: Spec | null = null;
  let text = content;

  // Try to find JSON spec in code fences
  const fenceMatch = content.match(/```(?:json)?\s*([\s\S]*?)```/);
  
  if (fenceMatch) {
    const jsonStr = fenceMatch[1].trim();
    const fenceStart = content.indexOf('```');
    const fenceEnd = content.lastIndexOf('```') + 3;
    
    try {
      const parsed = JSON.parse(jsonStr);
      if (parsed && typeof parsed === 'object' && 'root' in parsed && 'elements' in parsed) {
        spec = parsed as Spec;
        // Remove the code fence from text
        text = (content.slice(0, fenceStart) + content.slice(fenceEnd)).trim();
      }
    } catch {
      // JSON parse failed, keep as text
    }
  } else {
    // Try to find raw JSON object with spec structure
    const jsonMatch = content.match(/(\{[\s\S]*"root"[\s\S]*"elements"[\s\S]*\})/);
    if (jsonMatch) {
      try {
        const parsed = JSON.parse(jsonMatch[1]);
        if (parsed && typeof parsed === 'object' && 'root' in parsed && 'elements' in parsed) {
          spec = parsed as Spec;
          const jsonStart = content.indexOf(jsonMatch[1]);
          text = content.slice(0, jsonStart).trim();
        }
      } catch {
        // JSON parse failed, keep as text
      }
    }
  }

  return { spec, text };
}

/**
 * Component to render a single message with json-render spec
 * Uses ai-elements MessageResponse for rich markdown rendering
 */
function PluginMessageContent({ message, isStreaming }: { message: ChatMessage; isStreaming?: boolean }) {
  const { spec, text } = message;
  const hasSpec = spec !== null && Object.keys(spec.elements || {}).length > 0;
  const [showRaw, setShowRaw] = useState(false);
  const [copied, setCopied] = useState(false);

  const handleAction = useCallback((actionName: string, params?: Record<string, unknown>) => {
    console.log('[PluginChatView] Action:', actionName, params);
  }, []);

  const handleCopy = useCallback(async () => {
    const contentToCopy = hasSpec && spec 
      ? JSON.stringify(spec, null, 2) 
      : text || message.content;
    await navigator.clipboard.writeText(contentToCopy);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [hasSpec, spec, text, message.content]);

  return (
    <>
      {/* Text content with rich markdown rendering */}
      {text && (
        <MessageResponse isAnimating={isStreaming}>
          {text}
        </MessageResponse>
      )}

      {/* Rendered spec */}
      {hasSpec && spec && (
        <div className="space-y-2 mt-3">
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
            <pre className="p-3 bg-muted rounded-lg text-xs overflow-auto max-h-64 font-mono">
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

      {/* Show raw content if no spec extracted and no text */}
      {!hasSpec && !text && message.content && (
        <MessageResponse isAnimating={isStreaming}>
          {message.content}
        </MessageResponse>
      )}

      {/* Message toolbar with actions (only for complete messages) */}
      {!isStreaming && (text || message.content) && (
        <MessageToolbar>
          <MessageActions>
            <MessageAction tooltip="Copy" onClick={handleCopy}>
              {copied ? <Check className="w-4 h-4" /> : <Copy className="w-4 h-4" />}
            </MessageAction>
            <MessageAction tooltip="Good response">
              <ThumbsUp className="w-4 h-4" />
            </MessageAction>
            <MessageAction tooltip="Bad response">
              <ThumbsDown className="w-4 h-4" />
            </MessageAction>
          </MessageActions>
        </MessageToolbar>
      )}
    </>
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

  const handleSubmitMessage = async (messageText: string) => {
    if (!messageText.trim() || isLoading) return;

    const userMessage: ChatMessage = {
      id: `user-${Date.now()}`,
      role: 'user',
      content: messageText,
      parts: [{ type: 'text', text: messageText }],
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
      messageText,
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

        {/* Messages using ai-elements */}
        <ScrollArea ref={scrollRef} className="flex-1">
          {messages.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full p-8 text-muted-foreground">
              <Sparkles className="w-12 h-12 mb-4" />
              <p className="text-center mb-2">
                Select a plugin from the sidebar to generate a UI spec.
              </p>
              <p className="text-center text-sm">
                Uses <code className="text-xs bg-muted px-1 rounded">useJsonRenderMessage</code> for spec extraction.
              </p>
            </div>
          ) : (
            <div className="p-4 space-y-4">
              {messages.map((message, index) => {
                const isLastAssistant = message.role === 'assistant' && 
                  index === messages.length - 1 && isLoading;
                
                return (
                  <Message key={message.id} from={message.role}>
                    <AIMessageContent>
                      {message.role === 'user' ? (
                        <p className="text-sm whitespace-pre-wrap">{message.content}</p>
                      ) : message.content ? (
                        <PluginMessageContent 
                          message={message} 
                          isStreaming={isLastAssistant}
                        />
                      ) : isLoading ? (
                        <div className="flex items-center gap-2 text-muted-foreground">
                          <Loader2 className="w-4 h-4 animate-spin" />
                          <span className="text-sm">Generating...</span>
                        </div>
                      ) : null}
                    </AIMessageContent>
                  </Message>
                );
              })}
            </div>
          )}
        </ScrollArea>

        {/* Error display */}
        {error && (
          <div className="p-3 bg-destructive/10 text-destructive text-sm">
            Error: {error}
          </div>
        )}

        {/* Input using ai-elements PromptInput */}
        <div className="p-3 border-t">
          <PromptInput
            onSubmit={({ text }) => {
              if (!text.trim() || isLoading) return;
              handleSubmitMessage(text);
            }}
          >
            <PromptInputTextarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder={`Describe the UI you want for ${selectedPlugin}...`}
              disabled={isLoading}
            />
            <PromptInputFooter>
              <PromptInputTools>
                <Badge variant="outline" className="text-xs">
                  json-render
                </Badge>
              </PromptInputTools>
              <div className="flex items-center gap-1">
                {isLoading ? (
                  <Button type="button" variant="outline" size="sm" onClick={handleStop}>
                    Stop
                  </Button>
                ) : (
                  <PromptInputSubmit disabled={!input.trim()} />
                )}
              </div>
            </PromptInputFooter>
          </PromptInput>
        </div>
      </div>
    </div>
  );
}

export default PluginChatView;
