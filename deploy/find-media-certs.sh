#!/bin/bash
# Find TLS certificates in /media/ directories
# Useful for recovering certificates from mounted drives

echo "=== Searching for TLS certificates in /media/ ==="
echo ""

found=0

for media_dir in /media/*; do
    if [ -d "$media_dir" ]; then
        echo "Checking: $media_dir"
        
        # Find certificate files
        certs=$(find "$media_dir" -type f \( \
            -name "*.crt" -o \
            -name "*.pem" -o \
            -name "*.cert" -o \
            -name "fullchain.pem" -o \
            -name "certificate.crt" -o \
            -name "ssl.crt" \
        \) 2>/dev/null)
        
        # Find key files
        keys=$(find "$media_dir" -type f \( \
            -name "*.key" -o \
            -name "privkey.pem" -o \
            -name "private.key" -o \
            -name "ssl.key" \
        \) 2>/dev/null)
        
        if [ -n "$certs" ]; then
            echo "  Certificates found:"
            echo "$certs" | while read -r cert; do
                echo "    - $cert"
                # Try to show certificate info
                if openssl x509 -in "$cert" -noout -subject 2>/dev/null; then
                    subject=$(openssl x509 -in "$cert" -noout -subject 2>/dev/null | sed 's/subject=/      Subject: /')
                    expiry=$(openssl x509 -in "$cert" -noout -enddate 2>/dev/null | sed 's/notAfter=/      Expires: /')
                    echo "$subject"
                    echo "$expiry"
                fi
            done
            found=1
        fi
        
        if [ -n "$keys" ]; then
            echo "  Keys found:"
            echo "$keys" | while read -r key; do
                echo "    - $key"
            done
            found=1
        fi
        
        echo ""
    fi
done

if [ $found -eq 0 ]; then
    echo "No certificates or keys found in /media/"
    echo ""
    echo "To mount a drive:"
    echo "  sudo mount /dev/sdX1 /media/backup"
    echo ""
    echo "Common certificate locations on mounted drives:"
    echo "  - /media/*/etc/nginx/ssl/"
    echo "  - /media/*/etc/letsencrypt/live/*/"
    echo "  - /media/*/etc/ssl/"
    echo "  - /media/*/backup/ssl/"
fi
