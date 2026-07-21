#!/usr/bin/env python3
"""
GhostBridge Session Manager
WireGuard key → Linux user → logind session → Compact MCP

This is the FOUNDATION - ties WireGuard identity to FreeDesktop sessions.
"""
import subprocess
import sys
import os
import json
from pathlib import Path

# Configuration
GHOSTBRIDGE_DIR = Path("/var/lib/ghostbridge")
SESSIONS_DIR = GHOSTBRIDGE_DIR / "sessions"
WORKSPACES_DIR = GHOSTBRIDGE_DIR / "workspaces"

def log(msg: str):
    """Log to journal"""
    print(f"[ghostbridge] {msg}", flush=True)

def get_user_for_key(pub_key: str) -> str:
    """Get or create Linux user for WireGuard key"""
    # Use first 12 chars of key as user suffix
    key_short = pub_key[:12].replace("/", "").replace("+", "")
    username = f"wg-{key_short}"
    
    # Check if user exists
    result = subprocess.run(['id', username], capture_output=True)
    if result.returncode != 0:
        log(f"Creating user {username}")
        subprocess.run([
            'useradd', '-m', '-s', '/bin/bash',
            '-c', f'GhostBridge user for WG key {key_short}',
            username
        ], check=True)
    
    return username

def get_session_for_user(username: str) -> str | None:
    """Check if user already has an active logind session"""
    result = subprocess.run(
        ['loginctl', 'list-sessions', '-o', 'json'],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        return None
    
    try:
        sessions = json.loads(result.stdout)
        for session in sessions:
            if session.get('user') == username:
                return session.get('session')
    except json.JSONDecodeError:
        pass
    return None

def create_session(pub_key: str) -> bool:
    """Create logind session for WireGuard peer"""
    username = get_user_for_key(pub_key)
    
    # Create workspace
    workspace = WORKSPACES_DIR / pub_key
    workspace.mkdir(parents=True, exist_ok=True)
    subprocess.run(['chown', '-R', f'{username}:{username}', str(workspace)], check=True)
    
    # Save session metadata
    session_file = SESSIONS_DIR / f"{pub_key}.json"
    session_file.parent.mkdir(parents=True, exist_ok=True)
    session_file.write_text(json.dumps({
        'pub_key': pub_key,
        'username': username,
        'workspace': str(workspace),
    }))
    
    # Enable lingering to keep user runtime dir and D-Bus session active
    log(f"Enabling linger for {username}")
    result = subprocess.run(
        ['loginctl', 'enable-linger', username],
        capture_output=True, text=True
    )
    
    if result.returncode != 0:
        log(f"Failed to enable linger for {username}: {result.stderr}")
        return False
    
    log(f"Session (finger) enabled for {username}")
    return True

def handle_connect(pub_key: str):
    """Handle peer connection"""
    log(f"Peer connected: {pub_key[:16]}...")
    return create_session(pub_key)

def handle_disconnect(pub_key: str):
    """Handle peer disconnection - keep session alive"""
    log(f"Peer disconnected: {pub_key[:16]}... (session persists)")

if __name__ == '__main__':
    if len(sys.argv) < 3:
        print("Usage: session-manager.py <connect|disconnect> <pub_key>")
        sys.exit(1)
    
    action = sys.argv[1]
    pub_key = sys.argv[2]
    
    if action == 'connect':
        success = handle_connect(pub_key)
        sys.exit(0 if success else 1)
    elif action == 'disconnect':
        handle_disconnect(pub_key)
        sys.exit(0)
    else:
        print(f"Unknown action: {action}")
        sys.exit(1)
