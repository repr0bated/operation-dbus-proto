# Operation-DBUS Dashboard - Complete Specification

Build a comprehensive real-time dashboard for operation-dbus, a privacy-focused VPN service with AI orchestration capabilities.

## Tech Stack
- React 18+ with TypeScript
- Vite for build tooling
- shadcn/ui component library (already configured)
- Tailwind CSS for styling
- Recharts for data visualization
- React Router for navigation
- Lucide React for icons
- WebSocket for real-time updates

## Design System

### Color Palette
- Primary: Blue (#3b82f6)
- Success: Green (#10b981)
- Warning: Amber (#f59e0b)
- Error: Red (#ef4444)
- Background: Slate-900 (#0f172a)
- Surface: Slate-800 (#1e293b)
- Text: Slate-100 (#f1f5f9)

### Layout
- Sidebar navigation (left, 240px wide)
- Main content area (responsive)
- Top bar with user info and status indicators
- Dark theme by default

## API Backend
All API calls go to `/api/` which nginx proxies to `http://127.0.0.1:8080`

### Core API Endpoints

**Authentication & Users**
```
POST   /api/privacy/signup          - Create new user account
GET    /api/privacy/verify?token=   - Verify magic link
GET    /api/users                   - List all users
GET    /api/users/:id               - Get user details
DELETE /api/users/:id               - Delete user
PUT    /api/users/:id/status        - Update user status
```

**VPN Management**
```
GET    /api/vpn/status              - Overall VPN status
GET    /api/vpn/connections         - Active connections
GET    /api/vpn/peers               - WireGuard peers list
GET    /api/vpn/config/:userId      - Get user VPN config
POST   /api/vpn/revoke/:userId      - Revoke user access
```

**Mail Server**
```
GET    /api/mail/status             - Maddy server status
GET    /api/mail/queue              - Mail queue
GET    /api/mail/accounts           - Email accounts list
POST   /api/mail/send               - Send email (for magic links)
```

**MCP Services**
```
GET    /api/mcp/services            - List MCP services
GET    /api/mcp/services/:id        - Service details
POST   /api/mcp/services/:id/start  - Start service
POST   /api/mcp/services/:id/stop   - Stop service
GET    /api/mcp/tools               - Available MCP tools
POST   /api/mcp/tools/execute       - Execute MCP tool
```

**Tools Management**
```
GET    /api/tools                   - List available tools
GET    /api/tools/:id               - Tool details
POST   /api/tools/execute           - Execute tool
GET    /api/tools/history           - Execution history
```

**Agents**
```
GET    /api/agents                  - List agents
GET    /api/agents/:id              - Agent details
POST   /api/agents                  - Create agent
PUT    /api/agents/:id              - Update agent
DELETE /api/agents/:id              - Delete agent
POST   /api/agents/:id/start        - Start agent
POST   /api/agents/:id/stop         - Stop agent
```

**Workflows**
```
GET    /api/workflows               - List workflows
GET    /api/workflows/:id           - Workflow details
POST   /api/workflows               - Create workflow
PUT    /api/workflows/:id           - Update workflow
DELETE /api/workflows/:id           - Delete workflow
POST   /api/workflows/:id/execute   - Execute workflow
GET    /api/workflows/:id/runs      - Workflow execution history
```

**Work Stacks**
```
GET    /api/workstacks              - List work stacks
GET    /api/workstacks/:id          - Work stack details
POST   /api/workstacks              - Create work stack
PUT    /api/workstacks/:id          - Update work stack
DELETE /api/workstacks/:id          - Delete work stack
POST   /api/workstacks/:id/push     - Push task to stack
POST   /api/workstacks/:id/pop      - Pop task from stack
```

**Orchestration**
```
GET    /api/orchestration/status    - Orchestration engine status
GET    /api/orchestration/graph     - Execution graph
POST   /api/orchestration/plan      - Create execution plan
POST   /api/orchestration/execute   - Execute plan
GET    /api/orchestration/logs      - Execution logs
POST   /api/orchestration/pause     - Pause execution
POST   /api/orchestration/resume    - Resume execution
POST   /api/orchestration/cancel    - Cancel execution
```

**Chat & Logs**
```
GET    /api/chat/history            - Chat history
POST   /api/chat/message            - Send message
GET    /api/logs/stream             - Server-Sent Events for real-time logs
GET    /api/logs                    - Query logs
GET    /api/system/prompt           - Get system prompt
PUT    /api/system/prompt           - Update system prompt
```

**Analytics**
```
GET    /api/analytics/overview      - System overview stats
GET    /api/analytics/vpn           - VPN usage metrics
GET    /api/analytics/mail          - Mail server metrics
GET    /api/analytics/mcp           - MCP usage metrics
GET    /api/analytics/performance   - Performance metrics
```

**WebSocket**
```
WS     /ws                          - Real-time updates
```

WebSocket message types:
```typescript
type WSMessage =
  | { type: 'vpn_status', data: VPNStatus }
  | { type: 'new_connection', data: Connection }
  | { type: 'disconnection', data: { userId: string } }
  | { type: 'log', data: LogEntry }
  | { type: 'chat_message', data: ChatMessage }
  | { type: 'tool_execution', data: ToolExecution }
  | { type: 'workflow_update', data: WorkflowUpdate }
  | { type: 'orchestration_event', data: OrchestrationEvent }
```

## Page-by-Page Specifications

### 1. Main Dashboard (`/`)

**Layout**: Grid with metric cards and charts

**Components**:
- System status indicator (top right)
- 4 metric cards in a row:
  - Active VPN Connections (with trend)
  - Total Users (with growth rate)
  - Mail Queue Size (with alert if >10)
  - Active MCP Services (with health status)
- 2-column layout below:
  - Left: VPN Connection Timeline (line chart, last 24h)
  - Right: Recent Activity Feed (scrollable list)
- Bottom section: Resource Usage (CPU, Memory, Network - 3 gauge charts)

**Real-time Updates**:
- WebSocket updates for all metrics
- Refresh every 5 seconds as fallback

**Code Structure**:
```typescript
// src/pages/Dashboard.tsx
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts'
import { Activity, Users, Mail, Server } from 'lucide-react'

export default function Dashboard() {
  const [metrics, setMetrics] = useState<DashboardMetrics>()
  const [connections, setConnections] = useState<ConnectionData[]>([])

  // WebSocket connection
  useEffect(() => {
    const ws = new WebSocket(`wss://${window.location.host}/ws`)
    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data)
      handleWSMessage(msg)
    }
    return () => ws.close()
  }, [])

  // Fetch initial data
  useEffect(() => {
    fetchDashboardData()
  }, [])

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-3xl font-bold">Dashboard</h1>

      <div className="grid grid-cols-4 gap-4">
        <MetricCard icon={Activity} title="VPN Connections" value={metrics?.activeConnections} />
        <MetricCard icon={Users} title="Total Users" value={metrics?.totalUsers} />
        <MetricCard icon={Mail} title="Mail Queue" value={metrics?.mailQueue} />
        <MetricCard icon={Server} title="MCP Services" value={metrics?.mcpServices} />
      </div>

      <div className="grid grid-cols-2 gap-4">
        <Card>
          <CardHeader>
            <CardTitle>VPN Connections (24h)</CardTitle>
          </CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={300}>
              <LineChart data={connections}>
                <XAxis dataKey="time" />
                <YAxis />
                <Tooltip />
                <Line type="monotone" dataKey="count" stroke="#3b82f6" />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Recent Activity</CardTitle>
          </CardHeader>
          <CardContent>
            <ActivityFeed />
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Resource Usage</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-3 gap-4">
            <GaugeChart title="CPU" value={metrics?.cpu} />
            <GaugeChart title="Memory" value={metrics?.memory} />
            <GaugeChart title="Network" value={metrics?.network} />
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
```

---

### 2. Chat Interface (`/chat`)

**Layout**: Full-page chat with tabs

**Components**:
- Tab navigation: "Chat", "Streaming Logs", "System Prompt"
- Chat tab: Message list + input area at bottom
- Logs tab: Auto-scrolling log viewer with filters
- System Prompt tab: Editable text area with save button

**Chat Features**:
- Message bubbles (user vs assistant)
- Typing indicator
- Code block syntax highlighting
- Markdown rendering
- Message timestamps

**Streaming Logs Features**:
- Real-time log stream via SSE (Server-Sent Events)
- Log level filters (DEBUG, INFO, WARN, ERROR)
- Component filters (vpn, mail, mcp, orchestration)
- Search box
- Auto-scroll toggle
- Export logs button

**System Prompt Features**:
- Monospace editor
- Save button (PUT /api/system/prompt)
- Character count
- Reset to default button

**Code Structure**:
```typescript
// src/pages/Chat.tsx
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"

export default function Chat() {
  const [activeTab, setActiveTab] = useState('chat')

  return (
    <div className="h-screen flex flex-col">
      <div className="border-b p-4">
        <h1 className="text-2xl font-bold">Chat & Logs</h1>
      </div>

      <Tabs value={activeTab} onValueChange={setActiveTab} className="flex-1 flex flex-col">
        <TabsList className="mx-4 mt-2">
          <TabsTrigger value="chat">Chat</TabsTrigger>
          <TabsTrigger value="logs">Streaming Logs</TabsTrigger>
          <TabsTrigger value="prompt">System Prompt</TabsTrigger>
        </TabsList>

        <TabsContent value="chat" className="flex-1">
          <ChatView />
        </TabsContent>

        <TabsContent value="logs" className="flex-1">
          <LogsView />
        </TabsContent>

        <TabsContent value="prompt" className="flex-1 p-4">
          <SystemPromptEditor />
        </TabsContent>
      </Tabs>
    </div>
  )
}

// Streaming logs implementation
function LogsView() {
  const [logs, setLogs] = useState<LogEntry[]>([])
  const [autoScroll, setAutoScroll] = useState(true)

  useEffect(() => {
    const eventSource = new EventSource('/api/logs/stream')
    eventSource.onmessage = (event) => {
      const log = JSON.parse(event.data)
      setLogs(prev => [...prev, log])
    }
    return () => eventSource.close()
  }, [])

  return (
    <div className="flex flex-col h-full p-4">
      <div className="flex gap-2 mb-4">
        <Select onValueChange={setLogLevel}>
          <SelectTrigger className="w-32">
            <SelectValue placeholder="Log Level" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All</SelectItem>
            <SelectItem value="debug">Debug</SelectItem>
            <SelectItem value="info">Info</SelectItem>
            <SelectItem value="warn">Warn</SelectItem>
            <SelectItem value="error">Error</SelectItem>
          </SelectContent>
        </Select>

        <Input placeholder="Search logs..." />

        <Button variant="outline" onClick={() => setAutoScroll(!autoScroll)}>
          {autoScroll ? 'Auto-scroll: ON' : 'Auto-scroll: OFF'}
        </Button>

        <Button variant="outline">Export</Button>
      </div>

      <ScrollArea className="flex-1 border rounded">
        <div className="p-4 font-mono text-sm space-y-1">
          {logs.map((log, i) => (
            <div key={i} className={`log-${log.level}`}>
              <span className="text-slate-500">[{log.timestamp}]</span>
              <span className="text-blue-400 ml-2">[{log.component}]</span>
              <span className="ml-2">{log.message}</span>
            </div>
          ))}
        </div>
      </ScrollArea>
    </div>
  )
}
```

---

### 3. Users Management (`/users`)

**Layout**: Table with filters and action buttons

**Components**:
- Search bar
- Status filter dropdown (All, Active, Suspended)
- Add User button
- User table with columns:
  - Email
  - WireGuard IP
  - Status (badge)
  - Created Date
  - Last Seen
  - Actions (View, Suspend, Delete)
- User detail modal (click row to open)
- Pagination

**User Detail Modal**:
- User info card
- VPN configuration (with copy button)
- QR code for mobile
- Activity log
- Revoke Access button

**Code Structure**:
```typescript
// src/pages/Users.tsx
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"

export default function Users() {
  const [users, setUsers] = useState<User[]>([])
  const [selectedUser, setSelectedUser] = useState<User | null>(null)

  useEffect(() => {
    fetch('/api/users')
      .then(res => res.json())
      .then(setUsers)
  }, [])

  return (
    <div className="p-6">
      <div className="flex justify-between mb-6">
        <h1 className="text-3xl font-bold">Users</h1>
        <Button>Add User</Button>
      </div>

      <div className="flex gap-4 mb-4">
        <Input placeholder="Search users..." className="max-w-sm" />
        <Select>
          <SelectTrigger className="w-40">
            <SelectValue placeholder="Status" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All</SelectItem>
            <SelectItem value="active">Active</SelectItem>
            <SelectItem value="suspended">Suspended</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <Card>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Email</TableHead>
              <TableHead>WireGuard IP</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Created</TableHead>
              <TableHead>Last Seen</TableHead>
              <TableHead>Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {users.map(user => (
              <TableRow key={user.id} onClick={() => setSelectedUser(user)} className="cursor-pointer">
                <TableCell>{user.email}</TableCell>
                <TableCell className="font-mono">{user.wireguard_ip}</TableCell>
                <TableCell>
                  <Badge variant={user.status === 'active' ? 'default' : 'secondary'}>
                    {user.status}
                  </Badge>
                </TableCell>
                <TableCell>{formatDate(user.created_at)}</TableCell>
                <TableCell>{formatDate(user.last_seen)}</TableCell>
                <TableCell>
                  <Button variant="ghost" size="sm">View</Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Card>

      <Dialog open={!!selectedUser} onOpenChange={() => setSelectedUser(null)}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>User Details</DialogTitle>
          </DialogHeader>
          <UserDetails user={selectedUser} />
        </DialogContent>
      </Dialog>
    </div>
  )
}
```

---

### 4. VPN Status (`/vpn`)

**Layout**: Status cards + connection table + network graph

**Components**:
- WireGuard status indicator (green/red)
- 3 metric cards:
  - Active Connections
  - Total Bandwidth (up/down)
  - Peer Count
- Active Connections table:
  - User Email
  - IP Address
  - Connected Since
  - RX/TX Bytes
  - Last Handshake
- Network topology graph (optional, use react-flow)

**Real-time Updates**:
- WebSocket for connection changes
- Bandwidth updates every second

**Code Structure**:
```typescript
// src/pages/VPN.tsx
export default function VPN() {
  const [status, setStatus] = useState<VPNStatus>()
  const [connections, setConnections] = useState<Connection[]>([])

  useEffect(() => {
    const ws = new WebSocket(`wss://${window.location.host}/ws`)
    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data)
      if (msg.type === 'vpn_status') setStatus(msg.data)
      if (msg.type === 'new_connection') setConnections(prev => [...prev, msg.data])
      if (msg.type === 'disconnection') setConnections(prev => prev.filter(c => c.userId !== msg.data.userId))
    }
    return () => ws.close()
  }, [])

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-3xl font-bold">VPN Status</h1>

      <div className="flex items-center gap-2">
        <div className={`h-3 w-3 rounded-full ${status?.running ? 'bg-green-500' : 'bg-red-500'}`} />
        <span className="text-lg">WireGuard: {status?.running ? 'Running' : 'Stopped'}</span>
      </div>

      <div className="grid grid-cols-3 gap-4">
        <MetricCard title="Active Connections" value={connections.length} />
        <MetricCard title="Bandwidth" value={formatBandwidth(status?.bandwidth)} />
        <MetricCard title="Total Peers" value={status?.peerCount} />
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Active Connections</CardTitle>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>User</TableHead>
                <TableHead>IP Address</TableHead>
                <TableHead>Connected</TableHead>
                <TableHead>RX/TX</TableHead>
                <TableHead>Last Handshake</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {connections.map(conn => (
                <TableRow key={conn.userId}>
                  <TableCell>{conn.email}</TableCell>
                  <TableCell className="font-mono">{conn.ip}</TableCell>
                  <TableCell>{formatDuration(conn.connectedSince)}</TableCell>
                  <TableCell>{formatBytes(conn.rx)} / {formatBytes(conn.tx)}</TableCell>
                  <TableCell>{formatTime(conn.lastHandshake)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  )
}
```

---

### 5. Mail Server (`/mail`)

**Layout**: Status overview + queue + accounts

**Components**:
- Maddy service status (running/stopped)
- 3 metric cards:
  - Messages Sent (today)
  - Queue Size
  - Active Accounts
- Mail Queue table:
  - ID
  - From
  - To
  - Subject
  - Status (queued, sending, failed)
  - Retry Count
  - Actions (Retry, Delete)
- Email Accounts table:
  - Email Address
  - Created Date
  - Total Sent
  - Total Received

**Code Structure**:
```typescript
// src/pages/Mail.tsx
export default function Mail() {
  const [status, setStatus] = useState<MailStatus>()
  const [queue, setQueue] = useState<MailQueueItem[]>([])
  const [accounts, setAccounts] = useState<MailAccount[]>([])

  useEffect(() => {
    Promise.all([
      fetch('/api/mail/status').then(r => r.json()),
      fetch('/api/mail/queue').then(r => r.json()),
      fetch('/api/mail/accounts').then(r => r.json()),
    ]).then(([s, q, a]) => {
      setStatus(s)
      setQueue(q)
      setAccounts(a)
    })
  }, [])

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-3xl font-bold">Mail Server</h1>

      <Alert>
        <Mail className="h-4 w-4" />
        <AlertTitle>Maddy Mail Server</AlertTitle>
        <AlertDescription>
          Status: {status?.running ? 'Running' : 'Stopped'} |
          SMTP: {status?.smtpPort} |
          IMAP: {status?.imapPort}
        </AlertDescription>
      </Alert>

      <div className="grid grid-cols-3 gap-4">
        <MetricCard title="Messages Sent Today" value={status?.sentToday} />
        <MetricCard title="Queue Size" value={queue.length} alert={queue.length > 10} />
        <MetricCard title="Active Accounts" value={accounts.length} />
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Mail Queue</CardTitle>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>From</TableHead>
                <TableHead>To</TableHead>
                <TableHead>Subject</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Retry Count</TableHead>
                <TableHead>Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {queue.map(item => (
                <TableRow key={item.id}>
                  <TableCell>{item.from}</TableCell>
                  <TableCell>{item.to}</TableCell>
                  <TableCell>{item.subject}</TableCell>
                  <TableCell>
                    <Badge variant={item.status === 'failed' ? 'destructive' : 'default'}>
                      {item.status}
                    </Badge>
                  </TableCell>
                  <TableCell>{item.retryCount}</TableCell>
                  <TableCell>
                    <Button size="sm" variant="ghost">Retry</Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  )
}
```

---

### 6. MCP Services (`/mcp`)

**Layout**: Service cards grid + tools list

**Components**:
- MCP service cards (grid layout):
  - Service name & icon
  - Status indicator (running/stopped)
  - Connection status
  - Start/Stop button
  - View Details button
- Available Tools section:
  - Tool name
  - Description
  - Input schema
  - Execute button
- Service detail modal:
  - Configuration
  - Available tools
  - Logs
  - Health checks

**Code Structure**:
```typescript
// src/pages/MCP.tsx
export default function MCP() {
  const [services, setServices] = useState<MCPService[]>([])
  const [tools, setTools] = useState<MCPTool[]>([])

  useEffect(() => {
    Promise.all([
      fetch('/api/mcp/services').then(r => r.json()),
      fetch('/api/mcp/tools').then(r => r.json()),
    ]).then(([s, t]) => {
      setServices(s)
      setTools(t)
    })
  }, [])

  const handleStartStop = async (id: string, action: 'start' | 'stop') => {
    await fetch(`/api/mcp/services/${id}/${action}`, { method: 'POST' })
    // Refresh services
  }

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-3xl font-bold">MCP Services</h1>

      <div className="grid grid-cols-3 gap-4">
        {services.map(service => (
          <Card key={service.id}>
            <CardHeader>
              <div className="flex items-center justify-between">
                <CardTitle>{service.name}</CardTitle>
                <div className={`h-2 w-2 rounded-full ${service.running ? 'bg-green-500' : 'bg-red-500'}`} />
              </div>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-slate-400 mb-4">{service.description}</p>
              <div className="flex gap-2">
                <Button
                  size="sm"
                  onClick={() => handleStartStop(service.id, service.running ? 'stop' : 'start')}
                >
                  {service.running ? 'Stop' : 'Start'}
                </Button>
                <Button size="sm" variant="outline">Details</Button>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Available Tools</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            {tools.map(tool => (
              <div key={tool.name} className="border rounded p-4">
                <div className="flex justify-between items-start mb-2">
                  <div>
                    <h3 className="font-semibold">{tool.name}</h3>
                    <p className="text-sm text-slate-400">{tool.description}</p>
                  </div>
                  <Button size="sm">Execute</Button>
                </div>
                <details className="text-xs text-slate-500">
                  <summary className="cursor-pointer">Input Schema</summary>
                  <pre className="mt-2 p-2 bg-slate-900 rounded">
                    {JSON.stringify(tool.inputSchema, null, 2)}
                  </pre>
                </details>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
```

---

### 7. Analytics (`/analytics`)

**Layout**: Charts and metrics dashboard

**Components**:
- Time range selector (24h, 7d, 30d, custom)
- 4-column metric cards:
  - Total Requests
  - Average Response Time
  - Error Rate
  - Uptime Percentage
- Charts section (2x2 grid):
  - VPN Usage Over Time (line chart)
  - Request Distribution (bar chart)
  - Error Types (pie chart)
  - Response Times (histogram)
- System Performance section:
  - CPU Usage timeline
  - Memory Usage timeline
  - Network I/O timeline

**Code Structure**:
```typescript
// src/pages/Analytics.tsx
import { LineChart, BarChart, PieChart } from 'recharts'

export default function Analytics() {
  const [timeRange, setTimeRange] = useState('24h')
  const [metrics, setMetrics] = useState<AnalyticsMetrics>()

  useEffect(() => {
    fetch(`/api/analytics/overview?range=${timeRange}`)
      .then(r => r.json())
      .then(setMetrics)
  }, [timeRange])

  return (
    <div className="p-6 space-y-6">
      <div className="flex justify-between items-center">
        <h1 className="text-3xl font-bold">Analytics</h1>
        <Select value={timeRange} onValueChange={setTimeRange}>
          <SelectTrigger className="w-32">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="24h">Last 24h</SelectItem>
            <SelectItem value="7d">Last 7 days</SelectItem>
            <SelectItem value="30d">Last 30 days</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div className="grid grid-cols-4 gap-4">
        <MetricCard title="Total Requests" value={metrics?.totalRequests} />
        <MetricCard title="Avg Response Time" value={`${metrics?.avgResponseTime}ms`} />
        <MetricCard title="Error Rate" value={`${metrics?.errorRate}%`} />
        <MetricCard title="Uptime" value={`${metrics?.uptime}%`} />
      </div>

      <div className="grid grid-cols-2 gap-4">
        <Card>
          <CardHeader>
            <CardTitle>VPN Usage</CardTitle>
          </CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={250}>
              <LineChart data={metrics?.vpnUsage}>
                <XAxis dataKey="time" />
                <YAxis />
                <Tooltip />
                <Line type="monotone" dataKey="connections" stroke="#3b82f6" />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Request Distribution</CardTitle>
          </CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={250}>
              <BarChart data={metrics?.requestDist}>
                <XAxis dataKey="endpoint" />
                <YAxis />
                <Tooltip />
                <Bar dataKey="count" fill="#10b981" />
              </BarChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
```

---

### 8. Settings (`/settings`)

**Layout**: Form-based configuration

**Components**:
- Tabs for different settings categories:
  - General
  - VPN Configuration
  - Mail Server
  - Security
  - API Keys
- Each tab contains form fields with Save button
- Danger zone at bottom (reset, factory defaults)

**Settings Categories**:

**General**:
- Site name
- Admin email
- Timezone
- Language

**VPN Configuration**:
- WireGuard interface
- Network CIDR
- DNS servers
- MTU
- Keepalive interval

**Mail Server**:
- SMTP host
- SMTP port
- SMTP credentials
- From address
- Mail templates

**Security**:
- Session timeout
- Magic link expiration
- Rate limiting
- IP whitelist/blacklist

**API Keys**:
- List of API keys
- Create new key
- Revoke key

**Code Structure**:
```typescript
// src/pages/Settings.tsx
export default function Settings() {
  const [activeTab, setActiveTab] = useState('general')

  return (
    <div className="p-6">
      <h1 className="text-3xl font-bold mb-6">Settings</h1>

      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <TabsList>
          <TabsTrigger value="general">General</TabsTrigger>
          <TabsTrigger value="vpn">VPN</TabsTrigger>
          <TabsTrigger value="mail">Mail</TabsTrigger>
          <TabsTrigger value="security">Security</TabsTrigger>
          <TabsTrigger value="api">API Keys</TabsTrigger>
        </TabsList>

        <TabsContent value="general">
          <Card>
            <CardHeader>
              <CardTitle>General Settings</CardTitle>
            </CardHeader>
            <CardContent>
              <form className="space-y-4">
                <div>
                  <Label>Site Name</Label>
                  <Input defaultValue="Operation DBUS" />
                </div>
                <div>
                  <Label>Admin Email</Label>
                  <Input type="email" defaultValue="admin@3tched.com" />
                </div>
                <Button>Save Changes</Button>
              </form>
            </CardContent>
          </Card>
        </TabsContent>

        {/* Similar for other tabs */}
      </Tabs>

      <Card className="mt-6 border-red-500">
        <CardHeader>
          <CardTitle className="text-red-500">Danger Zone</CardTitle>
        </CardHeader>
        <CardContent>
          <Button variant="destructive">Reset All Settings</Button>
        </CardContent>
      </Card>
    </div>
  )
}
```

---

### 9. Tools Management (`/tools`)

**Layout**: Tool library with execution interface

**Components**:
- Search bar for tools
- Category filter (System, Network, Database, Custom)
- Tools grid:
  - Tool card with name, description, category
  - Execute button
  - View History button
- Tool execution modal:
  - Input form (dynamic based on tool schema)
  - Execute button
  - Output display (JSON, logs, or formatted result)
  - Save to favorites
- Execution history table:
  - Tool name
  - Parameters (collapsed)
  - Result (success/failure)
  - Execution time
  - Duration
  - View Details button

**Code Structure**:
```typescript
// src/pages/Tools.tsx
export default function Tools() {
  const [tools, setTools] = useState<Tool[]>([])
  const [selectedTool, setSelectedTool] = useState<Tool | null>(null)
  const [executing, setExecuting] = useState(false)

  useEffect(() => {
    fetch('/api/tools').then(r => r.json()).then(setTools)
  }, [])

  const executeTool = async (params: any) => {
    setExecuting(true)
    try {
      const res = await fetch('/api/tools/execute', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ tool: selectedTool.id, params })
      })
      const result = await res.json()
      // Show result
    } finally {
      setExecuting(false)
    }
  }

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-3xl font-bold">Tools</h1>

      <div className="flex gap-4">
        <Input placeholder="Search tools..." className="max-w-sm" />
        <Select>
          <SelectTrigger className="w-40">
            <SelectValue placeholder="Category" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All</SelectItem>
            <SelectItem value="system">System</SelectItem>
            <SelectItem value="network">Network</SelectItem>
            <SelectItem value="database">Database</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div className="grid grid-cols-3 gap-4">
        {tools.map(tool => (
          <Card key={tool.id} className="cursor-pointer hover:border-blue-500"
                onClick={() => setSelectedTool(tool)}>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Wrench className="h-5 w-5" />
                {tool.name}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-slate-400">{tool.description}</p>
              <Badge className="mt-2">{tool.category}</Badge>
            </CardContent>
          </Card>
        ))}
      </div>

      <Dialog open={!!selectedTool} onOpenChange={() => setSelectedTool(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Execute: {selectedTool?.name}</DialogTitle>
          </DialogHeader>
          <ToolExecutionForm tool={selectedTool} onExecute={executeTool} loading={executing} />
        </DialogContent>
      </Dialog>

      <Card>
        <CardHeader>
          <CardTitle>Execution History</CardTitle>
        </CardHeader>
        <CardContent>
          <ExecutionHistoryTable />
        </CardContent>
      </Card>
    </div>
  )
}
```

---

### 10. Agents (`/agents`)

**Layout**: Agent management dashboard

**Components**:
- Create Agent button
- Agents grid:
  - Agent card with name, type, status
  - Start/Stop button
  - Edit button
  - Delete button
- Agent status indicators:
  - Idle (gray)
  - Running (green, with spinner)
  - Error (red)
  - Paused (yellow)
- Agent detail view:
  - Configuration
  - Current task
  - Execution logs
  - Performance metrics
- Create/Edit Agent form:
  - Name
  - Type (Worker, Monitor, Scheduler, Custom)
  - Configuration (JSON editor)
  - Triggers (event-based, scheduled, manual)

**Code Structure**:
```typescript
// src/pages/Agents.tsx
export default function Agents() {
  const [agents, setAgents] = useState<Agent[]>([])
  const [creating, setCreating] = useState(false)

  useEffect(() => {
    fetch('/api/agents').then(r => r.json()).then(setAgents)
  }, [])

  const handleStartStop = async (id: string, action: 'start' | 'stop') => {
    await fetch(`/api/agents/${id}/${action}`, { method: 'POST' })
    // Refresh agents list
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex justify-between">
        <h1 className="text-3xl font-bold">Agents</h1>
        <Button onClick={() => setCreating(true)}>
          <Plus className="mr-2 h-4 w-4" />
          Create Agent
        </Button>
      </div>

      <div className="grid grid-cols-3 gap-4">
        {agents.map(agent => (
          <Card key={agent.id}>
            <CardHeader>
              <div className="flex items-center justify-between">
                <CardTitle>{agent.name}</CardTitle>
                <AgentStatusBadge status={agent.status} />
              </div>
              <p className="text-sm text-slate-400">{agent.type}</p>
            </CardHeader>
            <CardContent>
              <div className="space-y-2">
                <div className="text-sm">
                  <span className="text-slate-400">Current Task:</span> {agent.currentTask || 'None'}
                </div>
                <div className="flex gap-2">
                  <Button
                    size="sm"
                    onClick={() => handleStartStop(agent.id, agent.status === 'running' ? 'stop' : 'start')}
                  >
                    {agent.status === 'running' ? (
                      <><Pause className="mr-1 h-3 w-3" /> Stop</>
                    ) : (
                      <><Play className="mr-1 h-3 w-3" /> Start</>
                    )}
                  </Button>
                  <Button size="sm" variant="outline">
                    <Settings className="h-3 w-3" />
                  </Button>
                  <Button size="sm" variant="destructive">
                    <Trash className="h-3 w-3" />
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <Dialog open={creating} onOpenChange={setCreating}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create New Agent</DialogTitle>
          </DialogHeader>
          <AgentForm onSubmit={createAgent} />
        </DialogContent>
      </Dialog>
    </div>
  )
}
```

---

### 11. Workflows (`/workflows`)

**Layout**: Workflow builder and execution

**Components**:
- Create Workflow button
- Workflows list:
  - Workflow card with name, description, status
  - Last run timestamp
  - Success rate
  - Execute button
  - Edit button
- Workflow builder (visual or YAML):
  - Drag-and-drop steps
  - Step configuration
  - Conditional logic
  - Error handling
  - Save button
- Execution history:
  - Run ID
  - Start time
  - Duration
  - Status (success, failed, running)
  - View logs button

**Code Structure**:
```typescript
// src/pages/Workflows.tsx
export default function Workflows() {
  const [workflows, setWorkflows] = useState<Workflow[]>([])
  const [editing, setEditing] = useState<Workflow | null>(null)

  useEffect(() => {
    fetch('/api/workflows').then(r => r.json()).then(setWorkflows)
  }, [])

  const executeWorkflow = async (id: string) => {
    await fetch(`/api/workflows/${id}/execute`, { method: 'POST' })
    toast.success('Workflow execution started')
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex justify-between">
        <h1 className="text-3xl font-bold">Workflows</h1>
        <Button onClick={() => setEditing({ id: 'new', name: '', steps: [] })}>
          <Plus className="mr-2 h-4 w-4" />
          Create Workflow
        </Button>
      </div>

      <div className="grid grid-cols-2 gap-4">
        {workflows.map(workflow => (
          <Card key={workflow.id}>
            <CardHeader>
              <CardTitle>{workflow.name}</CardTitle>
              <p className="text-sm text-slate-400">{workflow.description}</p>
            </CardHeader>
            <CardContent>
              <div className="space-y-2">
                <div className="flex justify-between text-sm">
                  <span className="text-slate-400">Last Run:</span>
                  <span>{formatDate(workflow.lastRun)}</span>
                </div>
                <div className="flex justify-between text-sm">
                  <span className="text-slate-400">Success Rate:</span>
                  <span>{workflow.successRate}%</span>
                </div>
                <div className="flex justify-between text-sm">
                  <span className="text-slate-400">Steps:</span>
                  <span>{workflow.steps.length}</span>
                </div>
                <div className="flex gap-2 mt-4">
                  <Button size="sm" onClick={() => executeWorkflow(workflow.id)}>
                    <Play className="mr-1 h-3 w-3" />
                    Execute
                  </Button>
                  <Button size="sm" variant="outline" onClick={() => setEditing(workflow)}>
                    <Edit className="mr-1 h-3 w-3" />
                    Edit
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      {editing && (
        <Dialog open={!!editing} onOpenChange={() => setEditing(null)}>
          <DialogContent className="max-w-4xl">
            <DialogHeader>
              <DialogTitle>
                {editing.id === 'new' ? 'Create Workflow' : 'Edit Workflow'}
              </DialogTitle>
            </DialogHeader>
            <WorkflowBuilder workflow={editing} onSave={saveWorkflow} />
          </DialogContent>
        </Dialog>
      )}
    </div>
  )
}
```

---

### 12. Work Stacks (`/stacks`)

**Layout**: Stack-based task management

**Components**:
- Create Stack button
- Stacks grid:
  - Stack card with name, size, status
  - Top task preview
  - Push/Pop buttons
  - View All button
- Stack detail view:
  - Task list (stack order)
  - Push task form
  - Pop task button
  - Clear stack button
- Task cards:
  - Task name
  - Priority
  - Status
  - Assigned agent
  - Created time

**Code Structure**:
```typescript
// src/pages/WorkStacks.tsx
export default function WorkStacks() {
  const [stacks, setStacks] = useState<WorkStack[]>([])
  const [selectedStack, setSelectedStack] = useState<WorkStack | null>(null)

  useEffect(() => {
    fetch('/api/workstacks').then(r => r.json()).then(setStacks)
  }, [])

  const pushTask = async (stackId: string, task: Task) => {
    await fetch(`/api/workstacks/${stackId}/push`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(task)
    })
    // Refresh stack
  }

  const popTask = async (stackId: string) => {
    const res = await fetch(`/api/workstacks/${stackId}/pop`, { method: 'POST' })
    const task = await res.json()
    toast.success(`Popped task: ${task.name}`)
    // Refresh stack
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex justify-between">
        <h1 className="text-3xl font-bold">Work Stacks</h1>
        <Button>
          <Plus className="mr-2 h-4 w-4" />
          Create Stack
        </Button>
      </div>

      <div className="grid grid-cols-3 gap-4">
        {stacks.map(stack => (
          <Card key={stack.id} onClick={() => setSelectedStack(stack)} className="cursor-pointer">
            <CardHeader>
              <CardTitle>{stack.name}</CardTitle>
              <p className="text-sm text-slate-400">
                {stack.tasks.length} tasks
              </p>
            </CardHeader>
            <CardContent>
              {stack.tasks[0] && (
                <div className="border-l-2 border-blue-500 pl-3 mb-4">
                  <p className="text-sm font-medium">Top: {stack.tasks[0].name}</p>
                  <p className="text-xs text-slate-400">{stack.tasks[0].status}</p>
                </div>
              )}
              <div className="flex gap-2">
                <Button size="sm" onClick={(e) => { e.stopPropagation(); popTask(stack.id); }}>
                  Pop
                </Button>
                <Button size="sm" variant="outline">
                  Push
                </Button>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <Dialog open={!!selectedStack} onOpenChange={() => setSelectedStack(null)}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>{selectedStack?.name}</DialogTitle>
          </DialogHeader>
          <StackDetailView stack={selectedStack} />
        </DialogContent>
      </Dialog>
    </div>
  )
}
```

---

### 13. Orchestration Control (`/orchestration`)

**Layout**: Orchestration engine control panel

**Components**:
- Engine status card:
  - Running/Stopped status
  - Current plan name
  - Progress percentage
  - Pause/Resume/Cancel buttons
- Execution graph visualization:
  - Nodes representing tasks/agents
  - Edges showing dependencies
  - Color-coded status (pending, running, completed, failed)
  - Click node for details
- Create Plan form:
  - Plan name
  - Select agents
  - Define dependencies
  - Set triggers
  - Execute button
- Active executions list:
  - Execution ID
  - Plan name
  - Progress
  - Start time
  - Status
  - Actions

**Code Structure**:
```typescript
// src/pages/Orchestration.tsx
import ReactFlow, { Node, Edge } from 'reactflow'
import 'reactflow/dist/style.css'

export default function Orchestration() {
  const [status, setStatus] = useState<OrchestrationStatus>()
  const [graph, setGraph] = useState<{ nodes: Node[], edges: Edge[] }>()

  useEffect(() => {
    const ws = new WebSocket(`wss://${window.location.host}/ws`)
    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data)
      if (msg.type === 'orchestration_event') {
        updateGraph(msg.data)
      }
    }

    fetch('/api/orchestration/status').then(r => r.json()).then(setStatus)
    fetch('/api/orchestration/graph').then(r => r.json()).then(setGraph)

    return () => ws.close()
  }, [])

  const handleControl = async (action: 'pause' | 'resume' | 'cancel') => {
    await fetch(`/api/orchestration/${action}`, { method: 'POST' })
  }

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-3xl font-bold">Orchestration</h1>

      <Card>
        <CardHeader>
          <CardTitle>Engine Status</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-between">
            <div>
              <div className="flex items-center gap-2 mb-2">
                <div className={`h-3 w-3 rounded-full ${status?.running ? 'bg-green-500' : 'bg-red-500'}`} />
                <span className="font-semibold">{status?.running ? 'Running' : 'Stopped'}</span>
              </div>
              {status?.currentPlan && (
                <>
                  <p className="text-sm text-slate-400">Plan: {status.currentPlan}</p>
                  <div className="mt-2">
                    <div className="w-64 h-2 bg-slate-700 rounded-full overflow-hidden">
                      <div
                        className="h-full bg-blue-500 transition-all"
                        style={{ width: `${status.progress}%` }}
                      />
                    </div>
                    <p className="text-xs text-slate-400 mt-1">{status.progress}% complete</p>
                  </div>
                </>
              )}
            </div>
            <div className="flex gap-2">
              <Button onClick={() => handleControl('pause')} disabled={!status?.running}>
                <Pause className="mr-2 h-4 w-4" />
                Pause
              </Button>
              <Button onClick={() => handleControl('resume')} disabled={status?.running}>
                <Play className="mr-2 h-4 w-4" />
                Resume
              </Button>
              <Button variant="destructive" onClick={() => handleControl('cancel')}>
                <X className="mr-2 h-4 w-4" />
                Cancel
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Execution Graph</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="h-96 border rounded">
            {graph && (
              <ReactFlow
                nodes={graph.nodes}
                edges={graph.edges}
                fitView
              />
            )}
          </div>
        </CardContent>
      </Card>

      <div className="grid grid-cols-2 gap-4">
        <Card>
          <CardHeader>
            <CardTitle>Create Plan</CardTitle>
          </CardHeader>
          <CardContent>
            <OrchestrationPlanForm />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Active Executions</CardTitle>
          </CardHeader>
          <CardContent>
            <ActiveExecutionsList />
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
```

---

### 14. Tool Execution Logs (`/logs`)

**Layout**: Searchable log viewer with filters

**Components**:
- Search bar
- Filter controls:
  - Date range picker
  - Tool filter dropdown
  - Status filter (success, failed, all)
  - Log level filter
- Logs table:
  - Timestamp
  - Tool name
  - User
  - Status
  - Duration
  - View Details button
- Log detail modal:
  - Full execution details
  - Input parameters (JSON)
  - Output/result (JSON)
  - Error message (if failed)
  - Stack trace (if error)
  - Export button

**Code Structure**:
```typescript
// src/pages/Logs.tsx
export default function Logs() {
  const [logs, setLogs] = useState<LogEntry[]>([])
  const [filters, setFilters] = useState({
    search: '',
    tool: 'all',
    status: 'all',
    dateFrom: null,
    dateTo: null
  })

  useEffect(() => {
    const params = new URLSearchParams(filters)
    fetch(`/api/logs?${params}`).then(r => r.json()).then(setLogs)
  }, [filters])

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-3xl font-bold">Execution Logs</h1>

      <Card>
        <CardHeader>
          <CardTitle>Filters</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex gap-4">
            <Input
              placeholder="Search logs..."
              value={filters.search}
              onChange={(e) => setFilters({...filters, search: e.target.value})}
              className="max-w-sm"
            />

            <Select value={filters.tool} onValueChange={(v) => setFilters({...filters, tool: v})}>
              <SelectTrigger className="w-40">
                <SelectValue placeholder="Tool" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All Tools</SelectItem>
                <SelectItem value="vpn">VPN</SelectItem>
                <SelectItem value="mail">Mail</SelectItem>
                <SelectItem value="mcp">MCP</SelectItem>
              </SelectContent>
            </Select>

            <Select value={filters.status} onValueChange={(v) => setFilters({...filters, status: v})}>
              <SelectTrigger className="w-40">
                <SelectValue placeholder="Status" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All</SelectItem>
                <SelectItem value="success">Success</SelectItem>
                <SelectItem value="failed">Failed</SelectItem>
              </SelectContent>
            </Select>

            <Button variant="outline">
              <Download className="mr-2 h-4 w-4" />
              Export
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Timestamp</TableHead>
              <TableHead>Tool</TableHead>
              <TableHead>User</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Duration</TableHead>
              <TableHead>Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {logs.map(log => (
              <TableRow key={log.id}>
                <TableCell className="font-mono text-xs">{formatTimestamp(log.timestamp)}</TableCell>
                <TableCell>{log.tool}</TableCell>
                <TableCell>{log.user}</TableCell>
                <TableCell>
                  <Badge variant={log.status === 'success' ? 'default' : 'destructive'}>
                    {log.status}
                  </Badge>
                </TableCell>
                <TableCell>{log.duration}ms</TableCell>
                <TableCell>
                  <Button size="sm" variant="ghost">View</Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Card>
    </div>
  )
}
```

---

### 15. Orchestration Debugger (`/debugger`)

**Layout**: Advanced debugging interface

**Components**:
- Execution selector dropdown
- Timeline view:
  - Horizontal timeline of execution
  - Events marked on timeline
  - Click to jump to event
- State inspector:
  - Current execution state (JSON tree)
  - Variable values
  - Agent states
- Breakpoints panel:
  - Set conditional breakpoints
  - Pause on error toggle
  - Step through execution
- Debug controls:
  - Step Over
  - Step Into
  - Step Out
  - Continue
  - Stop
- Console output:
  - Real-time logs
  - Error messages
  - Warning messages

**Code Structure**:
```typescript
// src/pages/Debugger.tsx
export default function Debugger() {
  const [execution, setExecution] = useState<string>()
  const [state, setState] = useState<any>()
  const [breakpoints, setBreakpoints] = useState<Breakpoint[]>([])
  const [timeline, setTimeline] = useState<TimelineEvent[]>([])
  const [paused, setPaused] = useState(false)

  useEffect(() => {
    if (!execution) return

    const ws = new WebSocket(`wss://${window.location.host}/ws`)
    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data)
      if (msg.type === 'debug_event') {
        handleDebugEvent(msg.data)
      }
    }

    return () => ws.close()
  }, [execution])

  const stepOver = () => {
    fetch(`/api/orchestration/debug/step`, { method: 'POST' })
  }

  return (
    <div className="p-6 space-y-4">
      <div className="flex justify-between items-center">
        <h1 className="text-3xl font-bold">Orchestration Debugger</h1>

        <Select value={execution} onValueChange={setExecution}>
          <SelectTrigger className="w-64">
            <SelectValue placeholder="Select execution..." />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="exec-1">Execution #1234</SelectItem>
            <SelectItem value="exec-2">Execution #1235</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div className="grid grid-cols-3 gap-4">
        <Card className="col-span-2">
          <CardHeader>
            <CardTitle>Timeline</CardTitle>
          </CardHeader>
          <CardContent>
            <TimelineVisualization events={timeline} />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Debug Controls</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              <Button className="w-full" onClick={stepOver} disabled={!paused}>
                <ArrowRight className="mr-2 h-4 w-4" />
                Step Over
              </Button>
              <Button className="w-full" disabled={!paused}>
                <ArrowDown className="mr-2 h-4 w-4" />
                Step Into
              </Button>
              <Button className="w-full" disabled={!paused}>
                <ArrowUp className="mr-2 h-4 w-4" />
                Step Out
              </Button>
              <Button className="w-full" disabled={!paused}>
                <Play className="mr-2 h-4 w-4" />
                Continue
              </Button>
              <Button variant="destructive" className="w-full">
                <Square className="mr-2 h-4 w-4" />
                Stop
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <Card>
          <CardHeader>
            <CardTitle>State Inspector</CardTitle>
          </CardHeader>
          <CardContent>
            <pre className="text-xs bg-slate-900 p-4 rounded overflow-auto max-h-96">
              {JSON.stringify(state, null, 2)}
            </pre>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Breakpoints</CardTitle>
          </CardHeader>
          <CardContent>
            <BreakpointsList breakpoints={breakpoints} onChange={setBreakpoints} />
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Console</CardTitle>
        </CardHeader>
        <CardContent>
          <ScrollArea className="h-64">
            <div className="font-mono text-xs space-y-1">
              <ConsoleOutput />
            </div>
          </ScrollArea>
        </CardContent>
      </Card>
    </div>
  )
}
```

---

## Navigation Structure

**Sidebar Navigation** (`src/components/Layout/Sidebar.tsx`):

```typescript
const navigation = [
  { name: 'Dashboard', href: '/', icon: LayoutDashboard },
  { name: 'Chat', href: '/chat', icon: MessageSquare },
  { name: 'Users', href: '/users', icon: Users },
  { name: 'VPN', href: '/vpn', icon: Shield },
  { name: 'Mail', href: '/mail', icon: Mail },
  { name: 'MCP Services', href: '/mcp', icon: Server },
  { name: 'Analytics', href: '/analytics', icon: BarChart },
  { name: 'Settings', href: '/settings', icon: Settings },

  // Orchestration section (with divider)
  { divider: true, label: 'Orchestration' },
  { name: 'Tools', href: '/tools', icon: Wrench },
  { name: 'Agents', href: '/agents', icon: Bot },
  { name: 'Workflows', href: '/workflows', icon: GitBranch },
  { name: 'Work Stacks', href: '/stacks', icon: Layers },
  { name: 'Control', href: '/orchestration', icon: Cpu },
  { name: 'Logs', href: '/logs', icon: FileText },
  { name: 'Debugger', href: '/debugger', icon: Bug },
]
```

---

## Shared Components

Create these reusable components in `src/components/`:

**MetricCard** (`src/components/MetricCard.tsx`):
```typescript
interface MetricCardProps {
  title: string
  value: number | string
  icon?: React.ComponentType
  trend?: number
  alert?: boolean
}

export function MetricCard({ title, value, icon: Icon, trend, alert }: MetricCardProps) {
  return (
    <Card className={alert ? 'border-red-500' : ''}>
      <CardHeader className="flex flex-row items-center justify-between pb-2">
        <CardTitle className="text-sm font-medium text-slate-400">{title}</CardTitle>
        {Icon && <Icon className="h-4 w-4 text-slate-400" />}
      </CardHeader>
      <CardContent>
        <div className="text-2xl font-bold">{value}</div>
        {trend !== undefined && (
          <p className={`text-xs ${trend > 0 ? 'text-green-500' : 'text-red-500'}`}>
            {trend > 0 ? '↑' : '↓'} {Math.abs(trend)}%
          </p>
        )}
      </CardContent>
    </Card>
  )
}
```

**GaugeChart** (`src/components/GaugeChart.tsx`):
```typescript
export function GaugeChart({ title, value }: { title: string, value: number }) {
  const rotation = (value / 100) * 180 - 90

  return (
    <div className="flex flex-col items-center">
      <div className="relative w-32 h-16">
        <svg viewBox="0 0 100 50" className="w-full h-full">
          <path
            d="M 10 45 A 40 40 0 0 1 90 45"
            fill="none"
            stroke="#334155"
            strokeWidth="8"
          />
          <path
            d="M 10 45 A 40 40 0 0 1 90 45"
            fill="none"
            stroke="#3b82f6"
            strokeWidth="8"
            strokeDasharray={`${(value / 100) * 125.6} 125.6`}
          />
          <line
            x1="50"
            y1="45"
            x2="80"
            y2="45"
            stroke="#3b82f6"
            strokeWidth="2"
            transform={`rotate(${rotation} 50 45)`}
          />
        </svg>
      </div>
      <p className="text-sm text-slate-400 mt-2">{title}</p>
      <p className="text-lg font-bold">{value}%</p>
    </div>
  )
}
```

**ActivityFeed** (`src/components/ActivityFeed.tsx`):
```typescript
export function ActivityFeed() {
  const [activities, setActivities] = useState<Activity[]>([])

  useEffect(() => {
    const ws = new WebSocket(`wss://${window.location.host}/ws`)
    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data)
      if (msg.type === 'activity') {
        setActivities(prev => [msg.data, ...prev].slice(0, 10))
      }
    }
    return () => ws.close()
  }, [])

  return (
    <ScrollArea className="h-72">
      <div className="space-y-2">
        {activities.map((activity, i) => (
          <div key={i} className="flex items-start gap-2 p-2 hover:bg-slate-800 rounded">
            <ActivityIcon type={activity.type} />
            <div className="flex-1">
              <p className="text-sm">{activity.message}</p>
              <p className="text-xs text-slate-400">{formatTime(activity.timestamp)}</p>
            </div>
          </div>
        ))}
      </div>
    </ScrollArea>
  )
}
```

---

## WebSocket Integration

**Global WebSocket Hook** (`src/hooks/useWebSocket.ts`):
```typescript
export function useWebSocket(onMessage: (msg: WSMessage) => void) {
  useEffect(() => {
    const ws = new WebSocket(`wss://${window.location.host}/ws`)

    ws.onopen = () => console.log('WebSocket connected')
    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data)
        onMessage(msg)
      } catch (err) {
        console.error('Failed to parse WS message:', err)
      }
    }
    ws.onerror = (err) => console.error('WebSocket error:', err)
    ws.onclose = () => console.log('WebSocket disconnected')

    return () => ws.close()
  }, [onMessage])
}
```

---

## TypeScript Types

**Core Types** (`src/types/index.ts`):
```typescript
export interface User {
  id: string
  email: string
  wireguard_ip: string
  wireguard_public_key: string
  status: 'active' | 'suspended'
  created_at: string
  last_seen?: string
}

export interface VPNStatus {
  running: boolean
  interface: string
  activeConnections: number
  peerCount: number
  bandwidth: { up: number, down: number }
}

export interface Connection {
  userId: string
  email: string
  ip: string
  connectedSince: string
  rx: number
  tx: number
  lastHandshake: string
}

export interface MailStatus {
  running: boolean
  smtpPort: number
  imapPort: number
  sentToday: number
}

export interface MailQueueItem {
  id: string
  from: string
  to: string
  subject: string
  status: 'queued' | 'sending' | 'failed'
  retryCount: number
  createdAt: string
}

export interface MCPService {
  id: string
  name: string
  description: string
  running: boolean
  connected: boolean
  tools: MCPTool[]
}

export interface MCPTool {
  name: string
  description: string
  inputSchema: object
}

export interface Tool {
  id: string
  name: string
  description: string
  category: string
  inputSchema: object
}

export interface Agent {
  id: string
  name: string
  type: string
  status: 'idle' | 'running' | 'error' | 'paused'
  currentTask?: string
  config: object
}

export interface Workflow {
  id: string
  name: string
  description: string
  steps: WorkflowStep[]
  lastRun?: string
  successRate: number
}

export interface WorkflowStep {
  id: string
  name: string
  type: string
  config: object
}

export interface WorkStack {
  id: string
  name: string
  tasks: Task[]
}

export interface Task {
  id: string
  name: string
  priority: number
  status: string
  assignedAgent?: string
  createdAt: string
}

export interface OrchestrationStatus {
  running: boolean
  currentPlan?: string
  progress: number
}

export interface LogEntry {
  id: string
  timestamp: string
  level: 'debug' | 'info' | 'warn' | 'error'
  component: string
  message: string
  tool?: string
  user?: string
  status?: 'success' | 'failed'
  duration?: number
}

export type WSMessage =
  | { type: 'vpn_status', data: VPNStatus }
  | { type: 'new_connection', data: Connection }
  | { type: 'disconnection', data: { userId: string } }
  | { type: 'log', data: LogEntry }
  | { type: 'chat_message', data: ChatMessage }
  | { type: 'tool_execution', data: ToolExecution }
  | { type: 'workflow_update', data: WorkflowUpdate }
  | { type: 'orchestration_event', data: OrchestrationEvent }
  | { type: 'activity', data: Activity }
```

---

## Routing

**App Router** (`src/App.tsx`):
```typescript
import { BrowserRouter, Routes, Route } from 'react-router-dom'
import Layout from './components/Layout'
import Dashboard from './pages/Dashboard'
import Chat from './pages/Chat'
// ... import all pages

function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<Dashboard />} />
          <Route path="chat" element={<Chat />} />
          <Route path="users" element={<Users />} />
          <Route path="vpn" element={<VPN />} />
          <Route path="mail" element={<Mail />} />
          <Route path="mcp" element={<MCP />} />
          <Route path="analytics" element={<Analytics />} />
          <Route path="settings" element={<Settings />} />
          <Route path="tools" element={<Tools />} />
          <Route path="agents" element={<Agents />} />
          <Route path="workflows" element={<Workflows />} />
          <Route path="stacks" element={<WorkStacks />} />
          <Route path="orchestration" element={<Orchestration />} />
          <Route path="logs" element={<Logs />} />
          <Route path="debugger" element={<Debugger />} />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}

export default App
```

---

## Implementation Notes

1. **Start with the layout**: Build `Layout.tsx` with sidebar navigation first
2. **Implement Dashboard**: This is the landing page and sets the design tone
3. **Add WebSocket support**: Critical for real-time updates across all pages
4. **Build pages incrementally**: Start with simpler pages (Users, VPN, Mail) before complex ones (Orchestration, Debugger)
5. **Use shadcn/ui components**: Already configured, just import and use
6. **Mock API data initially**: Can switch to real API once backend is ready
7. **Add error boundaries**: Wrap pages in error boundaries for graceful failures
8. **Implement loading states**: Show skeletons while data loads
9. **Add toast notifications**: For user feedback on actions
10. **Test responsiveness**: Ensure works on tablet and mobile

---

## Final Deliverable

A production-ready SPA dashboard with:
- 15 fully-functional pages
- Real-time updates via WebSocket
- Beautiful, consistent UI using shadcn/ui
- Type-safe TypeScript throughout
- Responsive design
- Comprehensive feature set covering all operation-dbus functionality

Copy this entire prompt into Lovable and it will build the complete dashboard!
