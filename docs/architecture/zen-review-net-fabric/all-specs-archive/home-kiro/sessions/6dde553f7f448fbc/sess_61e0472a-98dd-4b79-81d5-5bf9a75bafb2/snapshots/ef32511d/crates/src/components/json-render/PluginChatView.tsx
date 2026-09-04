/**
 * PluginChatView - Chat interface with json-render UI extraction
 *
 * Uses AI SDK's useChat + json-render's useJsonRenderMessage to
 * extract and render UI specs from streamed assistant messages.
 */
import React, { useState, FormEvent, useRef, useEffect } from 'react';
import { useChat } from 'ai/react';
import { useJsonRenderMessage } from '@json-render/react';
import { JsonRenderView } from './JsonRenderView';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import { Send, Loader2, RefreshCw, Bot, User } from 'lucide-react';

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

interface MessageWithSpec {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  spec: ReturnType<typeof useJsonRenderMessage>['spec'];
  isStreaming: boolean;
}

interface PluginChatViewProps {
  /** API endpoint for chat */
  apiEndpoint?: string;
  /** Initial plugin to focus on */
  initialPlugin?: string;
  /** Custom class name */
  className?: string;
}

/**
 * Individual message component with json-render extraction
 */
function ChatMessage({
  message,
  onAction,
}: {
  message: MessageWithSpec;
  onAction?: (action: string, params?: Record<string, unknown>) => void;
}) {
  const isUser = message.role === 'user';

  return (
    <div className={cn('flex gap-3 p-4', isUser ? 'bg-muted/50' : 'bg-background')}>
      <div
        className={cn(
          'flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center',
          isUser ? 'bg-primary text-primary-foreground' : 'bg-secondary'
        )}
      >
        {isUser ? <User className="w-4 h-4" /> : <Bot className="w-4 h-4" />}
      </div>
      <div className="flex-1 space-y-2">
        {/* Show text content if no spec, or if it's the user */}
        {(isUser || !message.spec) && (
          <p className="text-sm whitespace-pre-wrap">{message.content}</p>
        )}

        {/* Render the extracted spec */}
        {message.spec && !isUser && (
          <JsonRenderView
            spec={message.spec}
            loading={message.isStreaming}
            onAction={onAction}
          />
        )}

        {/* Show streaming indicator */}
        {message.isStreaming && !message.spec && (
          <div className="flex items-center gap-2 text-muted-foreground">
            <Loader2 className="w-4 h-4 animate-spin" />
            <span className="text-sm">Generating...</span>
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * Hook to extract json-render spec from a message
 */
function useMessageSpec(content: string, isStreaming: boolean) {
  const { spec, error } = useJsonRenderMessage(content);

  // Only return spec if valid
  if (error || !spec) {
    return null;
  }

  return spec;
}

/**
 * Wrapper component for message with spec extraction
 */
function MessageWrapper({
  message,
  isStreaming,
  onAction,
}: {
  message: { id: string; role: 'user' | 'assistant' | 'system'; content: string };
  isStreaming: boolean;
  onAction?: (action: string, params?: Record<string, unknown>) => void;
}) {
  const spec = useMessageSpec(message.content, isStreaming);

  return (
    <ChatMessage
      message={{
        ...message,
        spec,
        isStreaming,
      }}
      onAction={onAction}
    />
  );
}

export function PluginChatView({
  apiEndpoint = '/api/chat',
  initialPlugin = 'tchedRouter',
  className,
}: PluginChatViewProps) {
  const [input, setInput] = useState('');
  const [selectedPlugin, setSelectedPlugin] = useState(initialPlugin);
  const scrollRef = useRef<HTMLDivElement>(null);

  const { messages, sendMessage, isLoading, error, stop, reload } = useChat({
    api: apiEndpoint,
  });

  // Auto-scroll on new messages
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (!input.trim() || isLoading) return;

    sendMessage(input);
    setInput('');
  };

  const handlePluginSelect = (plugin: string) => {
    setSelectedPlugin(plugin);
    // Send a prompt to render UI for this plugin
    const prompt = `Generate a json-render UI spec for the ${plugin} plugin. Include status, configuration options, and any child objects it manages.`;
    sendMessage(prompt);
  };

  const handleAction = (actionName: string, params?: Record<string, unknown>) => {
    console.log('[PluginChatView] Action:', actionName, params);
    // Could dispatch to D-Bus/gRPC here
  };

  return (
    <div className={cn('flex h-full', className)}>
      {/* Plugin sidebar */}
      <div className="w-48 border-r flex flex-col">
        <div className="p-3 border-b">
          <h3 className="font-semibold text-sm">Plugins</h3>
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
            <h2 className="font-semibold">Plugin UI Chat</h2>
            <Badge variant="outline" className="font-mono">
              {selectedPlugin}
            </Badge>
          </div>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => reload()}
            disabled={isLoading || messages.length === 0}
          >
            <RefreshCw className="w-4 h-4" />
          </Button>
        </div>

        {/* Messages */}
        <ScrollArea ref={scrollRef} className="flex-1">
          {messages.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full p-8 text-muted-foreground">
              <Bot className="w-12 h-12 mb-4" />
              <p className="text-center">
                Select a plugin from the sidebar or type a message to generate a UI.
              </p>
            </div>
          ) : (
            <div className="divide-y">
              {messages.map((message, index) => (
                <MessageWrapper
                  key={message.id}
                  message={message}
                  isStreaming={isLoading && index === messages.length - 1}
                  onAction={handleAction}
                />
              ))}
            </div>
          )}
        </ScrollArea>

        {/* Error display */}
        {error && (
          <div className="p-3 bg-destructive/10 text-destructive text-sm">
            Error: {error.message}
          </div>
        )}

        {/* Input */}
        <form onSubmit={handleSubmit} className="p-3 border-t flex gap-2">
          <Input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder={`Ask about ${selectedPlugin} or describe a UI...`}
            disabled={isLoading}
            className="flex-1"
          />
          {isLoading ? (
            <Button type="button" variant="outline" onClick={stop}>
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
