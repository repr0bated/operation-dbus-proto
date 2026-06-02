const DEFAULT_TARGET = "http://100.90.37.254";

function resolveTarget() {
  return process.env.VITE_API_PROXY || DEFAULT_TARGET;
}

function sendJson(res, status, payload) {
  res.statusCode = status;
  res.setHeader("content-type", "application/json");
  res.end(JSON.stringify(payload));
}

function writeSse(res, event, data) {
  res.write(`event: ${event}\n`);
  res.write(`data: ${JSON.stringify(data)}\n\n`);
}

function parseSseChunk(chunk, carry) {
  const text = carry + chunk;
  const parts = text.split(/\n\n/);
  return {
    frames: parts.slice(0, -1),
    carry: parts.at(-1) ?? "",
  };
}

function normalizeFrame(frame, source) {
  const lines = frame.split(/\r?\n/);
  let event = "message";
  const dataLines = [];

  for (const line of lines) {
    if (line.startsWith("event:")) {
      event = line.slice(6).trim() || event;
    } else if (line.startsWith("data:")) {
      dataLines.push(line.slice(5).trimStart());
    }
  }

  const raw = dataLines.join("\n");
  let payload = raw;
  try {
    payload = JSON.parse(raw);
  } catch {
    // Upstream sometimes emits plain text log lines. Keep the original text.
  }

  return {
    source,
    event,
    payload,
    received_at: new Date().toISOString(),
  };
}

async function bridgeSse(req, res, upstreamPath) {
  const target = resolveTarget();
  const controller = new AbortController();
  const upstreamUrl = new URL(upstreamPath, target);

  req.on("close", () => controller.abort());
  res.statusCode = 200;
  res.setHeader("content-type", "text/event-stream");
  res.setHeader("cache-control", "no-cache");
  res.setHeader("connection", "keep-alive");
  res.flushHeaders?.();

  writeSse(res, "shim.ready", {
    ok: true,
    target,
    upstream: upstreamUrl.toString(),
  });

  try {
    const upstream = await fetch(upstreamUrl, {
      headers: { accept: "text/event-stream" },
      signal: controller.signal,
    });

    if (!upstream.ok || !upstream.body) {
      writeSse(res, "shim.error", {
        ok: false,
        status: upstream.status,
        status_text: upstream.statusText,
      });
      res.end();
      return;
    }

    const reader = upstream.body.getReader();
    const decoder = new TextDecoder();
    let carry = "";

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      const parsed = parseSseChunk(decoder.decode(value, { stream: true }), carry);
      carry = parsed.carry;

      for (const frame of parsed.frames) {
        const normalized = normalizeFrame(frame, upstreamPath);
        writeSse(res, normalized.event, normalized);
      }
    }

    if (carry.trim()) {
      const normalized = normalizeFrame(carry, upstreamPath);
      writeSse(res, normalized.event, normalized);
    }
  } catch (error) {
    if (controller.signal.aborted) return;
    writeSse(res, "shim.error", {
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  } finally {
    res.end();
  }
}

export function zeroclawDevShim() {
  return {
    name: "zeroclaw-dev-shim",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use(async (req, res, next) => {
        const url = req.url || "";

        if (url === "/api/dev-shim/status") {
          const target = resolveTarget();
          sendJson(res, 200, {
            ok: true,
            name: "zeroclaw-dev-shim",
            target,
            endpoints: {
              events: "/api/dev-shim/events",
              logs: "/api/dev-shim/logs",
            },
          });
          return;
        }

        if (url === "/api/dev-shim/events") {
          await bridgeSse(req, res, "/api/events");
          return;
        }

        if (url === "/api/dev-shim/logs") {
          await bridgeSse(req, res, "/api/logs/stream");
          return;
        }

        next();
      });
    },
  };
}
