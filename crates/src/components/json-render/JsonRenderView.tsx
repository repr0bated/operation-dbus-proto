/**
 * JsonRenderView - Main renderer component for json-render specs
 * Renders streamed UI specs using shadcn/ui components
 */
import React from 'react';
import { Renderer, JSONUIProvider, type Spec } from '@json-render/react';
import { registry } from './registry';
import { Loader2 } from 'lucide-react';

interface JsonRenderViewProps {
  /** The UI spec to render */
  spec: Spec | null;
  /** Whether spec is currently streaming */
  loading?: boolean;
  /** Initial state for bindings */
  state?: Record<string, unknown>;
  /** Callback when state changes */
  onStateChange?: (changes: Array<{ path: string; value: unknown }>) => void;
  /** Action handler for button clicks, etc. */
  onAction?: (actionName: string, params?: Record<string, unknown>) => void;
  /** Custom class name */
  className?: string;
}

/**
 * Fallback component for unknown element types
 */
const FallbackComponent = ({ element }: { element: { type: string; props?: Record<string, unknown> } }) => (
  <div className="p-2 border border-dashed border-yellow-500 rounded bg-yellow-50 text-yellow-800 text-sm">
    Unknown component: <code>{element.type}</code>
  </div>
);

/**
 * Loading indicator shown while streaming
 */
const StreamingIndicator = () => (
  <div className="flex items-center gap-2 text-sm text-muted-foreground">
    <Loader2 className="h-4 w-4 animate-spin" />
    <span>Generating UI...</span>
  </div>
);

/**
 * Empty state when no spec is provided
 */
const EmptyState = () => (
  <div className="flex items-center justify-center p-8 text-muted-foreground">
    No UI specification to render
  </div>
);

export function JsonRenderView({
  spec,
  loading = false,
  state = {},
  onStateChange,
  onAction,
  className,
}: JsonRenderViewProps) {
  // Convert action handler to handlers map
  const handlers = React.useMemo(() => {
    if (!onAction) return {};
    return {
      // Generic action dispatcher
      dispatch: async (params: Record<string, unknown>) => {
        onAction('dispatch', params);
      },
      'dbus.call': async (params: Record<string, unknown>) => {
        onAction('dbus.call', params);
      },
      'grpc.call': async (params: Record<string, unknown>) => {
        onAction('grpc.call', params);
      },
      navigate: async (params: Record<string, unknown>) => {
        onAction('navigate', params);
      },
    };
  }, [onAction]);

  // Show loading state
  if (!spec && loading) {
    return (
      <div className={className}>
        <StreamingIndicator />
      </div>
    );
  }

  // Show empty state
  if (!spec) {
    return (
      <div className={className}>
        <EmptyState />
      </div>
    );
  }

  return (
    <div className={className}>
      {loading && <StreamingIndicator />}
      <JSONUIProvider
        registry={registry}
        initialState={state}
        handlers={handlers}
        onStateChange={onStateChange}
      >
        <Renderer
          spec={spec}
          registry={registry}
          loading={loading}
          fallback={FallbackComponent}
        />
      </JSONUIProvider>
    </div>
  );
}

export default JsonRenderView;
