/**
 * Binary gRPC-Web transport. Sends application/grpc-web+proto frames,
 * parses data frames (0x00) and trailers (0x80) from the response.
 */

const CONTENT_TYPE = "application/grpc-web+proto";

function getBaseUrl(): string {
  if (typeof window !== "undefined" && window.location?.origin) {
    return window.location.origin;
  }
  return "";
}

function identityHeaders(): Record<string, string> {
  const headers: Record<string, string> = { "x-grpc-web": "1" };
  // Identity is resolved from the sled endpoint on first load
  const stored = sessionStorage.getItem("opdbus-identity");
  if (stored) {
    try {
      const { hashed_footprint, trace_id } = JSON.parse(stored);
      if (hashed_footprint) headers["x-ghostbridge-footprint"] = hashed_footprint;
      if (trace_id) headers["x-ghostbridge-trace-id"] = trace_id;
    } catch { /* ignore */ }
  }
  return headers;
}

function frameMessage(payload: Uint8Array): Uint8Array {
  const frame = new Uint8Array(5 + payload.length);
  frame[0] = 0x00;
  new DataView(frame.buffer).setUint32(1, payload.length, false);
  frame.set(payload, 5);
  return frame;
}

export interface StreamHandle<T> {
  stream: ReadableStream<T>;
  abort: () => void;
}

/**
 * Open a server-streaming gRPC-Web call. Returns a ReadableStream of raw
 * Uint8Array data frame payloads (caller decodes protobuf).
 */
export function grpcServerStream(
  service: string,
  method: string,
  requestBytes: Uint8Array = new Uint8Array(0),
): StreamHandle<Uint8Array> {
  const controller = new AbortController();
  const url = `${getBaseUrl()}/${service}/${method}`;
  const frame = frameMessage(requestBytes);

  const responsePromise = fetch(url, {
    method: "POST",
    headers: { "Content-Type": CONTENT_TYPE, Accept: CONTENT_TYPE, ...identityHeaders() },
    body: frame as unknown as BodyInit,
    signal: controller.signal,
  });

  const stream = new ReadableStream<Uint8Array>({
    async start(ctrl) {
      try {
        const res = await responsePromise;
        if (!res.ok || !res.body) {
          ctrl.error(new Error(`gRPC stream failed: HTTP ${res.status}`));
          return;
        }
        const reader = res.body.getReader();
        let pending = new Uint8Array(0);

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          const merged = new Uint8Array(pending.length + value.length);
          merged.set(pending);
          merged.set(value, pending.length);
          pending = merged;

          while (pending.length >= 5) {
            const flags = pending[0]!;
            const len = new DataView(pending.buffer, pending.byteOffset + 1, 4).getUint32(0, false);
            if (pending.length < 5 + len) break;
            const payload = pending.slice(5, 5 + len);

            if (flags & 0x80) {
              // Trailers — check for error
              const text = new TextDecoder().decode(payload);
              const trailers = new Map<string, string>();
              for (const line of text.split(/\r?\n/)) {
                const idx = line.indexOf(":");
                if (idx > 0) trailers.set(line.slice(0, idx).trim().toLowerCase(), line.slice(idx + 1).trim());
              }
              const st = trailers.get("grpc-status");
              if (st && st !== "0") {
                ctrl.error(new Error(`gRPC error ${st}: ${trailers.get("grpc-message") ?? ""}`));
                return;
              }
            } else {
              ctrl.enqueue(payload);
            }
            pending = pending.slice(5 + len);
          }
        }
        ctrl.close();
      } catch (err) {
        if ((err as Error).name !== "AbortError") ctrl.error(err);
      }
    },
  });

  return { stream, abort: () => controller.abort() };
}

/**
 * Resolve identity from the sled endpoint and store in sessionStorage.
 */
export async function resolveIdentity(): Promise<boolean> {
  try {
    const res = await fetch("/api/identity/sled");
    if (!res.ok) return false;
    const body = await res.json();
    if (!body.is_valid || !body.hashed_footprint || !body.trace_id) return false;
    sessionStorage.setItem("opdbus-identity", JSON.stringify({
      hashed_footprint: body.hashed_footprint,
      trace_id: body.trace_id,
    }));
    return true;
  } catch {
    return false;
  }
}
