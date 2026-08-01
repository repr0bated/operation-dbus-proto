# Operation-DBUS API Implementation Checklist

## Current Status
- ✅ Dashboard UI: Complete (all 15 pages built)
- ⚠️ Backend API: Partial (only signup/verify endpoints exist)
- 🔧 Next: Implement missing API endpoints in op-web

---

## Priority 1: Core Dashboard Functionality

### Users Management (`/api/users/*`)
- [ ] `GET /api/users` - List all users with VPN info
- [ ] `GET /api/users/:id` - Get user details
- [ ] `GET /api/users/:id/activity` - User activity history
- [ ] `DELETE /api/users/:id` - Delete user
- [ ] `PUT /api/users/:id/status` - Update user status
- [ ] `POST /api/users/:id/revoke` - Revoke VPN access

### VPN Status (`/api/vpn/*`)
- [ ] `GET /api/vpn/status` - Overall WireGuard status
- [ ] `GET /api/vpn/connections` - Active connections list
- [ ] `GET /api/vpn/peers` - WireGuard peers
- [ ] `GET /api/vpn/config/:userId` - User VPN config
- [ ] `GET /api/vpn/config` - Server config (public key, endpoint)

### Mail Server (`/api/mail/*`)
- [ ] `GET /api/mail/status` - Maddy server status
- [ ] `GET /api/mail/queue` - Mail queue
- [ ] `GET /api/mail/stats` - Today's mail stats
- [ ] `GET /api/mail/dns-status` - DNS records health check
- [ ] `GET /api/mail/recent` - Recent emails
- [ ] `POST /api/mail/resend/:id` - Resend failed email

### Dashboard Overview (`/api/analytics/*`)
- [ ] `GET /api/analytics/overview` - Main dashboard metrics
- [ ] `GET /api/analytics/connections` - VPN connection timeline (24h)
- [ ] `GET /api/health` - System health (CPU, Memory, Disk)
- [ ] `GET /api/health/metrics` - Resource metrics timeline
- [ ] `GET /api/activity/recent` - Recent activity feed

---

## Priority 2: MCP & Orchestration

### MCP Services (`/api/mcp/*`)
- [ ] `GET /api/mcp/services` - List MCP servers
- [ ] `GET /api/mcp/services/:id` - Server details
- [ ] `GET /api/mcp/services/:id/tools` - Server tools
- [ ] `GET /api/mcp/services/:id/logs` - Server logs
- [ ] `POST /api/mcp/services/:id/start` - Start MCP server
- [ ] `POST /api/mcp/services/:id/stop` - Stop MCP server
- [ ] `PUT /api/mcp/services/:id/config` - Update server config
- [ ] `GET /api/mcp/tools` - All available MCP tools
- [ ] `POST /api/mcp/tools/execute` - Execute MCP tool

### Tools Management (`/api/tools/*`)
- [ ] `GET /api/tools` - List all tools
- [ ] `GET /api/tools/:id` - Tool details
- [ ] `GET /api/tools/:id/history` - Tool execution history
- [ ] `POST /api/tools/execute` - Execute tool
- [ ] `POST /api/tools` - Create custom tool
- [ ] `PUT /api/tools/:id` - Update tool
- [ ] `DELETE /api/tools/:id` - Delete tool

### Agents (`/api/agents/*`)
- [ ] `GET /api/agents` - List agents
- [ ] `GET /api/agents/:id` - Agent details
- [ ] `GET /api/agents/:id/history` - Agent call history
- [ ] `GET /api/agents/:id/metrics` - Agent performance metrics
- [ ] `POST /api/agents` - Create agent
- [ ] `PUT /api/agents/:id/config` - Update agent config
- [ ] `DELETE /api/agents/:id` - Delete agent
- [ ] `POST /api/agents/:id/start` - Start agent
- [ ] `POST /api/agents/:id/stop` - Stop agent

### Workflows (`/api/workflows/*`)
- [ ] `GET /api/workflows` - List workflows
- [ ] `GET /api/workflows/:id` - Workflow details
- [ ] `GET /api/workflows/:id/runs` - Execution history
- [ ] `POST /api/workflows` - Create workflow
- [ ] `PUT /api/workflows/:id` - Update workflow
- [ ] `DELETE /api/workflows/:id` - Delete workflow
- [ ] `POST /api/workflows/:id/execute` - Execute workflow
- [ ] `POST /api/workflows/from-template` - Create from template

