# Proxmox Server Integration Guide

## Overview

Your setup:
- **Proxmox Web UI**: https://proxmox.ghostbridge.tech:8006 (port 8006)
- **Chat Server**: https://proxmox.ghostbridge.tech/chat/ (ports 80/443)
- **IP Address**: 80.209.240.244

We'll install Nginx alongside Proxmox's `pveproxy` to serve the chat interface on standard HTTPS port (443) while keeping Proxmox on its dedicated port (8006).

## Quick Setup (Automated)

```bash
cd /home/jeremy/op-dbus-v2/deploy
./setup-with-existing-proxmox.sh
```

This will:
1. ✅ Install Nginx
2. ✅ Copy existing Proxmox SSL certificates
3. ✅ Configure reverse proxy on port 443
4. ✅ Set up systemd service for op-web
5. ✅ Configure firewall
6. ✅ Start all services

## What Happens

### Before:
```
Port 8006 (HTTPS) → Proxmox Web UI
Port 8080 (HTTP)  → (unused)
Ports 80/443      → (unused)
```

### After:
```
Port 8006 (HTTPS) → Proxmox Web UI (unchanged)
Port 8080 (HTTP)  → op-web backend (localhost only)
Port 443 (HTTPS)  → Nginx → /chat/ → op-web backend
Port 80 (HTTP)    → Nginx → redirect to HTTPS
```

## Manual Setup

### 1. Install Nginx

```bash
sudo apt update
sudo apt install -y nginx
```

### 2. Copy SSL Certificates

```bash
# Create nginx SSL directory
sudo mkdir -p /etc/nginx/ssl

# Copy Proxmox certificates
sudo cp /etc/pve/nodes/proxmox/pve-ssl.pem /etc/nginx/ssl/ghostbridge.crt
sudo cp /etc/pve/nodes/proxmox/pve-ssl.key /etc/nginx/ssl/ghostbridge.key

# Set permissions
sudo chmod 600 /etc/nginx/ssl/ghostbridge.key
sudo chmod 644 /etc/nginx/ssl/ghostbridge.crt
```

### 3. Configure Nginx

```bash
sudo nano /etc/nginx/sites-available/op-web
```

Use the configuration from the script (see `setup-with-existing-proxmox.sh`).

### 4. Enable Site

```bash
sudo ln -s /etc/nginx/sites-available/op-web /etc/nginx/sites-enabled/
sudo rm -f /etc/nginx/sites-enabled/default
sudo nginx -t
sudo systemctl restart nginx
sudo systemctl enable nginx
```

### 5. Start op-web Service

```bash
# Create environment file
cat > ~/.op-web.env << EOF
HF_TOKEN=${HF_TOKEN}
GITHUB_PERSONAL_ACCESS_TOKEN=${GH_TOKEN}
MCP_CONFIG_FILE=/home/jeremy/op-dbus-v2/crates/op-mcp/mcp-config.json
EOF
chmod 600 ~/.op-web.env

# Install and start service
sudo cp /home/jeremy/op-dbus-v2/deploy/systemd/op-web.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable op-web
sudo systemctl start op-web
```

### 6. Configure Firewall

```bash
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw allow 8006/tcp  # Keep Proxmox accessible
```

## Access Your Services

### Chat Interface
```
https://proxmox.ghostbridge.tech/chat/
https://proxmox.ghostbridge.tech/chat/chat.html
```

### Proxmox (Unchanged)
```
https://proxmox.ghostbridge.tech:8006
```

### API Health Check
```bash
curl https://proxmox.ghostbridge.tech/chat/api/health
```

## SSL Certificate Options

### Option 1: Use Existing Proxmox Certificate (Default)
The setup script copies your existing Proxmox SSL certificate. This works but may show browser warnings if it's self-signed.

### Option 2: Get Proper Let's Encrypt Certificate (Recommended)
```bash
# Install certbot
sudo apt install -y certbot python3-certbot-nginx

# Get certificate (automatic nginx configuration)
sudo certbot --nginx -d proxmox.ghostbridge.tech

# Auto-renewal is set up automatically
sudo certbot renew --dry-run
```

**Note**: This will replace the certificate for the main domain. Proxmox on port 8006 will continue using its own certificate.

### Option 3: Separate Subdomain (Best Practice)
If you control DNS, create a subdomain:

```bash
# Add DNS record:
# Type: A
# Name: chat
# Value: 80.209.240.244

# Get certificate for subdomain
sudo certbot --nginx -d chat.ghostbridge.tech
```

Then update nginx config to use `chat.ghostbridge.tech` instead of `proxmox.ghostbridge.tech`.

## Troubleshooting

### Port Conflicts

Check what's using ports:
```bash
sudo ss -tlnp | grep -E ":80|:443|:8006|:8080"
```

