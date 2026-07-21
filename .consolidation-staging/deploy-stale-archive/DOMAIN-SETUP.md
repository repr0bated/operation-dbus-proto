# Public Domain Setup Guide

## Quick Setup (Recommended)

```bash
cd /home/jeremy/op-dbus-v2/deploy
./setup-public-domain.sh
```

This will:
1. Install Caddy or Nginx
2. Configure automatic HTTPS
3. Set up systemd service
4. Configure firewall
5. Start the service

## Prerequisites

### 1. Domain Name
- Register a domain (e.g., from Cloudflare, Namecheap, GoDaddy)
- Point DNS A record to your server's public IP

Check your public IP:
```bash
curl -4 ifconfig.me
```

### 2. DNS Configuration
Add these records at your DNS provider:

```
Type    Name    Value               TTL
A       chat    YOUR_SERVER_IP      300
A       @       YOUR_SERVER_IP      300  (if you want root domain)
```

Wait 5-10 minutes for DNS propagation. Verify:
```bash
dig chat.yourdomain.com
nslookup chat.yourdomain.com
```

### 3. Server Requirements
- Public IP address
- Ports 80 and 443 open (firewall/cloud security group)
- Root or sudo access

## Manual Setup Options

### Option A: Caddy (Easiest - Auto HTTPS)

#### Install Caddy
```bash
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https curl
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt update
sudo apt install -y caddy
```

#### Configure
```bash
# Edit Caddyfile
sudo nano /etc/caddy/Caddyfile

# Replace DOMAIN with your actual domain:
chat.yourdomain.com {
    reverse_proxy localhost:8080
}
```

#### Start
```bash
sudo systemctl restart caddy
sudo systemctl enable caddy
```

**That's it!** Caddy automatically gets Let's Encrypt certificates.

### Option B: Nginx + Certbot

#### Install
```bash
sudo apt update
sudo apt install -y nginx certbot python3-certbot-nginx
```

#### Configure
```bash
# Copy config
sudo cp /home/jeremy/op-dbus-v2/deploy/nginx/op-web.conf /etc/nginx/sites-available/

# Edit domain name
sudo nano /etc/nginx/sites-available/op-web.conf
# Replace all instances of "your-domain.com" with your actual domain

# Enable site
sudo ln -s /etc/nginx/sites-available/op-web.conf /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

#### Get SSL Certificate
```bash
sudo certbot --nginx -d chat.yourdomain.com
```

Certbot will:
- Verify domain ownership
- Get SSL certificate
- Auto-configure nginx
- Set up auto-renewal

### Option C: Cloudflare Tunnel (No Public IP Needed!)

If you can't open ports or don't have a static IP:

#### Install Cloudflared
```bash
curl -L https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64.deb -o cloudflared.deb
sudo dpkg -i cloudflared.deb
```

#### Authenticate
```bash
cloudflared tunnel login
```

#### Create Tunnel
```bash
cloudflared tunnel create op-web
cloudflared tunnel route dns op-web chat.yourdomain.com
```

#### Configure
```bash
mkdir -p ~/.cloudflared
cat > ~/.cloudflared/config.yml << EOF
tunnel: op-web
credentials-file: /home/jeremy/.cloudflared/<TUNNEL-ID>.json

ingress:
  - hostname: chat.yourdomain.com
    service: http://localhost:8080
  - service: http_status:404
EOF
```

#### Run as Service
```bash
sudo cloudflared service install
sudo systemctl start cloudflared
sudo systemctl enable cloudflared
```

## Systemd Service Setup

### Install Service
```bash
sudo cp /home/jeremy/op-dbus-v2/deploy/systemd/op-web.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable op-web
sudo systemctl start op-web
```

### Check Status
```bash
sudo systemctl status op-web
sudo journalctl -u op-web -f
```

### Service Commands
```bash
sudo systemctl start op-web      # Start
sudo systemctl stop op-web       # Stop
sudo systemctl restart op-web    # Restart
sudo systemctl status op-web     # Status
sudo journalctl -u op-web -f     # Live logs
```

## Security Considerations

### 1. Add Authentication (Recommended for Public)

#### Caddy Basic Auth
```bash
# Generate password hash
caddy hash-password

