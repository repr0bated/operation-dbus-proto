# Op-Web Server Status

## ✅ Current Status

**op-web is running** on port 8080 (PID 477, running as root since Feb 20)

### Test Results

```bash
# Status endpoint works
curl http://localhost:8080/api/privacy/status
# Returns: {"available":true,...}

# Signup endpoint works
curl -X POST http://localhost:8080/api/privacy/signup \
  -H 'Content-Type: application/json' \
  -d '{"email": "test@3tched.com"}'
# Returns: {"success":true,"message":"Check your email for the login link"}
```

## ⚠️ Mail Configuration Status

The running op-web instance may not have the new mail server configured. It was started on Feb 20, before we set up the Maddy mail server.

### To Configure Mail Server

**Option 1: Restart op-web with mail config (requires root)**

```bash
# Stop current op-web (as root)
doas pkill -f op-web-server

# Start with mail configuration
export SMTP_HOST="10.149.181.121"
export SMTP_PORT="587"
export SMTP_USER="jeremy@3tched.com"
export SMTP_PASS="jeremy123"
export SMTP_FROM_EMAIL="noreply@3tched.com"
export SMTP_FROM_NAME="Operation DBUS"
export BASE_URL="http://3tched.com"

doas ./target/release/op-web-server
```

**Option 2: Use systemd service**

Create `/etc/systemd/system/op-web.service`:

```ini
[Unit]
Description=Operation DBUS Web Server
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/home/jeremy/git/operation-dbus
Environment="SMTP_HOST=10.149.181.121"
Environment="SMTP_PORT=587"
Environment="SMTP_USER=jeremy@3tched.com"
Environment="SMTP_PASS=jeremy123"
Environment="SMTP_FROM_EMAIL=noreply@3tched.com"
Environment="SMTP_FROM_NAME=Operation DBUS"
Environment="BASE_URL=http://3tched.com"
Environment="PORT=8080"
ExecStart=/home/jeremy/git/operation-dbus/target/release/op-web-server
Restart=always

[Install]
WantedBy=multi-user.target
```

Then:
```bash
doas systemctl daemon-reload
doas systemctl enable op-web
doas systemctl restart op-web
```

## Registration URLs

Once mail is configured:

- **Signup**: `POST http://3tched.com/api/privacy/signup`
- **Verify**: `GET http://3tched.com/api/privacy/verify?token=XXX`
- **Status**: `GET http://3tched.com/api/privacy/status`
- **Google OAuth**: `GET http://3tched.com/api/privacy/google/auth`

## Test Magic Link Flow

```bash
# 1. Register
curl -X POST http://localhost:8080/api/privacy/signup \
  -H 'Content-Type: application/json' \
  -d '{"email": "jeremy@3tched.com"}'

# 2. Check webmail for magic link
open http://10.149.181.121

# 3. Click link or use token directly
curl "http://localhost:8080/api/privacy/verify?token=TOKEN_FROM_EMAIL"
```

## Current Server Info

- **Running**: Yes (PID 477)
- **Port**: 8080
- **Started**: Feb 20
- **User**: root
- **Mail configured**: ⚠️ Unknown (needs verification)