Should see:
- Port 8006: pveproxy (Proxmox)
- Port 8080: op-web-server (localhost only)
- Port 443: nginx
- Port 80: nginx

### Services Not Starting

```bash
# Check op-web
sudo systemctl status op-web
sudo journalctl -u op-web -n 50

# Check nginx
sudo systemctl status nginx
sudo nginx -t
sudo tail -f /var/log/nginx/error.log

# Check if backend is responding
curl http://localhost:8080/api/health
```

### SSL Certificate Issues

```bash
# Check certificate
sudo openssl x509 -in /etc/nginx/ssl/ghostbridge.crt -noout -text

# Test SSL
echo | openssl s_client -connect proxmox.ghostbridge.tech:443 2>/dev/null | openssl x509 -noout -dates

# If using Let's Encrypt
sudo certbot certificates
sudo certbot renew --dry-run
```

### 502 Bad Gateway

This means nginx can't reach the backend:

```bash
# Check if op-web is running
sudo systemctl status op-web

# Check if it's listening on 8080
curl http://localhost:8080/api/health

# Check nginx error log
sudo tail -f /var/log/nginx/op-web-error.log
```

### Can't Access from Internet

```bash
# Check firewall
sudo ufw status

# Check if nginx is listening on public interface
sudo ss -tlnp | grep :443

# Test from server
curl -I https://localhost/chat/

# Check DNS
dig proxmox.ghostbridge.tech
```

## Service Management

```bash
# op-web service
sudo systemctl start op-web
sudo systemctl stop op-web
sudo systemctl restart op-web
sudo systemctl status op-web
sudo journalctl -u op-web -f

# nginx
sudo systemctl start nginx
sudo systemctl stop nginx
sudo systemctl restart nginx
sudo systemctl reload nginx  # Reload config without downtime
sudo systemctl status nginx
sudo tail -f /var/log/nginx/op-web-access.log

# Both services
sudo systemctl restart op-web nginx
```

## Configuration Files

```
/etc/nginx/sites-available/op-web     - Nginx config
/etc/nginx/ssl/ghostbridge.crt        - SSL certificate
/etc/nginx/ssl/ghostbridge.key        - SSL private key
/etc/systemd/system/op-web.service    - Systemd service
~/.op-web.env                         - Environment variables
/var/log/nginx/op-web-access.log      - Access logs
/var/log/nginx/op-web-error.log       - Error logs
```

## Security Notes

1. **Existing Proxmox SSL**: The script copies your Proxmox certificate. This is fine for internal use but may show browser warnings if self-signed.

2. **Get Proper Certificate**: Use Let's Encrypt (free) for a trusted certificate:
   ```bash
   sudo certbot --nginx -d proxmox.ghostbridge.tech
   ```

3. **Separate Services**: Proxmox and chat run independently. If one fails, the other continues working.

4. **Firewall**: Make sure only necessary ports are open:
   - 80/443: Public (chat server)
   - 8006: Proxmox (consider restricting to VPN/trusted IPs)
   - 22: SSH (restrict to trusted IPs)

5. **Add Authentication**: For public access, add basic auth to nginx:
   ```bash
   sudo apt install apache2-utils
   sudo htpasswd -c /etc/nginx/.htpasswd admin
   
   # Add to nginx config:
   location /chat/ {
       auth_basic "Restricted Access";
       auth_basic_user_file /etc/nginx/.htpasswd;
       # ... rest of config
   }
   ```

## Architecture Diagram

```
Internet
   ↓
DNS (proxmox.ghostbridge.tech)
   ↓
80.209.240.244
   ↓
Firewall (ports 80, 443, 8006)
   ↓
   ├─→ Port 8006 → pveproxy → Proxmox Web UI
   │
   └─→ Port 443 → Nginx (SSL termination)
       │
       ├─→ /chat/ → localhost:8080 → op-web-server
       │
       └─→ Port 80 → redirect to HTTPS
```

## Monitoring

```bash
# Check service status
sudo systemctl status op-web nginx

# Watch logs
sudo journalctl -u op-web -f

# Nginx access logs
sudo tail -f /var/log/nginx/op-web-access.log

# Check connections
watch -n 1 'ss -t | grep -E ":8080|:443"'

# Test endpoints
watch -n 5 'curl -s https://proxmox.ghostbridge.tech/chat/api/health | jq'
```

## Next Steps

After setup:

1. ✅ Test chat interface: https://proxmox.ghostbridge.tech/chat/
2. 🔐 Consider getting Let's Encrypt certificate
3. 🔒 Add authentication for public access
4. 📊 Set up monitoring/alerting
5. 🤖 Use chat interface to complete remaining tool registration
