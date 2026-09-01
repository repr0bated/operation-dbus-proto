/**
 * OP-DBUS singleton Streamable-HTTP transport overlay for notebooklm-mcp 2.0.0.
 *
 * The upstream package advertises multiple HTTP sessions while connecting each
 * session to one shared MCP Server instance. The MCP SDK permits only one
 * transport per Server, so a second initialize otherwise raises HTTP 500 and a
 * closed session cannot be replaced reliably.
 *
 * OP-DBUS has one owner for this internal provider: the bridge's persistent
 * SupervisedMcpProvider. Keep one transport, reject concurrent initializers,
 * and terminate cleanly when that transport closes. Runit then starts a fresh
 * provider process with a fresh MCP Server for the bridge to reconnect to.
 */
import { createServer } from "node:http";
import { randomUUID } from "node:crypto";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import { isInitializeRequest } from "@modelcontextprotocol/sdk/types.js";
import { log } from "../utils/logger.js";

const SESSION_HEADER = "mcp-session-id";
// Coordination guard only, not a credential: the provider is confined to
// loopback and the bridge is the sole configured consumer.  The authenticated
// security boundary remains the bridge's external OIB1-gated :8090 endpoint.
const OWNER_CLIENT_NAME = process.env.OPDBUS_MCP_OWNER_NAME ?? "op-dbus-mcp-aggregator";

export async function startHttpTransport(opts) {
    const transports = new Map();
    const singleton = {
        activeTransport: undefined,
        restartRequested: false,
    };
    const server = createServer((req, res) => {
        void handleRequest(req, res, transports, singleton, opts).catch((err) => {
            log.error(`❌ [HTTP] Unhandled request error: ${err}`);
            if (!res.headersSent) {
                res.writeHead(500, { "Content-Type": "application/json" });
                res.end(JSON.stringify({ error: "internal server error" }));
            }
        });
    });
    await new Promise((resolve, reject) => {
        server.once("error", reject);
        server.listen(opts.port, opts.host ?? "127.0.0.1", () => {
            server.off("error", reject);
            log.success(`🌐 HTTP transport listening on http://${opts.host ?? "127.0.0.1"}:${opts.port}/mcp`);
            resolve();
        });
    });
    return {
        server,
        close: async () => {
            const activeTransport = singleton.activeTransport;
            singleton.activeTransport = undefined;
            if (activeTransport) {
                try {
                    await activeTransport.close();
                }
                catch {
                    /* ignore — best-effort shutdown */
                }
            }
            transports.clear();
            await new Promise((resolve, reject) => server.close((err) => (err ? reject(err) : resolve())));
        },
    };
}

/**
 * Use the MCP Server high-level class. OP-DBUS deliberately binds exactly one
 * bridge-owned transport during each runit-supervised provider process.
 */
export async function bindMcpServer(mcpServer, transport) {
    await mcpServer.connect(transport);
}

async function handleRequest(req, res, transports, singleton, opts) {
    const url = new URL(req.url ?? "/", `http://${req.headers.host ?? "localhost"}`);
    if (url.pathname === "/healthz" && req.method === "GET") {
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ status: "ok", protocol: "mcp-streamable-http" }));
        return;
    }
    if (url.pathname !== "/mcp") {
        res.writeHead(404, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "not found", expected: "/mcp" }));
        return;
    }

    const sessionId = headerString(req.headers[SESSION_HEADER]);
    if (req.method === "GET" || req.method === "DELETE") {
        const transport = sessionId ? transports.get(sessionId) : undefined;
        if (!transport) {
            res.writeHead(404, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ error: "unknown session" }));
            return;
        }
        await transport.handleRequest(req, res);
        return;
    }
    if (req.method !== "POST") {
        res.writeHead(405, { "Content-Type": "application/json", Allow: "POST, GET, DELETE" });
        res.end(JSON.stringify({ error: "method not allowed" }));
        return;
    }

    const body = await readJsonBody(req);
    let transport = sessionId ? transports.get(sessionId) : undefined;
    if (!transport && isInitializeRequest(body)) {
        if (body?.params?.clientInfo?.name !== OWNER_CLIENT_NAME) {
            res.writeHead(409, { "Content-Type": "application/json" });
            res.end(JSON.stringify({
                jsonrpc: "2.0",
                id: body?.id ?? null,
                error: {
                    code: -32002,
                    message: "NotebookLM provider is reserved for the OP-DBUS bridge",
                },
            }));
            return;
        }
        if (singleton.activeTransport) {
            // There is only one runit-supervised bridge consumer.  Seeing a
            // fresh initialize from that same owner means its prior process
            // died or lost the initialize response without sending DELETE.
            // The SDK Server cannot be rebound safely, so terminate this
            // provider and let runit create a clean Server for the retry.
            res.writeHead(503, {
                "Content-Type": "application/json",
                "Retry-After": "1",
            });
            res.end(JSON.stringify({
                jsonrpc: "2.0",
                id: body?.id ?? null,
                error: {
                    code: -32003,
                    message: "Replacing stale OP-DBUS bridge session; retry after provider restart",
                },
            }));
            requestRunitRestart(singleton);
            return;
        }

        transport = new StreamableHTTPServerTransport({
            sessionIdGenerator: () => randomUUID(),
            onsessioninitialized: (sid) => {
                transports.set(sid, transport);
            },
        });
        singleton.activeTransport = transport;
        transport.onclose = () => {
            if (transport.sessionId)
                transports.delete(transport.sessionId);
            if (singleton.activeTransport === transport)
                singleton.activeTransport = undefined;
            requestRunitRestart(singleton);
        };
        await opts.connect(transport);
    }
    if (!transport) {
        res.writeHead(400, { "Content-Type": "application/json" });
        res.end(JSON.stringify({
            error: "no transport for request — pass an `Mcp-Session-Id` header or send `initialize`",
        }));
        return;
    }
    await transport.handleRequest(req, res, body);
}

function requestRunitRestart(singleton) {
    if (singleton.restartRequested)
        return;
    singleton.restartRequested = true;
    setImmediate(() => {
        try {
            process.kill(process.pid, "SIGTERM");
        }
        catch (error) {
            log.error(`❌ [HTTP] Could not request clean provider restart: ${error}`);
            process.exitCode = 1;
        }
    });
}

async function readJsonBody(req) {
    const chunks = [];
    for await (const chunk of req) {
        chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    }
    if (chunks.length === 0)
        return undefined;
    const raw = Buffer.concat(chunks).toString("utf8");
    if (!raw.trim())
        return undefined;
    try {
        return JSON.parse(raw);
    }
    catch {
        throw new Error("Invalid JSON request body");
    }
}

function headerString(value) {
    if (Array.isArray(value))
        return value[0];
    return value;
}