### Work Stacks (`/api/workstacks/*`)
- [ ] `GET /api/workstacks` - List work stacks
- [ ] `GET /api/workstacks/active` - Active stacks
- [ ] `GET /api/workstacks/:id` - Stack details
- [ ] `GET /api/workstacks/:id/history` - Stack execution history
- [ ] `POST /api/workstacks` - Create stack
- [ ] `PUT /api/workstacks/:id` - Update stack
- [ ] `PUT /api/workstacks/:id/context` - Update stack context
- [ ] `DELETE /api/workstacks/:id` - Delete stack
- [ ] `POST /api/workstacks/:id/push` - Push task to stack
- [ ] `POST /api/workstacks/:id/pop` - Pop task from stack
- [ ] `POST /api/workstacks/:id/control` - Control stack (start/pause/resume)

### Orchestration (`/api/orchestration/*`)
- [ ] `GET /api/orchestration/status` - Engine status
- [ ] `GET /api/orchestration/graph` - Execution graph
- [ ] `GET /api/orchestration/queue` - Execution queue
- [ ] `GET /api/orchestration/executions` - Active executions
- [ ] `GET /api/orchestration/resources` - Resource allocation
- [ ] `GET /api/orchestration/anti-hallucination` - Anti-hallucination metrics
- [ ] `GET /api/orchestration/process-mining` - Process mining data
- [ ] `POST /api/orchestration/plan` - Create execution plan
- [ ] `POST /api/orchestration/execute` - Execute plan
- [ ] `POST /api/orchestration/pause` - Pause execution
- [ ] `POST /api/orchestration/resume` - Resume execution
- [ ] `POST /api/orchestration/cancel` - Cancel execution
- [ ] `PUT /api/orchestration/policies` - Update policies
- [ ] `POST /api/orchestration/debug/step` - Debug step control

---

## Priority 3: Chat & Logs

### Chat Interface (`/api/chat/*`)
- [ ] `GET /api/chat/sessions` - List chat sessions
- [ ] `GET /api/chat/:id/messages` - Get messages
- [ ] `POST /api/chat/message` - Send message
- [ ] `GET /api/chat/system-prompt` - Get system prompt
- [ ] `GET /api/chat/system-prompt-templates` - Get templates
- [ ] `PUT /api/chat/system-prompt` - Update system prompt

### Logs (`/api/logs/*`)
- [ ] `GET /api/logs` - Query logs with filters
- [ ] `GET /api/execution/logs` - Execution logs

---

## Priority 4: Analytics & Settings

### Analytics (`/api/analytics/*`)
- [ ] `GET /api/analytics/vpn` - VPN usage metrics
- [ ] `GET /api/analytics/vpn/traffic` - Data transfer over time
- [ ] `GET /api/analytics/chat/messages` - Messages per day
- [ ] `GET /api/analytics/mail` - Mail server metrics
- [ ] `GET /api/analytics/mail/volume` - Email volume over time
- [ ] `GET /api/analytics/mcp` - MCP usage metrics
- [ ] `GET /api/analytics/performance` - Performance metrics

### Settings (`/api/settings/*`)
- [ ] `GET /api/settings/general` - Get general settings
- [ ] `PUT /api/settings/general` - Update general settings
- [ ] `GET /api/settings/smtp` - Get SMTP config
- [ ] `PUT /api/settings/smtp` - Update SMTP config
- [ ] `POST /api/settings/smtp/test` - Test SMTP connection
- [ ] `GET /api/settings/api-keys` - List API keys
- [ ] `POST /api/settings/api-keys` - Generate API key
- [ ] `DELETE /api/settings/api-keys/:id` - Revoke API key
- [ ] `POST /api/settings/backup` - Create backup
- [ ] `POST /api/settings/restore` - Restore from backup

---

## Priority 5: Real-Time Updates

### WebSocket (`/ws`)
- [ ] WebSocket endpoint at `/ws`
- [ ] Message types:
  - `vpn_status` - VPN status updates
  - `new_connection` - New VPN connection
  - `disconnection` - VPN disconnection
  - `log` - Log entry
  - `chat_message` - Chat message
  - `tool_execution` - Tool execution event
  - `workflow_update` - Workflow status update
  - `orchestration_event` - Orchestration event
  - `debug_event` - Debug event
  - `activity` - Activity feed event

### Server-Sent Events (SSE)
- [ ] `GET /api/logs/stream` - Streaming logs
- [ ] `GET /api/chat/:id/stream` - Streaming chat
- [ ] `GET /api/agents/:id/activity` - Agent activity stream
- [ ] `GET /api/workflows/:id/executions/:exec_id/logs` - Workflow logs stream

---

## Implementation Strategy

### Phase 1: Basic Functionality (Week 1)
1. Implement user management endpoints
2. Implement VPN status endpoints
3. Implement mail server endpoints
4. Implement dashboard overview/analytics
5. Add WebSocket support for real-time VPN updates

### Phase 2: Orchestration Core (Week 2)
1. Implement MCP service management
2. Implement tools management
3. Implement agents management
4. Add basic orchestration control

