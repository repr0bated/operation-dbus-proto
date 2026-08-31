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

function identityHeaders(capability?: string): Record<string, string> {
  const headers: Record<string, string> = { "x-grpc-web": "1" };
  if (capability) headers["x-opdbus-capability"] = capability;
  // Identity is resolved from the sled endpoint on first load
  const stored = sessionStorage.getItem("opdbus-identity");
  if (stored) {
    try {
      const { genesis, hashed_footprint, trace_id } = JSON.parse(stored);
      const footprint = genesis ?? hashed_footprint;
      if (footprint) {
        headers["x-ghostbridge-genesis"] = footprint;
        headers["x-ghostbridge-footprint"] = footprint;
      }
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
  options: { capability?: string } = {},
): StreamHandle<Uint8Array> {
  const controller = new AbortController();
  const url = `${getBaseUrl()}/${service}/${method}`;
  const frame = frameMessage(requestBytes);

  const responsePromise = fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": CONTENT_TYPE,
      Accept: CONTENT_TYPE,
      ...identityHeaders(options.capability),
    },
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

function encodeVarint(n: number): Uint8Array {
  const bytes: number[] = [];
  let x = n >>> 0;
  while (x > 0x7f) {
    bytes.push((x & 0x7f) | 0x80);
    x >>>= 7;
  }
  bytes.push(x);
  return Uint8Array.from(bytes);
}

/** Length-delimited protobuf string field. */
export function encodeProtoString(field: number, value: string): Uint8Array {
  if (!value) return new Uint8Array(0);
  const payload = new TextEncoder().encode(value);
  const key = encodeVarint((field << 3) | 2);
  const len = encodeVarint(payload.length);
  const out = new Uint8Array(key.length + len.length + payload.length);
  out.set(key, 0);
  out.set(len, key.length);
  out.set(payload, key.length + len.length);
  return out;
}

export async function grpcUnary(
  service: string,
  method: string,
  requestBytes: Uint8Array = new Uint8Array(0),
  options: { capability?: string } = {},
): Promise<Uint8Array> {
  await resolveIdentity();
  const url = `${getBaseUrl()}/${service}/${method}`;
  const frame = frameMessage(requestBytes);
  const res = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": CONTENT_TYPE,
      Accept: CONTENT_TYPE,
      ...identityHeaders(options.capability),
    },
    body: frame as unknown as BodyInit,
  });
  if (!res.ok) {
    throw new Error(`gRPC unary failed: HTTP ${res.status}`);
  }
  const buf = new Uint8Array(await res.arrayBuffer());
  let offset = 0;
  let data: Uint8Array | null = null;
  const trailers = new Map<string, string>();
  while (offset + 5 <= buf.length) {
    const flags = buf[offset]!;
    const len = new DataView(buf.buffer, buf.byteOffset + offset + 1, 4).getUint32(0, false);
    offset += 5;
    if (offset + len > buf.length) break;
    const payload = buf.slice(offset, offset + len);
    offset += len;
    if (flags & 0x80) {
      const text = new TextDecoder().decode(payload);
      for (const line of text.split(/\r?\n/)) {
        const idx = line.indexOf(":");
        if (idx > 0) trailers.set(line.slice(0, idx).trim().toLowerCase(), line.slice(idx + 1).trim());
      }
    } else {
      data = payload;
    }
  }
  const st = trailers.get("grpc-status") ?? res.headers.get("grpc-status");
  if (st && st !== "0") {
    throw new Error(`gRPC error ${st}: ${trailers.get("grpc-message") ?? ""}`);
  }
  return data ?? new Uint8Array(0);
}

/** Resolve identity from the sled endpoint and store in sessionStorage. */
export async function resolveIdentity(): Promise<boolean> {
  try {
    const res = await fetch("/api/identity/sled");
    if (!res.ok) return false;
    const body = await res.json();
    const genesis = body.genesis ?? body.hashed_footprint;
    if (!body.is_valid || !genesis || !body.trace_id) return false;
    sessionStorage.setItem("opdbus-identity", JSON.stringify({
      genesis,
      hashed_footprint: genesis,
      trace_id: body.trace_id,
    }));
    return true;
  } catch {
    return false;
  }
}
