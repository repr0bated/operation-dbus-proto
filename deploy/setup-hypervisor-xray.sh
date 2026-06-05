#!/bin/bash
# Install Xray natively on the hypervisor (Artix Linux/s6)
# Binds to eth0 and proxies to internal container

set -euo pipefail

echo "=== Installing Xray Server on Hypervisor ==="

# 0. Get external IP dynamically (supports eth0 or ovsbr0 AF_XDP architectures)
EXT_IP=$(ip -4 route get 8.8.8.8 | grep -oP 'src \K\S+')
if [ -z "$EXT_IP" ]; then
    echo "Could not detect external IP"
    exit 1
fi
echo "Detected external IP: $EXT_IP"

# 1. Download and install Xray
echo "Downloading Xray-linux-64..."
curl -L https://github.com/XTLS/Xray-core/releases/latest/download/Xray-linux-64.zip -o /tmp/xray.zip
unzip -o /tmp/xray.zip -d /tmp/xray
install -m 755 /tmp/xray/xray /usr/local/bin/xray
rm -rf /tmp/xray*

# 2. Setup Xray config directory
mkdir -p /etc/xray

# Create the full server config matching production architecture
cat > /etc/xray/config.json << EOF
{
  "log": {
    "loglevel": "warning",
    "access": "/var/log/xray/access.log",
    "error": "/var/log/xray/error.log"
  },
  "inbounds": [
    {
      "tag": "op-web-tls",
      "listen": "\${EXT_IP}",
      "port": 443,
      "protocol": "vless",
      "settings": {
        "clients": [
          { "id": "f997ea91-94bb-4078-b172-649cd21a7647" }
        ],
        "decryption": "none",
        "fallbacks": [
          { "dest": "10.200.0.1:8080" }
        ]
      },
      "streamSettings": {
        "network": "tcp",
        "security": "tls",
        "tlsSettings": {
          "alpn": ["http/1.1"],
          "certificates": [
            {
              "certificateFile": "/etc/xray/tls.crt",
              "keyFile": "/etc/xray/tls.key"
            }
          ]
        }
      }
    },
    {
      "tag": "op-grpc-tls",
      "listen": "\${EXT_IP}",
      "port": 50051,
      "protocol": "vless",
      "settings": {
        "clients": [
          { "id": "f997ea91-94bb-4078-b172-649cd21a7647" }
        ],
        "decryption": "none",
        "fallbacks": [
          { "dest": "10.200.0.1:50051" }
        ]
      },
      "streamSettings": {
        "network": "tcp",
        "security": "tls",
        "tlsSettings": {
          "alpn": ["h2"],
          "certificates": [
            {
              "certificateFile": "/etc/xray/tls.crt",
              "keyFile": "/etc/xray/tls.key"
            }
          ]
        }
      }
    },
    {
      "tag": "ghostbridge-reality",
      "listen": "\${EXT_IP}",
      "port": 8443,
      "protocol": "vless",
      "settings": {
        "clients": [
          {
            "id": "f997ea91-94bb-4078-b172-649cd21a7647",
            "flow": "xtls-rprx-vision"
          }
        ],
        "decryption": "none",
        "fallbacks": [
          { "dest": "10.200.0.1:8080" }
        ]
      },
      "streamSettings": {
        "network": "tcp",
        "security": "reality",
        "realitySettings": {
          "dest": "www.microsoft.com:443",
          "serverNames": ["www.microsoft.com"],
          "privateKey": "${XRAY_PRIVATE_KEY}",
          "shortIds": ["821c52c7ba699c33"]
        }
      },
      "sniffing": {
        "enabled": true,
        "destOverride": ["http", "tls", "quic"],
        "routeOnly": true
      }
    }
  ],
  "outbounds": [
    { "tag": "direct", "protocol": "freedom", "settings": {} },
    { "tag": "block", "protocol": "blackhole" }
  ]
}
EOF

# 3. Setup s6 service
mkdir -p /etc/s6/sv/xray
cat > /etc/s6/sv/xray/run << 'EOF'
#!/bin/execlineb -P
fdmove -c 2 1
/usr/local/bin/xray run -c /etc/xray/config.json
EOF
chmod +x /etc/s6/sv/xray/run

# Link to scan directory if s6 is active
if [ -d /run/service ]; then
    ln -sf /etc/s6/sv/xray /run/service/xray
    s6-svscanctl -a /run/service
    echo "Started Xray via s6."
fi

echo "=== Xray Hypervisor Installation Complete ==="