### Phase 3: Advanced Features (Week 3)
1. Implement workflows
2. Implement work stacks
3. Add advanced orchestration (debugger, process mining)
4. Implement chat interface

### Phase 4: Polish (Week 4)
1. Add all analytics endpoints
2. Implement settings management
3. Add SSE streaming for logs
4. Performance optimization
5. Error handling improvements

---

## Database Schema Needed

```sql
-- Users (already exists in op-web)
CREATE TABLE users (
  id TEXT PRIMARY KEY,
  email TEXT UNIQUE NOT NULL,
  wireguard_ip TEXT NOT NULL,
  wireguard_public_key TEXT NOT NULL,
  wireguard_private_key TEXT NOT NULL,
  status TEXT DEFAULT 'active',
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  last_seen TIMESTAMP
);

-- User Activity
CREATE TABLE user_activity (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id TEXT NOT NULL,
  event_type TEXT NOT NULL, -- 'connected', 'disconnected'
  timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  duration INTEGER, -- seconds
  data_rx INTEGER, -- bytes
  data_tx INTEGER, -- bytes
  FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Mail Queue (if not already in Maddy)
CREATE TABLE mail_queue (
  id TEXT PRIMARY KEY,
  from_email TEXT NOT NULL,
  to_email TEXT NOT NULL,
  subject TEXT,
  status TEXT DEFAULT 'queued', -- 'queued', 'sending', 'sent', 'failed'
  retry_count INTEGER DEFAULT 0,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  sent_at TIMESTAMP
);

-- Agents
CREATE TABLE agents (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  type TEXT NOT NULL,
  status TEXT DEFAULT 'idle', -- 'idle', 'running', 'error', 'paused'
  config TEXT, -- JSON
  current_task TEXT,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Workflows
CREATE TABLE workflows (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  definition TEXT NOT NULL, -- JSON
  last_run TIMESTAMP,
  success_count INTEGER DEFAULT 0,
  failure_count INTEGER DEFAULT 0,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Workflow Executions
CREATE TABLE workflow_executions (
  id TEXT PRIMARY KEY,
  workflow_id TEXT NOT NULL,
  status TEXT DEFAULT 'running', -- 'running', 'completed', 'failed', 'paused'
  started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  completed_at TIMESTAMP,
  result TEXT, -- JSON
  FOREIGN KEY (workflow_id) REFERENCES workflows(id)
);

-- Work Stacks
CREATE TABLE work_stacks (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  priority TEXT DEFAULT 'medium',
  status TEXT DEFAULT 'queued',
  context TEXT, -- JSON
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Stack Tasks
CREATE TABLE stack_tasks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  stack_id TEXT NOT NULL,
  position INTEGER NOT NULL,
  name TEXT NOT NULL,
  status TEXT DEFAULT 'pending',
  completed_at TIMESTAMP,
  duration INTEGER,
  result TEXT, -- JSON
  FOREIGN KEY (stack_id) REFERENCES work_stacks(id)
);

-- Tool Executions (Logs)
CREATE TABLE tool_executions (
  id TEXT PRIMARY KEY,
  tool_name TEXT NOT NULL,
  input TEXT, -- JSON
  output TEXT, -- JSON
  status TEXT NOT NULL, -- 'success', 'failed'
  duration INTEGER, -- milliseconds
  user_id TEXT,
  triggered_by TEXT, -- 'agent', 'workflow', 'manual'
  error TEXT,
  timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Settings
CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- API Keys
CREATE TABLE api_keys (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  key_hash TEXT NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  last_used TIMESTAMP
);
```

---

## Testing Checklist

Once APIs are implemented:

- [ ] Test each endpoint with curl/Postman
- [ ] Verify WebSocket connection from dashboard
- [ ] Check CORS headers for dashboard origin
- [ ] Test real-time updates (VPN connections, logs)
- [ ] Verify database queries are performant
- [ ] Check error handling (500s return proper JSON)
- [ ] Test authentication/authorization
- [ ] Verify file uploads work (if any)
- [ ] Test SSE streams don't timeout
- [ ] Load test with multiple concurrent users

---

## Quick Start: Minimal Working Dashboard

**To get basic functionality working ASAP, implement these 10 endpoints first:**

1. ✅ `POST /api/privacy/signup` (already done)
2. ✅ `GET /api/privacy/verify` (already done)
3. `GET /api/users` - Show user list
4. `GET /api/vpn/status` - Show VPN status
5. `GET /api/vpn/connections` - Show active connections
6. `GET /api/mail/status` - Show mail server status
7. `GET /api/analytics/overview` - Dashboard metrics
8. `GET /api/health` - System health
9. `GET /api/logs` - Basic logs
10. `WS /ws` - WebSocket for real-time updates

This gives you a functional dashboard with the most important features!