# Add to Caddyfile:
chat.yourdomain.com {
    basicauth /* {
        admin $2a$14$...your-hash-here...
    }
    reverse_proxy localhost:8080
}
```

#### Nginx Basic Auth
```bash
# Create password file
sudo apt install apache2-utils
sudo htpasswd -c /etc/nginx/.htpasswd admin

# Add to nginx config:
location / {
    auth_basic "Restricted Access";
    auth_basic_user_file /etc/nginx/.htpasswd;
    proxy_pass http://localhost:8080;
}
```

### 2. Firewall Setup

#### UFW
```bash
sudo ufw allow 22/tcp   # SSH (don't lock yourself out!)
sudo ufw allow 80/tcp   # HTTP
sudo ufw allow 443/tcp  # HTTPS
sudo ufw enable
```

#### Firewalld
```bash
sudo firewall-cmd --permanent --add-service=ssh
sudo firewall-cmd --permanent --add-service=http
sudo firewall-cmd --permanent --add-service=https
sudo firewall-cmd --reload
```

### 3. Rate Limiting

Already configured in nginx/caddy configs:
- API: 10 requests/second
- Chat: 5 requests/second
- Burst allowance for normal usage

### 4. SSL/TLS Security

Both configs use:
- TLS 1.2 and 1.3 only
- Strong cipher suites
- HSTS headers
- Security headers (XSS, frame options, etc.)

## Monitoring

### Health Check
```bash
curl https://chat.yourdomain.com/api/health
```

### Logs
```bash
# Application logs
sudo journalctl -u op-web -f

# Nginx logs
sudo tail -f /var/log/nginx/op-web-access.log
sudo tail -f /var/log/nginx/op-web-error.log

# Caddy logs
sudo journalctl -u caddy -f
sudo tail -f /var/log/caddy/op-web-access.log
```

## Troubleshooting

### Service won't start
```bash
# Check logs
sudo journalctl -u op-web -n 100

# Check if binary exists
ls -la /home/jeremy/op-dbus-v2/target/release/op-web-server

# Check permissions
sudo systemctl status op-web
```

### SSL certificate fails
```bash
# Check DNS
dig chat.yourdomain.com

# Test certbot
sudo certbot certificates
sudo certbot renew --dry-run

# Check port 80 is open
sudo netstat -tlnp | grep :80
```

### Can't connect to server
```bash
# Check firewall
sudo ufw status
sudo firewall-cmd --list-all

# Check nginx/caddy
sudo systemctl status nginx
sudo systemctl status caddy

# Check op-web
sudo systemctl status op-web
curl http://localhost:8080/api/health
```

### WebSocket not working
- Check proxy config has `Upgrade` and `Connection` headers
- Verify nginx/caddy is passing WebSocket traffic
- Check browser console for errors

## Architecture

```
Internet
   ↓
DNS (chat.yourdomain.com)
   ↓
Firewall (ports 80, 443)
   ↓
Reverse Proxy (Caddy/Nginx)
   ├── SSL/TLS termination
   ├── Rate limiting
   ├── Security headers
   └── WebSocket support
       ↓
   op-web-server (localhost:8080)
       ├── HTTP server
       ├── WebSocket handler
       ├── Chat orchestrator
       └── Tool registry
```

## Quick Commands Reference

```bash
# Setup (automated)
./deploy/setup-public-domain.sh

# Manual service control
sudo systemctl start op-web
sudo systemctl status op-web
sudo systemctl restart op-web
sudo journalctl -u op-web -f

# Web server control
sudo systemctl restart caddy  # or nginx
sudo systemctl status caddy   # or nginx

# Check if site is accessible
curl -I https://chat.yourdomain.com

# View SSL certificate
echo | openssl s_client -connect chat.yourdomain.com:443 2>/dev/null | openssl x509 -noout -text

# Test health endpoint
curl https://chat.yourdomain.com/api/health
```

## Cost-Free Options

### Free Domains
- Freenom (free .tk, .ml, .ga domains)
- DuckDNS (free subdomain)
- No-IP (free subdomain)

### Free SSL
- Let's Encrypt (automated with Caddy/Certbot)
- Cloudflare (automatic with Cloudflare Tunnel)

### Free Hosting
- Run on existing server
- Oracle Cloud free tier
- Google Cloud free tier
- AWS free tier (12 months)

## Next Steps

1. ✅ Run setup script
2. ✅ Verify DNS points to your server
3. ✅ Access https://chat.yourdomain.com/chat.html
4. ⚠️  Add authentication for public access
5. 📊 Set up monitoring/alerting
6. 🔐 Review security best practices
