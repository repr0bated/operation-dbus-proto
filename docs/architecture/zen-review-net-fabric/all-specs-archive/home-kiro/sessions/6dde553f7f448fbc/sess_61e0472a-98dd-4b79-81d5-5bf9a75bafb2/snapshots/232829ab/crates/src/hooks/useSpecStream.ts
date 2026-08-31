/**
 * useSpecStream - Hook for streaming json-render specs
 * Wraps @json-render/react's useUIStream with project-specific defaults
 */
import { useUIStream, type Spec } from '@json-render/react';
import { useCallback, useState } from 'react';

interface UseSpecStreamOptions {
  /** API endpoint for spec generation */
  endpoint: string;
  /** Callback when generation completes */
  onComplete?: (spec: Spec) => void;
  /** Callback on error */
  onError?: (error: Error) => void;
}

interface UseSpecStreamReturn {
  /** Current compiled spec */
  spec: Spec | null;
  /** Whether currently streaming */
  isStreaming: boolean;
  /** Error if any */
  error: Error | null;
  /** Raw JSONL lines received */
  rawLines: string[];
  /** Send a prompt to generate UI */
  generate: (prompt: string, context?: Record<string, unknown>) => Promise<void>;
  /** Clear current spec and reset */
  clear: () => void;
  /** Abort current generation */
  abort: () => void;
}

/**
 * Hook for streaming json-render UI specifications.
 * 
 * @example
 * ```tsx
 * const { spec, isStreaming, generate } = useSpecStream({
 *   endpoint: '/api/generate-ui',
 * });
 * 
 * // Generate UI from prompt
 * await generate('Show network status dashboard');
 * 
 * // Render the result
 * <JsonRenderView spec={spec} loading={isStreaming} />
 * ```
 */
export function useSpecStream({
  endpoint,
  onComplete,
  onError,
}: UseSpecStreamOptions): UseSpecStreamReturn {
  const [abortController, setAbortController] = useState<AbortController | null>(null);

  const {
    spec,
    isStreaming,
    error,
    rawLines,
    send,
    clear: clearSpec,
  } = useUIStream({
    api: endpoint,
    onComplete,
    onError,
  });

  const generate = useCallback(async (prompt: string, context?: Record<string, unknown>) => {
    // Create new abort controller for this request
    const controller = new AbortController();
    setAbortController(controller);
    
    try {
      await send(prompt, context);
    } finally {
      setAbortController(null);
    }
  }, [send]);

  const abort = useCallback(() => {
    if (abortController) {
      abortController.abort();
      setAbortController(null);
    }
  }, [abortController]);

  const clear = useCallback(() => {
    abort();
    clearSpec();
  }, [abort, clearSpec]);

  return {
    spec,
    isStreaming,
    error,
    rawLines,
    generate,
    clear,
    abort,
  };
}

export default useSpecStream;
