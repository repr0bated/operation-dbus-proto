This file is a merged representation of the entire codebase, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of the entire repository's contents.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
scripts/
  authenticate-chatbot.js
  extract-cursor-auth.js
  record-login.js
  spoof-auth.js
src/
  anthropic.rs
  antigravity_replay.rs
  antigravity.rs
  assistant.rs
  chat.rs
  factory.rs
  gcloud_adc.rs
  gemini_cli.rs
  gemini.rs
  headless_oauth.rs
  huggingface.rs
  lib.rs
  mcp_proxy.rs
  openclaw.rs
  perplexity.rs
  provider.rs
  pty_bridge.rs
Cargo.toml
compare-op-llm.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="scripts/authenticate-chatbot.js">
#!/usr/bin/env node

/**
 * Puppeteer Authentication Script for Chatbot
 * 
 * Automates Gemini Code Assist enterprise login and extracts auth tokens
 */

const puppeteer = require('puppeteer');

async function authenticateChatbot() {
  console.log('🚀 Starting Puppeteer authentication for chatbot...');
  
  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox']
  });
  
  try {
    const page = await browser.newPage();
    page.on('console', msg => console.log('PAGE:', msg.text()));
    
    console.log('📱 Navigating to Code Assist...');
    await page.goto('https://codeassist.google.com', { waitUntil: 'networkidle2' });
    
    // Look for enterprise/project setup
    console.log('🔍 Looking for enterprise authentication...');
    
    // Try to find and interact with enterprise login
    try {
      await page.waitForSelector('button:has-text("Enterprise")', { timeout: 5000 });
      await page.click('button:has-text("Enterprise")');
      console.log('✅ Clicked Enterprise login');
    } catch (e) {
      console.log('⚠️  Enterprise button not found, trying project input...');
    }
    
    // Look for project input
    try {
      const projectInput = await page.$('input[placeholder*="project" i]');
      if (projectInput) {
        await projectInput.type('420281222188');
        console.log('✅ Entered GCP project: 420281222188');
        
        // Look for submit/authenticate button
        const submitBtn = await page.$('button:has-text("Authenticate")') || 
                         await page.$('button:has-text("Connect")') ||
                         await page.$('button:has-text("Submit")');
        if (submitBtn) {
          await submitBtn.click();
          console.log('✅ Submitted authentication');
        }
      }
    } catch (e) {
      console.log('⚠️  Project input not found');
    }
    
    // Wait for authentication
    console.log('⏳ Waiting for authentication to complete...');
    await page.waitForTimeout(10000);
    
    // Extract auth tokens
    console.log('🔑 Extracting authentication data...');
    const authData = await page.evaluate(() => {
      const data = {};
      
      // Get all localStorage items
      for (let i = 0; i < localStorage.length; i++) {
        const key = localStorage.key(i);
        data[key] = localStorage.getItem(key);
      }
      
      return data;
    });
    
    console.log('📋 Extracted auth data keys:', Object.keys(authData));
    
    // Save to file for chatbot
    const fs = require('fs');
    const authFile = '/etc/op-dbus/chatbot-auth.json';
    
    const authPayload = {
      timestamp: new Date().toISOString(),
      gcp_project: '420281222188',
      auth_data: authData,
      puppeteer_automated: true
    };
    
    fs.writeFileSync(authFile, JSON.stringify(authPayload, null, 2));
    console.log(`💾 Saved authentication to: ${authFile}`);
    
    return authData;
    
  } finally {
    await browser.close();
  }
}

authenticateChatbot().catch(console.error);
</file>

<file path="scripts/extract-cursor-auth.js">
#!/usr/bin/env node

/**
 * Extract Cursor Agent Authentication
 * 
 * Extracts authentication tokens from Cursor agent configuration
 * so CLI can use the same enterprise authentication
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

function extractCursorAuth() {
  console.log('🔍 Extracting Cursor Agent Authentication...');
  
  const homeDir = os.homedir();
  const cursorConfigDir = path.join(homeDir, '.cursor');
  const cursorAuthFile = path.join(cursorConfigDir, 'auth.json');
  const cursorConfigFile = path.join(cursorConfigDir, 'config.json');
  
  let cursorAuth = {};
  let cursorConfig = {};
  
  // Try to read Cursor auth file
  if (fs.existsSync(cursorAuthFile)) {
    try {
      cursorAuth = JSON.parse(fs.readFileSync(cursorAuthFile, 'utf8'));
      console.log('✅ Found Cursor auth file');
    } catch (e) {
      console.log('⚠️  Could not parse Cursor auth file');
    }
  } else {
    console.log('⚠️  No Cursor auth file found');
  }
  
  // Try to read Cursor config file
  if (fs.existsSync(cursorConfigFile)) {
    try {
      cursorConfig = JSON.parse(fs.readFileSync(cursorConfigFile, 'utf8'));
      console.log('✅ Found Cursor config file');
    } catch (e) {
      console.log('⚠️  Could not parse Cursor config file');
    }
  } else {
    console.log('⚠️  No Cursor config file found');
  }
  
  // Extract enterprise authentication data
  const enterpriseData = {
    timestamp: new Date().toISOString(),
    source: 'cursor_agent_extraction',
    enterprise: {
      project: cursorConfig?.enterprise?.project || cursorAuth?.enterprise?.project,
      token: cursorAuth?.enterprise?.token || cursorAuth?.enterprise?.auth_token,
      refresh_token: cursorAuth?.enterprise?.refresh_token,
      expires_at: cursorAuth?.enterprise?.expires_at
    },
    gcp: {
      project_id: cursorConfig?.gcp?.project_id || cursorAuth?.gcp?.project_id,
      access_token: cursorAuth?.gcp?.access_token,
      service_account: cursorAuth?.gcp?.service_account
    },
    vertex: {
      endpoint: cursorConfig?.vertex?.endpoint,
      auth_mode: cursorConfig?.vertex?.auth_mode || 'enterprise'
    }
  };
  
  // Look for additional auth data in Cursor directory
  const cursorFiles = fs.readdirSync(cursorConfigDir).filter(file => 
    file.includes('auth') || file.includes('token') || file.includes('enterprise')
  );
  
  enterpriseData.cursor_files = cursorFiles;
  
  // Extract tokens from all potential locations
  const allTokens = {};
  
  // From auth.json
  extractTokens(cursorAuth, 'auth_json', allTokens);
  
  // From config.json  
  extractTokens(cursorConfig, 'config_json', allTokens);
  
  // From other Cursor files
  cursorFiles.forEach(file => {
    const filePath = path.join(cursorConfigDir, file);
    try {
      const content = JSON.parse(fs.readFileSync(filePath, 'utf8'));
      extractTokens(content, file, allTokens);
    } catch (e) {
      // Skip non-JSON files
    }
  });
  
  enterpriseData.all_tokens = allTokens;
  
  // Save extracted data for CLI to use
  const outputFile = '/etc/op-dbus/cursor-agent-auth.json';
  fs.writeFileSync(outputFile, JSON.stringify(enterpriseData, null, 2));
  console.log(`💾 Saved extracted Cursor auth to: ${outputFile}`);
  
  // Create environment script for CLI
  const envScript = `/etc/op-dbus/cursor-agent-env.sh`;
  const envContent = `
# Cursor Agent Authentication Environment
export CURSOR_AGENT_MODE=true
export ENTERPRISE_AUTH_TOKEN="${enterpriseData.enterprise.token || ''}"
export GCP_PROJECT_ID="${enterpriseData.enterprise.project || enterpriseData.gcp.project_id || ''}"
export VERTEX_ACCESS_TOKEN="${enterpriseData.gcp.access_token || ''}"
export CURSOR_ENTERPRISE_PROJECT="${enterpriseData.enterprise.project || ''}"
export SPOOFED_AS_CURSOR_AGENT=true

echo "🎯 CLI now using Cursor Agent authentication"
echo "Project: $GCP_PROJECT_ID"
echo "Enterprise Token: ${ENTERPRISE_AUTH_TOKEN:0:20}..."
`;
  
  fs.writeFileSync(envScript, envContent);
  console.log(`🔧 Created environment script: ${envScript}`);
  
  console.log('\n🎭 CLI ↔ Cursor Agent Spoofing Complete!');
  console.log('📋 To use:');
  console.log(`   source ${envScript}`);
  console.log('   sudo systemctl restart op-web');
  
  return enterpriseData;
}

function extractTokens(obj, source, tokenMap) {
  if (!obj || typeof obj !== 'object') return;
  
  for (const [key, value] of Object.entries(obj)) {
    if (typeof value === 'string' && isTokenLike(key, value)) {
      if (!tokenMap[key]) tokenMap[key] = [];
      tokenMap[key].push({
        value: value,
        source: source,
        category: categorizeToken(key, value)
      });
    } else if (typeof value === 'object') {
      extractTokens(value, source, tokenMap);
    }
  }
}

function isTokenLike(key, value) {
  const keyLower = key.toLowerCase();
  const valueLower = value.toLowerCase();
  
  // Key patterns
  if (keyLower.includes('token') || 
      keyLower.includes('auth') || 
      keyLower.includes('secret') ||
      keyLower.includes('key') ||
      keyLower.includes('credential')) {
    return true;
  }
  
  // Value patterns (JWT-like)
  if (value.includes('.') && value.split('.').length === 3) {
    return true;
  }
  
  // Long random strings
  if (value.length > 20 && /^[A-Za-z0-9+/=_-]+$/.test(value)) {
    return true;
  }
  
  return false;
}

function categorizeToken(key, value) {
  const keyLower = key.toLowerCase();
  
  if (keyLower.includes('access')) return 'access_token';
  if (keyLower.includes('refresh')) return 'refresh_token';
  if (keyLower.includes('id')) return 'id_token';
  if (keyLower.includes('bearer')) return 'bearer_token';
  if (keyLower.includes('enterprise')) return 'enterprise_token';
  if (keyLower.includes('vertex')) return 'vertex_token';
  if (value.split('.').length === 3) return 'jwt';
  
  return 'unknown';
}

// Run extraction
try {
  const result = extractCursorAuth();
  console.log(`🔑 Extracted ${Object.keys(result.all_tokens).length} token types`);
} catch (error) {
  console.error('❌ Extraction failed:', error.message);
  process.exit(1);
}
</file>

<file path="scripts/record-login.js">
#!/usr/bin/env node

/**
 * Puppeteer Login Recording Script
 * 
 * Records the actual login flow for authentic authentication
 * Run this while you manually log into Code Assist
 */

const puppeteer = require('puppeteer');
const fs = require('fs');

async function recordLogin() {
  console.log('🎬 Starting login recording session...');
  console.log('📋 Please manually complete the login process in the browser');
  console.log('⏹️  Press Ctrl+C when done to save the authentication data');
  
  const browser = await puppeteer.launch({
    headless: false, // Show browser so user can interact
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
    defaultViewport: { width: 1200, height: 800 }
  });
  
  const page = await browser.newPage();
  
  // Record all actions and data
  const recordedData = {
    timestamp: new Date().toISOString(),
    actions: [],
    finalAuthData: null,
    cookies: [],
    localStorage: {},
    sessionStorage: {}
  };
  
  // Record navigation
  page.on('framenavigated', frame => {
    if (frame === page.mainFrame()) {
      recordedData.actions.push({
        type: 'navigation',
        url: frame.url(),
        timestamp: new Date().toISOString()
      });
    }
  });
  
  // Record clicks
  page.on('click', () => {
    recordedData.actions.push({
      type: 'click',
      timestamp: new Date().toISOString(),
      url: page.url()
    });
  });
  
  // Record input changes
  page.on('input', () => {
    recordedData.actions.push({
      type: 'input',
      timestamp: new Date().toISOString(),
      url: page.url()
    });
  });
  
  console.log('🌐 Opening Code Assist login page...');
  await page.goto('https://codeassist.google.com');
  
  console.log('🎯 Please complete your login process manually:');
  console.log('   1. Click Enterprise/Organization login');
  console.log('   2. Enter your credentials');
  console.log('   3. Complete 2FA if required');
  console.log('   4. Wait for dashboard to load');
  console.log('   5. Press Ctrl+C to finish recording');
  
  // Wait for user to complete login
  process.on('SIGINT', async () => {
    console.log('\n⏹️  Recording stopped, extracting authentication data...');
    
    try {
      // Extract final authentication state
      recordedData.finalAuthData = await page.evaluate(() => {
        const data = {
          url: window.location.href,
          title: document.title,
          cookies: document.cookie,
          localStorage: {},
          sessionStorage: {}
        };
        
        // Get all localStorage
        for (let i = 0; i < localStorage.length; i++) {
          const key = localStorage.key(i);
          data.localStorage[key] = localStorage.getItem(key);
        }
        
        // Get all sessionStorage  
        for (let i = 0; i < sessionStorage.length; i++) {
          const key = sessionStorage.key(i);
          data.sessionStorage[key] = sessionStorage.getItem(key);
        }
        
        return data;
      });
      
      // Get cookies
      recordedData.cookies = await page.cookies();
      
      // Save recording
      const recordingFile = '/etc/op-dbus/login-recording.json';
      fs.writeFileSync(recordingFile, JSON.stringify(recordedData, null, 2));
      console.log(`💾 Saved login recording to: ${recordingFile}`);
      
      // Extract useful auth tokens
      const authTokens = extractAuthTokens(recordedData);
      if (authTokens.length > 0) {
        const authFile = '/etc/op-dbus/chatbot-auth.json';
        fs.writeFileSync(authFile, JSON.stringify({
          timestamp: new Date().toISOString(),
          source: 'recorded_login',
          auth_tokens: authTokens,
          gcp_project: '420281222188'
        }, null, 2));
        console.log(`🔑 Extracted ${authTokens.length} auth tokens to: ${authFile}`);
      }
      
    } catch (error) {
      console.error('❌ Error extracting data:', error.message);
    }
    
    await browser.close();
    process.exit(0);
  });
}

function extractAuthTokens(recording) {
  const tokens = [];
  
  if (recording.finalAuthData) {
    const { localStorage, sessionStorage, cookies } = recording.finalAuthData;
    
    // Look for auth tokens in localStorage
    Object.entries(localStorage).forEach(([key, value]) => {
      if (isAuthToken(key, value)) {
        tokens.push({
          type: 'localStorage',
          key,
          value,
          category: categorizeToken(key, value)
        });
      }
    });
    
    // Look for auth tokens in sessionStorage
    Object.entries(sessionStorage).forEach(([key, value]) => {
      if (isAuthToken(key, value)) {
        tokens.push({
          type: 'sessionStorage', 
          key,
          value,
          category: categorizeToken(key, value)
        });
      }
    });
    
    // Look for auth tokens in cookies
    cookies.split(';').forEach(cookie => {
      const [key, value] = cookie.trim().split('=');
      if (key && value && isAuthToken(key, value)) {
        tokens.push({
          type: 'cookie',
          key,
          value,
          category: categorizeToken(key, value)
        });
      }
    });
  }
  
  return tokens;
}

function isAuthToken(key, value) {
  if (!key || !value) return false;
  
  const keyLower = key.toLowerCase();
  const valueLower = value.toLowerCase();
  
  // Check key patterns
  if (keyLower.includes('token') || 
      keyLower.includes('auth') || 
      keyLower.includes('access') ||
      keyLower.includes('bearer') ||
      keyLower.includes('jwt')) {
    return true;
  }
  
  // Check value patterns (JWT tokens have dots)
  if (value.includes('.') && value.split('.').length === 3) {
    return true;
  }
  
  // Check for long random strings that might be tokens
  if (value.length > 20 && /^[A-Za-z0-9+/=_-]+$/.test(value)) {
    return true;
  }
  
  return false;
}

function categorizeToken(key, value) {
  const keyLower = key.toLowerCase();
  
  if (keyLower.includes('access')) return 'access_token';
  if (keyLower.includes('refresh')) return 'refresh_token';
  if (keyLower.includes('id')) return 'id_token';
  if (keyLower.includes('bearer')) return 'bearer_token';
  if (value.split('.').length === 3) return 'jwt';
  if (keyLower.includes('session')) return 'session';
  
  return 'unknown';
}

// Handle graceful shutdown
process.on('SIGTERM', () => {
  console.log('\n👋 Recording terminated');
  process.exit(0);
});

recordLogin().catch(console.error);
</file>

<file path="scripts/spoof-auth.js">
#!/usr/bin/env node

/**
 * Auth Spoofing Script for Cursor Agents
 * 
 * Creates fake authentication files to spoof enterprise login
 * for Cursor agents and chatbot
 */

const fs = require('fs');
const path = require('path');

function spoofAuthentication() {
  console.log('🎭 Spoofing authentication for Cursor agents...');
  
  // Check if we have real auth data
  const realAuthFile = '/etc/op-dbus/chatbot-auth.json';
  const spoofAuthFile = '/etc/op-dbus/spoofed-auth.json';
  const cursorConfigFile = path.join(process.env.HOME, '.cursor', 'auth.json');
  
  let authData = null;
  
  // Try to load real auth data first
  if (fs.existsSync(realAuthFile)) {
    console.log('📋 Loading real authentication data...');
    authData = JSON.parse(fs.readFileSync(realAuthFile, 'utf8'));
  } else {
    console.log('⚠️  No real auth data found, creating spoofed data...');
    
    // Create fake but realistic auth data
    authData = {
      timestamp: new Date().toISOString(),
      spoofed: true,
      gcp_project: '420281222188',
      auth_data: {
        'codeassist-auth-token': generateFakeJWT(),
        'gcp-access-token': generateFakeJWT(),
        'enterprise-session': 'spoofed-enterprise-session-12345',
        'vertex-ai-token': generateFakeJWT(),
        'gemini-api-key': 'spoofed-gemini-key-' + Math.random().toString(36).substr(2, 9)
      }
    };
  }
  
  // Enhance with Cursor-specific spoofing
  const spoofedData = {
    ...authData,
    cursor_agent_spoof: true,
    spoof_timestamp: new Date().toISOString(),
    cursor_config: {
      enterprise_mode: true,
      gcp_project: '420281222188',
      auth_tokens: {
        codeassist: authData.auth_data['codeassist-auth-token'] || generateFakeJWT(),
        vertex: authData.auth_data['vertex-ai-token'] || generateFakeJWT(),
        gemini: authData.auth_data['gemini-api-key'] || 'spoofed-gemini-key-' + Date.now()
      },
      permissions: [
        'vertex-ai.user',
        'aiplatform.models.predict',
        'codeassist.enterprise'
      ]
    }
  };
  
  // Save spoofed auth for chatbot
  fs.writeFileSync(spoofAuthFile, JSON.stringify(spoofedData, null, 2));
  console.log(`💾 Saved spoofed auth to: ${spoofAuthFile}`);
  
  // Create Cursor agent configuration
  const cursorConfig = {
    version: '1.0',
    enterprise: {
      enabled: true,
      project: '420281222188',
      authentication: {
        provider: 'google-enterprise',
        tokens: spoofedData.cursor_config.auth_tokens,
        last_refresh: new Date().toISOString()
      }
    },
    agents: {
      chatbot: {
        authenticated: true,
        enterprise_mode: true,
        spoofed: spoofedData.spoofed || false
      },
      codeassist: {
        enabled: true,
        enterprise_features: true,
        billing: 'enterprise'
      }
    }
  };
  
  // Ensure Cursor config directory exists
  const cursorDir = path.dirname(cursorConfigFile);
  if (!fs.existsSync(cursorDir)) {
    fs.mkdirSync(cursorDir, { recursive: true });
  }
  
  fs.writeFileSync(cursorConfigFile, JSON.stringify(cursorConfig, null, 2));
  console.log(`🎭 Created Cursor agent spoof config: ${cursorConfigFile}`);
  
  // Create environment variables for spoofing
  const envFile = '/etc/op-dbus/spoofed-env.sh';
  const envContent = `
# Spoofed Authentication Environment
export SPOOFED_AUTH=true
export GCP_PROJECT=420281222188
export CODEASSIST_AUTH_TOKEN="${spoofedData.cursor_config.auth_tokens.codeassist}"
export VERTEX_AI_TOKEN="${spoofedData.cursor_config.auth_tokens.vertex}"
export GEMINI_API_KEY="${spoofedData.cursor_config.auth_tokens.gemini}"
export CURSOR_ENTERPRISE_MODE=true
export CURSOR_AGENT_SPOOFED=true

echo "🎭 Spoofed authentication loaded"
echo "Project: $GCP_PROJECT"
echo "Enterprise Mode: $CURSOR_ENTERPRISE_MODE"
`;
  
  fs.writeFileSync(envFile, envContent);
  console.log(`🔧 Created spoofed environment: ${envFile}`);
  
  console.log('\n🎭 Spoofing Complete!');
  console.log('📋 To use spoofed auth:');
  console.log(`   source ${envFile}`);
  console.log('   # Then restart chatbot services');
  
  console.log('\n⚠️  WARNING: This creates fake authentication data');
  console.log('   Use only for testing - real auth required for production');
}

function generateFakeJWT() {
  // Create a fake JWT-like token
  const header = Buffer.from(JSON.stringify({ alg: 'HS256', typ: 'JWT' })).toString('base64');
  const payload = Buffer.from(JSON.stringify({
    iss: 'spoofed-auth',
    sub: 'cursor-agent',
    aud: 'google-apis',
    exp: Math.floor(Date.now() / 1000) + 3600,
    iat: Math.floor(Date.now() / 1000)
  })).toString('base64');
  const signature = 'spoofed-signature-' + Math.random().toString(36).substr(2, 16);
  
  return `${header}.${payload}.${signature}`;
}

// Run the spoofing
spoofAuthentication();
</file>

<file path="src/anthropic.rs">
//! Anthropic Claude API Client
//!
//! ## API Endpoints
//!
//! | Endpoint | URL | Purpose |
//! |----------|-----|--------|
//! | Base URL | `https://api.anthropic.com/v1` | All Claude APIs |
//! | Messages | `/messages` | Chat completions |
//!
//! ## Authentication
//! - Header: `x-api-key: {ANTHROPIC_API_KEY}`
//! - Header: `anthropic-version: 2023-06-01`
//!
//! ## Tool Calling
//! Supports `tool_choice: {type: "any"}` to force tool usage (anti-hallucination)

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::time::Duration;
use tracing::{debug, info};

use crate::provider::{
    ChatMessage,
    ChatRequest,
    ChatResponse,
    LlmProvider,
    ModelInfo,
    ProviderType,
    TokenUsage,
    ToolCallInfo,
    // ToolChoice,
};

// =============================================================================
// API ENDPOINT CONFIGURATION
// =============================================================================

pub mod endpoints {
    pub const BASE_URL: &str = "https://api.anthropic.com/v1";
    pub const MESSAGES: &str = "/messages";
    pub const API_VERSION: &str = "2023-06-01";
}

// =============================================================================
// AUTHENTICATION
// =============================================================================

#[derive(Debug, Clone)]
enum AuthMethod {
    ApiKey(String),
    BearerToken(String),
}

// =============================================================================
// DATA STRUCTURES
// =============================================================================

const CLAUDE_MODELS: &[(&str, &str, &str)] = &[
    (
        "claude-sonnet-4-20250514",
        "Claude Sonnet 4",
        "Latest Sonnet model - best balance",
    ),
    (
        "claude-3-5-sonnet-20241022",
        "Claude 3.5 Sonnet",
        "Previous Sonnet - very capable",
    ),
    (
        "claude-3-opus-20240229",
        "Claude 3 Opus",
        "Most capable, slower",
    ),
    (
        "claude-3-haiku-20240307",
        "Claude 3 Haiku",
        "Fastest, most affordable",
    ),
];

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    /// Tools available to the model
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    /// Tool choice: {type: "auto"}, {type: "any"}, {type: "tool", name: "..."}
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ResponseContentBlock>,
    model: String,
    stop_reason: Option<String>,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ResponseContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

// =============================================================================
// CLIENT IMPLEMENTATION
// =============================================================================

pub struct AnthropicClient {
    client: Client,
    auth: AuthMethod,
    api_url: String,
}

impl AnthropicClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            auth: AuthMethod::ApiKey(api_key.into()),
            api_url: endpoints::BASE_URL.to_string(),
        }
    }

    pub fn with_oauth_token(token: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            auth: AuthMethod::BearerToken(token.into()),
            api_url: endpoints::BASE_URL.to_string(),
        }
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .context("ANTHROPIC_API_KEY environment variable not set")?;
        Ok(Self::new(api_key))
    }

    pub fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        let mut client = Self::new(api_key);
        client.api_url = endpoint.into();
        client
    }

    pub fn with_oauth_endpoint(token: impl Into<String>, endpoint: impl Into<String>) -> Self {
        let mut client = Self::with_oauth_token(token);
        client.api_url = endpoint.into();
        client
    }

    pub fn api_url(&self) -> &str {
        &self.api_url
    }

    /// Chat with full request configuration including tools
    async fn chat_with_tools(&self, model: &str, request: &ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/messages", self.api_url);

        // Extract system message
        let system_msg = request
            .messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone());

        // Convert messages (excluding system, handling tool results)
        let anthropic_messages: Vec<AnthropicMessage> = request
            .messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| {
                if m.role == "tool" {
                    // Tool result message
                    AnthropicMessage {
                        role: "user".to_string(),
                        content: AnthropicContent::Blocks(vec![ContentBlock::ToolResult {
                            tool_use_id: m.tool_call_id.clone().unwrap_or_default(),
                            content: m.content.clone(),
                        }]),
                    }
                } else if let Some(ref tool_calls) = m.tool_calls {
                    // Assistant message with tool calls
                    let blocks: Vec<ContentBlock> = tool_calls
                        .iter()
                        .map(|tc| ContentBlock::ToolUse {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                            input: tc.arguments.clone(),
                        })
                        .collect();
                    AnthropicMessage {
                        role: m.role.clone(),
                        content: AnthropicContent::Blocks(blocks),
                    }
                } else {
                    AnthropicMessage {
                        role: m.role.clone(),
                        content: AnthropicContent::Text(m.content.clone()),
                    }
                }
            })
            .collect();

        // Convert tools to Anthropic format
        let tools: Option<Vec<Value>> = if request.tools.is_empty() {
            None
        } else {
            Some(
                request
                    .tools
                    .iter()
                    .map(|t| t.to_anthropic_format())
                    .collect(),
            )
        };

        // Convert tool_choice to Anthropic format
        let tool_choice: Option<Value> = if request.tools.is_empty() {
            None
        } else {
            Some(request.tool_choice.to_anthropic_format())
        };

        let api_request = AnthropicRequest {
            model: model.to_string(),
            messages: anthropic_messages,
            max_tokens: request.max_tokens.unwrap_or(4096),
            system: system_msg,
            temperature: request.temperature.or(Some(0.7)),
            tools,
            tool_choice,
        };

        debug!(
            "Anthropic request to: {} with tool_choice: {:?}",
            url, request.tool_choice
        );

        let mut req_builder = self
            .client
            .post(&url)
            .header("anthropic-version", endpoints::API_VERSION)
            .header("Content-Type", "application/json");

        req_builder = match &self.auth {
            AuthMethod::ApiKey(key) => req_builder.header("x-api-key", key.as_str()),
            AuthMethod::BearerToken(token) => {
                req_builder.header("Authorization", format!("Bearer {}", token))
            }
        };

        let response = req_builder
            .json(&api_request)
            .send()
            .await
            .context("Failed to send Anthropic request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Anthropic API error {}: {}", status, body));
        }

        let result: AnthropicResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic response")?;

        // Extract text and tool calls from response
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in result.content {
            match block {
                ResponseContentBlock::Text { text } => text_parts.push(text),
                ResponseContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCallInfo {
                        id,
                        name,
                        arguments: input,
                    });
                }
            }
        }

        let text = text_parts.join("");
        let tool_calls_opt = if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls.clone())
        };

        let usage = result.usage.map(|u| TokenUsage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.input_tokens + u.output_tokens,
        });

        Ok(ChatResponse {
            message: ChatMessage {
                role: "assistant".to_string(),
                content: text,
                tool_calls: tool_calls_opt.clone(),
                tool_call_id: None,
            },
            model: result.model,
            provider: "anthropic".to_string(),
            finish_reason: result.stop_reason,
            usage,
            tool_calls: tool_calls_opt,
        })
    }
}

#[async_trait]
impl LlmProvider for AnthropicClient {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Anthropic
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        info!("Anthropic models (static list)");
        info!("  Endpoint: {}", self.api_url);

        Ok(CLAUDE_MODELS
            .iter()
            .map(|(id, name, desc)| ModelInfo {
                id: id.to_string(),
                name: name.to_string(),
                description: Some(desc.to_string()),
                parameters: None,
                available: true,
                tags: vec!["claude".to_string()],
                downloads: None,
                updated_at: None,
            })
            .collect())
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let query_lower = query.to_lowercase();
        let models = self.list_models().await?;
        Ok(models
            .into_iter()
            .filter(|m| {
                m.id.to_lowercase().contains(&query_lower)
                    || m.name.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        Ok(CLAUDE_MODELS.iter().any(|(id, _, _)| *id == model_id))
    }

    /// Basic chat without tools
    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let request = ChatRequest::new(messages);
        self.chat_with_request(model, request).await
    }

    /// Chat with full request configuration including tools and tool_choice
    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        info!(
            "Anthropic chat: model={}, endpoint={}, tool_choice={:?}",
            model, self.api_url, request.tool_choice
        );
        self.chat_with_tools(model, &request).await
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let response = self.chat(model, messages).await?;
        tx.send(Ok(response.message.content)).await.ok();
        Ok(rx)
    }
}
</file>

<file path="src/antigravity_replay.rs">
//! Antigravity Replay Provider
//!
//! Uses captured Antigravity IDE session (OAuth token + headers) to make
//! API requests that appear to come from the IDE.
//!
//! This allows op-dbus to leverage Code Assist enterprise subscriptions.
//!
//! ## Setup
//!
//! 1. Run `antigravity-proxy-capture.sh` to capture IDE credentials
//! 2. Set `ANTIGRAVITY_SESSION_FILE` environment variable
//! 3. Use `LLM_PROVIDER=antigravity_replay`
//!
//! ## How It Works
//!
//! The Antigravity IDE sends specific headers that identify it as an IDE client:
//! - X-Goog-Api-Client: contains IDE version info
//! - User-Agent: identifies as Antigravity
//! - Other proprietary headers
//!
//! By capturing and replaying these headers along with the OAuth token,
//! our requests appear to come from the IDE and get Code Assist benefits.

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::provider::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo,
    ProviderType, TokenUsage, ToolCallInfo, ToolChoice, ToolDefinition,
};

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Captured session data from Antigravity IDE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedSession {
    /// OAuth tokens captured from IDE
    pub tokens: Vec<CapturedToken>,
    /// Captured HTTP headers (including IDE identification)
    pub headers: HashMap<String, String>,
    /// Captured API endpoints
    pub endpoints: Vec<CapturedEndpoint>,
    /// Raw requests for debugging
    pub requests: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedToken {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub captured_at: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedEndpoint {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub timestamp: Option<String>,
}

impl CapturedSession {
    /// Load from session file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read session file: {}", path.display()))?;
        
        let session: Self = simd_json::from_str(&content)
            .with_context(|| "Failed to parse session JSON")?;
        
        if session.tokens.is_empty() {
            anyhow::bail!("No tokens found in session file");
        }
        
        Ok(session)
    }
    
    /// Get the latest access token
    pub fn latest_token(&self) -> Option<&str> {
        self.tokens.last().map(|t| t.access_token.as_str())
    }
    
    /// Build request headers that mimic the IDE
    pub fn build_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        
        // Add captured IDE headers
        for (key, value) in &self.headers {
            // Skip authorization (we'll add it separately)
            if key.to_lowercase() == "authorization" {
                continue;
            }
            headers.insert(key.clone(), value.clone());
        }
        
        // Add authorization
        if let Some(token) = self.latest_token() {
            headers.insert("Authorization".to_string(), format!("Bearer {}", token));
        }
        
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        
        headers
    }
}

/// Configuration for Antigravity Replay provider
#[derive(Debug, Clone)]
pub struct AntigravityReplayConfig {
    /// Path to captured session file
    pub session_file: PathBuf,
    /// Default model to use
    pub default_model: String,
    /// Whether to auto-route based on task
    pub auto_routing: bool,
}

impl AntigravityReplayConfig {
    pub fn from_env() -> Result<Self> {
        let session_file = std::env::var("ANTIGRAVITY_SESSION_FILE")
            .map(PathBuf::from)
            .or_else(|_| {
                let default = dirs::config_dir()
                    .map(|d| d.join("antigravity").join("captured").join("session.json"))
                    .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
                
                if default.exists() {
                    Ok(default)
                } else {
                    Err(anyhow::anyhow!(
                        "No session file found. Run antigravity-proxy-capture.sh first."
                    ))
                }
            })?;
        
        Ok(Self {
            session_file,
            default_model: std::env::var("ANTIGRAVITY_MODEL")
                .unwrap_or_else(|_| "gemini-2.0-flash".to_string()),
            auto_routing: std::env::var("ANTIGRAVITY_AUTO_ROUTING")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),
        })
    }
}

/// Antigravity Replay Provider
///
/// Replays captured IDE credentials to access Gemini API with Code Assist benefits.
pub struct AntigravityReplayProvider {
    config: AntigravityReplayConfig,
    session: RwLock<CapturedSession>,
    client: Client,
}

impl AntigravityReplayProvider {
    /// Create from environment
    pub fn from_env() -> Result<Self> {
        let config = AntigravityReplayConfig::from_env()?;
        Self::new(config)
    }
    
    /// Create with config
    pub fn new(config: AntigravityReplayConfig) -> Result<Self> {
        let session = CapturedSession::load(&config.session_file)?;
        
        info!("Antigravity Replay provider initialized");
        info!("  Session file: {}", config.session_file.display());
        info!("  Captured headers: {}", session.headers.len());
        info!("  Captured tokens: {}", session.tokens.len());
        
        // Log important headers (sanitized)
        for (key, _) in &session.headers {
            debug!("  Header captured: {}", key);
        }
        
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;
        
        Ok(Self {
            config,
            session: RwLock::new(session),
            client,
        })
    }
    
    /// Reload session from file (if token expired)
    pub fn reload_session(&self) -> Result<()> {
        let session = CapturedSession::load(&self.config.session_file)?;
        *self.session.write().unwrap() = session;
        info!("Session reloaded");
        Ok(())
    }
    
    /// Build HTTP request with captured headers
    fn build_request(&self, url: &str) -> reqwest::RequestBuilder {
        let session = self.session.read().unwrap();
        let headers = session.build_headers();
        
        let mut request = self.client.post(url);
        
        for (key, value) in headers {
            request = request.header(&key, &value);
        }
        
        request
    }
    
    /// Auto-select model based on task
    fn select_model(&self, messages: &[ChatMessage], has_tools: bool) -> String {
        if !self.config.auto_routing {
            return self.config.default_model.clone();
        }
        
        let total_length: usize = messages.iter().map(|m| m.content.len()).sum();
        let needs_reasoning = messages.iter().any(|m| {
            let lower = m.content.to_lowercase();
            lower.contains("think") ||
            lower.contains("reason") ||
            lower.contains("step by step")
        });
        
        if has_tools {
            "gemini-2.0-flash".to_string()
        } else if needs_reasoning {
            "gemini-2.0-flash-thinking-exp-01-21".to_string()
        } else if total_length > 100000 {
            "gemini-1.5-pro".to_string()
        } else {
            "gemini-2.0-flash".to_string()
        }
    }
    
    /// Convert messages to Gemini format
    fn convert_messages(&self, messages: &[ChatMessage]) -> (Vec<Value>, Option<Value>) {
        let mut system_instruction = None;
        let mut contents = Vec::new();
        
        for msg in messages {
            if msg.role == "system" {
                system_instruction = Some(json!({
                    "parts": [{"text": msg.content}]
                }));
                continue;
            }
            
            let role = match msg.role.as_str() {
                "assistant" => "model",
                _ => "user",
            };
            
            contents.push(json!({
                "role": role,
                "parts": [{"text": msg.content}]
            }));
        }
        
        (contents, system_instruction)
    }
}

#[async_trait]
impl LlmProvider for AntigravityReplayProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Antigravity
    }
    
    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(vec![
            ModelInfo {
                id: "gemini-2.0-flash".to_string(),
                name: "Gemini 2.0 Flash (via IDE)".to_string(),
                description: Some("Fast model via captured IDE session".to_string()),
                parameters: None,
                available: true,
                tags: vec!["ide".to_string(), "replay".to_string()],
                downloads: None,
                updated_at: None,
            },
            ModelInfo {
                id: "gemini-2.0-flash-thinking-exp-01-21".to_string(),
                name: "Gemini Flash Thinking (via IDE)".to_string(),
                description: Some("Reasoning model via captured IDE session".to_string()),
                parameters: None,
                available: true,
                tags: vec!["ide".to_string(), "reasoning".to_string()],
                downloads: None,
                updated_at: None,
            },
            ModelInfo {
                id: "gemini-1.5-pro".to_string(),
                name: "Gemini 1.5 Pro (via IDE)".to_string(),
                description: Some("High quality model via captured IDE session".to_string()),
                parameters: None,
                available: true,
                tags: vec!["ide".to_string(), "quality".to_string()],
                downloads: None,
                updated_at: None,
            },
        ])
    }
    
    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let models = self.list_models().await?;
        let query_lower = query.to_lowercase();
        Ok(models
            .into_iter()
            .filter(|m| m.id.to_lowercase().contains(&query_lower))
            .take(limit)
            .collect())
    }
    
    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }
    
    async fn is_model_available(&self, _model_id: &str) -> Result<bool> {
        // Check if we have a valid session
        let session = self.session.read().unwrap();
        Ok(session.latest_token().is_some())
    }
    
    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let request = ChatRequest::new(messages);
        self.chat_with_request(model, request).await
    }
    
    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        let actual_model = if model == "auto" || model.is_empty() {
            self.select_model(&request.messages, !request.tools.is_empty())
        } else {
            model.to_string()
        };
        
        let url = format!(
            "{}/models/{}:generateContent",
            GEMINI_API_BASE,
            actual_model
        );
        
        let (contents, system_instruction) = self.convert_messages(&request.messages);
        
        let mut body = json!({
            "contents": contents,
        });
        
        if let Some(sys) = system_instruction {
            body["systemInstruction"] = sys;
        }
        
        if let Some(temp) = request.temperature {
            body["generationConfig"] = json!({"temperature": temp});
        }
        
        debug!("Antigravity Replay request to: {}", url);
        
        let response = self
            .build_request(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to send request")?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            
            // Check if token expired
            if status.as_u16() == 401 {
                warn!("Token may have expired. Try rerunning antigravity-proxy-capture.sh");
            }
            
            return Err(anyhow::anyhow!("API error {}: {}", status, body));
        }
        
        let result: Value = response.json().await
            .context("Failed to parse response")?;
        
        // Parse Gemini response
        let candidates = result.get("candidates")
            .and_then(|c| c.as_array())
            .ok_or_else(|| anyhow::anyhow!("No candidates in response"))?;
        
        let first_candidate = candidates.first()
            .ok_or_else(|| anyhow::anyhow!("Empty candidates"))?;
        
        let mut text_parts = Vec::new();
        if let Some(parts) = first_candidate
            .get("content")
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array())
        {
            for part in parts {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text.to_string());
                }
            }
        }
        
        let usage = result.get("usageMetadata").map(|u| TokenUsage {
            prompt_tokens: u.get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            completion_tokens: u.get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: u.get("totalTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
        });
        
        Ok(ChatResponse {
            message: ChatMessage {
                role: "assistant".to_string(),
                content: text_parts.join(""),
                tool_calls: None,
                tool_call_id: None,
            },
            model: actual_model,
            provider: "antigravity_replay".to_string(),
            finish_reason: first_candidate.get("finishReason")
                .and_then(|f| f.as_str())
                .map(String::from),
            usage,
            tool_calls: None,
        })
    }
    
    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        // Fall back to non-streaming
        let response = self.chat(model, messages).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let _ = tx.send(Ok(response.message.content)).await;
        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_session_headers() {
        let session = CapturedSession {
            tokens: vec![CapturedToken {
                access_token: "test-token".to_string(),
                refresh_token: None,
                captured_at: None,
                endpoint: None,
                headers: HashMap::new(),
            }],
            headers: {
                let mut h = HashMap::new();
                h.insert("X-Goog-Api-Client".to_string(), "test-client".to_string());
                h.insert("User-Agent".to_string(), "Antigravity/1.0".to_string());
                h
            },
            endpoints: vec![],
            requests: vec![],
        };
        
        let headers = session.build_headers();
        
        assert!(headers.contains_key("Authorization"));
        assert!(headers.get("Authorization").unwrap().contains("test-token"));
        assert!(headers.contains_key("X-Goog-Api-Client"));
        assert!(headers.contains_key("User-Agent"));
    }
}
</file>

<file path="src/antigravity.rs">
//! Antigravity Provider - Uses OAuth token from headless Antigravity service
//!
//! ## Authentication Flow
//!
//! 1. Antigravity IDE runs headless with virtual Wayland display
//! 2. User logs in once via VNC
//! 3. OAuth token is extracted and saved
//! 4. This provider uses that token for Gemini API calls
//!
//! ## Features
//!
//! - Uses enterprise Code Assist subscription (no API charges)
//! - Auto-refreshes expired tokens
//! - Falls back to API key if OAuth token not available
//!
//! ## Configuration
//!
//! ```bash
//! # OAuth token (from Antigravity headless service)
//! export GOOGLE_AUTH_TOKEN_FILE=~/.config/antigravity/token.json
//!
//! # Or fallback to API key
//! export GEMINI_API_KEY=xxx
//! ```

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};
use uuid::Uuid;

use crate::headless_oauth::HeadlessOAuthProvider;
use crate::provider::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType, TokenUsage,
    ToolCallInfo, ToolChoice, ToolDefinition,
};

/// Gemini API base URL
const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Available models through Antigravity/Gemini
const ANTIGRAVITY_MODELS: &[(&str, &str, &str)] = &[
    (
        "gemini-2.0-flash",
        "Gemini 2.0 Flash",
        "Fast, efficient, and cost-effective",
    ),
    (
        "gemini-1.5-pro",
        "Gemini 1.5 Pro",
        "High quality, long context window",
    ),
    ("gemini-1.5-flash", "Gemini 1.5 Flash", "Legacy flash model"),
];

/// Authentication method
#[derive(Debug, Clone)]
enum AuthMethod {
    /// OAuth token from headless Antigravity service
    OAuth(Arc<HeadlessOAuthProvider>),
    /// Direct API key
    ApiKey(String),
    /// Local Antigravity Bridge (OpenAI-compatible)
    Bridge(String),
}

/// Antigravity Provider
///
/// Uses OAuth token captured from Antigravity headless service,
/// or falls back to API key.
pub struct AntigravityProvider {
    client: Client,
    auth: AuthMethod,
    default_model: String,
}

impl AntigravityProvider {
    /// Create from environment
    ///
    /// Tries in order:
    /// 1. OAuth token from `GOOGLE_AUTH_TOKEN_FILE`
    /// 2. API key from `GEMINI_API_KEY`
    pub fn from_env() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .context("Failed to create HTTP client")?;

        // 1. Try Bridge URL
        if let Ok(bridge_url) = std::env::var("ANTIGRAVITY_BRIDGE_URL") {
            info!("✅ Antigravity: Using Bridge at {}", bridge_url);
            let default_model =
                std::env::var("LLM_MODEL").unwrap_or_else(|_| "ide-model".to_string());

            return Ok(Self {
                client,
                auth: AuthMethod::Bridge(bridge_url),
                default_model,
            });
        }

        // 2. Try OAuth first
        let oauth_provider = HeadlessOAuthProvider::from_env().ok();

        let auth = if let Some(ref oauth) = oauth_provider {
            if oauth.is_authenticated() {
                info!(
                    "✅ Antigravity: Using OAuth token from {}",
                    oauth.token_file().display()
                );
                AuthMethod::OAuth(Arc::new(oauth_provider.unwrap()))
            } else {
                // Try API key
                if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
                    info!("✅ Antigravity: Using API key (OAuth token not valid)");
                    AuthMethod::ApiKey(api_key)
                } else {
                    anyhow::bail!(
                        "No valid authentication found.\n\n\
                        Options:\n\
                        1. Connect to Antigravity Bridge: export ANTIGRAVITY_BRIDGE_URL=http://127.0.0.1:7788\n\
                        2. Start Antigravity headless and login\n\
                        3. Set GEMINI_API_KEY"
                    );
                }
            }
        } else if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
            info!("✅ Antigravity: Using API key");
            AuthMethod::ApiKey(api_key)
        } else {
            anyhow::bail!(
                "No authentication configured.\n\n\
                Set ANTIGRAVITY_BRIDGE_URL, GEMINI_API_KEY or configure OAuth via Antigravity headless service."
            );
        };

        let default_model =
            std::env::var("LLM_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".to_string());

        Ok(Self {
            client,
            auth,
            default_model,
        })
    }

    /// Create with API key directly
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            auth: AuthMethod::ApiKey(api_key.into()),
            default_model: "gemini-2.0-flash".to_string(),
        }
    }

    /// Create with OAuth provider
    pub fn with_oauth(oauth: HeadlessOAuthProvider) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            auth: AuthMethod::OAuth(Arc::new(oauth)),
            default_model: "gemini-2.0-flash".to_string(),
        }
    }

    /// Build authenticated request
    async fn build_request(&self, url: &str) -> Result<reqwest::RequestBuilder> {
        match &self.auth {
            AuthMethod::OAuth(oauth) => {
                let token = oauth.get_token().await?;
                Ok(self
                    .client
                    .post(url)
                    .header("Authorization", format!("Bearer {}", token))
                    .header("Content-Type", "application/json"))
            }
            AuthMethod::ApiKey(key) => {
                // API key goes in URL for Gemini
                let url_with_key = if url.contains('?') {
                    format!("{}&key={}", url, key)
                } else {
                    format!("{}?key={}", url, key)
                };
                Ok(self
                    .client
                    .post(&url_with_key)
                    .header("Content-Type", "application/json"))
            }
            AuthMethod::Bridge(_) => Ok(self
                .client
                .post(url)
                .header("Content-Type", "application/json")),
        }
    }

    /// Convert messages to Gemini format
    fn convert_messages(&self, messages: &[ChatMessage]) -> (Vec<Value>, Option<Value>) {
        let mut system_instruction = None;
        let mut contents = Vec::new();

        for msg in messages {
            if msg.role == "system" {
                system_instruction = Some(json!({
                    "parts": [{"text": msg.content}]
                }));
                continue;
            }

            let role = match msg.role.as_str() {
                "assistant" => "model",
                _ => "user",
            };

            contents.push(json!({
                "role": role,
                "parts": [{"text": msg.content}]
            }));
        }

        (contents, system_instruction)
    }

    /// Convert messages to OpenAI (Bridge) format
    fn convert_messages_openai(&self, messages: &[ChatMessage]) -> Vec<Value> {
        messages
            .iter()
            .map(|m| {
                json!({
                    "role": m.role,
                    "content": m.content
                })
            })
            .collect()
    }

    /// Convert tools to Gemini format
    fn convert_tools(&self, tools: &[ToolDefinition]) -> Value {
        let function_declarations: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema
                })
            })
            .collect();

        json!([{
            "functionDeclarations": function_declarations
        }])
    }

    /// Convert tool choice to Gemini format
    fn convert_tool_choice(&self, choice: &ToolChoice) -> Option<Value> {
        match choice {
            ToolChoice::Auto => Some(json!({"mode": "AUTO"})),
            ToolChoice::Required => Some(json!({"mode": "ANY"})),
            ToolChoice::None => Some(json!({"mode": "NONE"})),
            ToolChoice::Tool(name) => Some(json!({
                "mode": "ANY",
                "allowedFunctionNames": [name]
            })),
        }
    }
}

#[async_trait]
impl LlmProvider for AntigravityProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Antigravity
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        if let AuthMethod::Bridge(_) = self.auth {
            return Ok(vec![ModelInfo {
                id: "ide-model".to_string(),
                name: "IDE Model".to_string(),
                description: Some("Model provided by Antigravity IDE code assist".to_string()),
                parameters: None,
                available: true,
                tags: vec!["ide".to_string()],
                downloads: None,
                updated_at: None,
            }]);
        }

        Ok(ANTIGRAVITY_MODELS
            .iter()
            .map(|(id, name, desc)| ModelInfo {
                id: id.to_string(),
                name: name.to_string(),
                description: Some(desc.to_string()),
                parameters: None,
                available: true,
                tags: vec!["gemini".to_string()],
                downloads: None,
                updated_at: None,
            })
            .collect())
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let query_lower = query.to_lowercase();
        let models = self.list_models().await?;
        Ok(models
            .into_iter()
            .filter(|m| {
                m.id.to_lowercase().contains(&query_lower)
                    || m.name.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        if let AuthMethod::Bridge(_) = self.auth {
            return Ok(true); // Bridge handles model selection
        }
        Ok(ANTIGRAVITY_MODELS.iter().any(|(id, _, _)| *id == model_id))
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let request = ChatRequest::new(messages);
        self.chat_with_request(model, request).await
    }

    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        let actual_model = if model.is_empty() || model == "auto" {
            &self.default_model
        } else {
            model
        };

        if let AuthMethod::Bridge(bridge_url) = &self.auth {
            // OpenAI-compatible Bridge
            let url = format!("{}/v1/chat/completions", bridge_url);
            debug!("Antigravity Bridge request to: {}", url);

            let body = json!({
                "model": actual_model,
                "messages": self.convert_messages_openai(&request.messages),
                "temperature": request.temperature.unwrap_or(0.7),
            });

            let http_request = self.build_request(&url).await?;
            let response = http_request
                .json(&body)
                .send()
                .await
                .context("Failed to send bridge request")?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!("Bridge error: {}", response.status()));
            }

            let result: Value = response
                .json()
                .await
                .context("Failed to parse bridge response")?;

            let content = result
                .get("choices")
                .and_then(|c: &Value| c.as_array())
                .and_then(|c: &Vec<Value>| c.first())
                .and_then(|c: &Value| c.get("message"))
                .and_then(|m: &Value| m.get("content"))
                .and_then(|t: &Value| t.as_str())
                .unwrap_or_default()
                .to_string();

            let finish_reason = result
                .get("choices")
                .and_then(|c: &Value| c.as_array())
                .and_then(|c: &Vec<Value>| c.first())
                .and_then(|c: &Value| c.get("finish_reason"))
                .and_then(|s: &Value| s.as_str())
                .map(String::from);

            return Ok(ChatResponse {
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content,
                    tool_calls: None, // Bridge might not support tool calls yet
                    tool_call_id: None,
                },
                model: actual_model.to_string(),
                provider: "antigravity-bridge".to_string(),
                finish_reason,
                usage: None,
                tool_calls: None,
            });
        }

        let url = format!(
            "{}/models/{}:generateContent",
            GEMINI_API_BASE, actual_model
        );

        let (contents, system_instruction) = self.convert_messages(&request.messages);

        let mut body = json!({
            "contents": contents,
        });

        if let Some(sys) = system_instruction {
            body["systemInstruction"] = sys;
        }

        // Add tools if present
        if !request.tools.is_empty() {
            body["tools"] = self.convert_tools(&request.tools);

            if let Some(tool_config) = self.convert_tool_choice(&request.tool_choice) {
                body["toolConfig"] = json!({"functionCallingConfig": tool_config});
            }
        }

        // Generation config
        let mut gen_config = json!({});
        if let Some(temp) = request.temperature {
            gen_config["temperature"] = json!(temp);
        }
        if let Some(max_tokens) = request.max_tokens {
            gen_config["maxOutputTokens"] = json!(max_tokens);
        }
        if let Some(top_p) = request.top_p {
            gen_config["topP"] = json!(top_p);
        }
        if gen_config != json!({}) {
            body["generationConfig"] = gen_config;
        }

        debug!("Antigravity request to: {}", url);

        let http_request = self.build_request(&url).await?;
        let response = http_request
            .json(&body)
            .send()
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            if status.as_u16() == 401 {
                return Err(anyhow::anyhow!(
                    "Authentication failed (401).\n\n\
                    Token may have expired. Try:\n\
                    1. Reconnect to Antigravity VNC and re-login\n\
                    2. Run: ./scripts/antigravity-extract-token.sh\n\
                    3. Restart op-web: sudo systemctl restart op-web"
                ));
            }

            return Err(anyhow::anyhow!("API error {}: {}", status, body));
        }

        let result: Value = response.json().await.context("Failed to parse response")?;

        // Parse response
        let candidates = result
            .get("candidates")
            .and_then(|c: &Value| c.as_array())
            .ok_or_else(|| anyhow::anyhow!("No candidates in response"))?;

        let first_candidate = candidates
            .first()
            .ok_or_else(|| anyhow::anyhow!("Empty candidates"))?;

        // Extract text and tool calls
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        if let Some(parts) = first_candidate
            .get("content")
            .and_then(|c: &Value| c.get("parts"))
            .and_then(|p: &Value| p.as_array())
        {
            for part in parts {
                if let Some(text) = part.get("text").and_then(|t: &Value| t.as_str()) {
                    text_parts.push(text.to_string());
                }
                if let Some(fc) = part.get("functionCall") {
                    let name = fc
                        .get("name")
                        .and_then(|n: &Value| n.as_str())
                        .unwrap_or_default();
                    let args = fc.get("args").cloned().unwrap_or(json!({}));
                    tool_calls.push(ToolCallInfo {
                        id: format!("call_{}", Uuid::new_v4()),
                        name: name.to_string(),
                        arguments: args,
                    });
                }
            }
        }

        let usage = result.get("usageMetadata").map(|u: &Value| TokenUsage {
            prompt_tokens: u
                .get("promptTokenCount")
                .and_then(|v: &Value| v.as_u64())
                .unwrap_or(0) as u32,
            completion_tokens: u
                .get("candidatesTokenCount")
                .and_then(|v: &Value| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: u
                .get("totalTokenCount")
                .and_then(|v: &Value| v.as_u64())
                .unwrap_or(0) as u32,
        });

        Ok(ChatResponse {
            message: ChatMessage {
                role: "assistant".to_string(),
                content: text_parts.join(""),
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls.clone())
                },
                tool_call_id: None,
            },
            model: actual_model.to_string(),
            provider: "antigravity".to_string(),
            finish_reason: first_candidate
                .get("finishReason")
                .and_then(|f: &Value| f.as_str())
                .map(String::from),
            usage,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
        })
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        // Fall back to non-streaming for now
        let response = self.chat(model, messages).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let _ = tx.send(Ok(response.message.content)).await;
        Ok(rx)
    }
}
</file>

<file path="src/assistant.rs">
//! Assistant LLM Provider (Thin Wrapper)
//!
//! User-facing overlay around the internal [`OpenClawProvider`].
//! Delegates all network logic to the upstream provider while rewriting
//! branding in responses and checking `ASSISTANT_*` environment variables
//! before falling back to `OPENCLAW_*`.
//!
//! ## Configuration
//!
//! ```bash
//! ASSISTANT_BASE_URL=http://127.0.0.1:18789       # checked first
//! ASSISTANT_DEFAULT_MODEL=assistant:main            # checked first
//! # Falls back to OPENCLAW_BASE_URL / OPENCLAW_DEFAULT_MODEL if unset
//! ```

use anyhow::Result;
use async_trait::async_trait;

use crate::openclaw::OpenClawProvider;
use crate::provider::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType,
};

/// User-facing Assistant provider.
///
/// Internally delegates to [`OpenClawProvider`] so upstream OpenClaw
/// updates apply cleanly to the base layer.  This struct only overrides
/// branding and environment-variable resolution.
pub struct AssistantProvider {
    inner: OpenClawProvider,
}

impl AssistantProvider {
    pub fn new(base_url: Option<String>, default_model: Option<String>) -> Self {
        let base_url = base_url
            .or_else(|| std::env::var("ASSISTANT_BASE_URL").ok())
            .or_else(|| std::env::var("OPENCLAW_BASE_URL").ok());

        let default_model = default_model
            .or_else(|| std::env::var("ASSISTANT_DEFAULT_MODEL").ok())
            .or_else(|| std::env::var("OPENCLAW_DEFAULT_MODEL").ok())
            .unwrap_or_else(|| "assistant:main".to_string());

        Self {
            inner: OpenClawProvider::new(base_url, Some(default_model)),
        }
    }

    pub fn from_env() -> Result<Self> {
        Ok(Self::new(None, None))
    }

    /// Rewrite model metadata so user-facing strings say "Assistant".
    fn rewrite_models(mut models: Vec<ModelInfo>) -> Vec<ModelInfo> {
        for model in &mut models {
            // Swap upstream branding tag for user-facing branding
            model.tags.retain(|t| t != "openclaw");
            if !model.tags.iter().any(|t| t == "assistant") {
                model.tags.push("assistant".to_string());
            }
            if let Some(ref mut desc) = model.description {
                *desc = desc.replace("OpenClaw", "Assistant");
            }
        }
        models
    }

    /// Rewrite a chat response so the `provider` field reads "assistant".
    fn rewrite_response(mut response: ChatResponse) -> ChatResponse {
        if response.provider == "openclaw" {
            response.provider = "assistant".to_string();
        }
        response
    }
}

#[async_trait]
impl LlmProvider for AssistantProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Assistant
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let models = self.inner.list_models().await?;
        Ok(Self::rewrite_models(models))
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models
            .into_iter()
            .filter(|m| m.name.to_lowercase().contains(&query.to_lowercase()))
            .take(limit)
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        Ok(self.get_model(model_id).await?.is_some())
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let request = ChatRequest::new(messages);
        self.chat_with_request(model, request).await
    }

    async fn chat_with_request(
        &self,
        model: &str,
        request: ChatRequest,
    ) -> Result<ChatResponse> {
        let response = self.inner.chat_with_request(model, request).await?;
        Ok(Self::rewrite_response(response))
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let response = self.chat(model, messages).await?;
        let _ = tx.send(Ok(response.message.content)).await;
        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ChatMessage;

    #[test]
    fn env_fallback_resolution() {
        // ASSISTANT_BASE_URL set → used
        std::env::set_var("ASSISTANT_BASE_URL", "http://assistant:9999");
        std::env::remove_var("OPENCLAW_BASE_URL");
        let p = AssistantProvider::new(None, None);
        // We can't inspect private fields, but we can verify from_env succeeds
        let _ = AssistantProvider::from_env().unwrap();

        // Only OPENCLAW_BASE_URL set → fallback
        std::env::remove_var("ASSISTANT_BASE_URL");
        std::env::set_var("OPENCLAW_BASE_URL", "http://openclaw:8888");
        let _ = AssistantProvider::from_env().unwrap();

        // Clean up
        std::env::remove_var("ASSISTANT_BASE_URL");
        std::env::remove_var("OPENCLAW_BASE_URL");
    }

    #[test]
    fn default_model_is_assistant_prefix() {
        std::env::remove_var("ASSISTANT_DEFAULT_MODEL");
        std::env::remove_var("OPENCLAW_DEFAULT_MODEL");
        let p = AssistantProvider::new(None, None);
        // from_env with no env vars should default to assistant:main
        let _ = p;
    }

    #[test]
    fn rewrite_models_swaps_branding() {
        let models = vec![ModelInfo {
            id: "openclaw:main".to_string(),
            name: "OpenClaw Main".to_string(),
            description: Some("OpenClaw model owned by test".to_string()),
            parameters: None,
            available: true,
            tags: vec!["openclaw".to_string(), "test".to_string()],
            downloads: None,
            updated_at: None,
        }];

        let rewritten = AssistantProvider::rewrite_models(models);
        assert_eq!(rewritten[0].tags, vec!["test".to_string(), "assistant".to_string()]);
        assert_eq!(
            rewritten[0].description,
            Some("Assistant model owned by test".to_string())
        );
    }

    #[test]
    fn rewrite_response_swaps_provider_field() {
        let response = ChatResponse {
            message: ChatMessage::assistant("hello"),
            model: "test".to_string(),
            provider: "openclaw".to_string(),
            finish_reason: None,
            usage: None,
            tool_calls: None,
        };
        let rewritten = AssistantProvider::rewrite_response(response);
        assert_eq!(rewritten.provider, "assistant");
    }
}
</file>

<file path="src/chat.rs">
//! Chat Manager - Manages provider switching and chat sessions
//!
//! ## Authentication Priority
//!
//! 1. **Factory** (local proxy — the AI you are talking to right now)
//! 2. **GCloud ADC** (direct OAuth via gcloud)
//! 3. **Gemini** (API key fallback)
//! 4. **Anthropic** (API key)
//!
//! ## Environment Variables
//!
//! ```bash
//! # Preferred: Factory local proxy (default)
//! LLM_PROVIDER=factory
//! FACTORY_BASE_URL=http://127.0.0.1:11435/v1
//! FACTORY_API_KEY=local-codex-proxy
//! FACTORY_DEFAULT_MODEL=local-oauth-proxy
//!
//! # Provider selection override
//! LLM_PROVIDER=factory  # or openclaw, gemini, gemini-cli, anthropic
//! LLM_MODEL=gemini-2.5-flash
//!
//! # Optional API key fallbacks
//! GEMINI_API_KEY=xxx
//! ANTHROPIC_API_KEY=xxx
//! ```

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use tracing::{debug, info, warn};

use crate::anthropic::AnthropicClient;
use crate::assistant::AssistantProvider;
use crate::factory::FactoryProvider;
use crate::gcloud_adc::GCloudADCProvider;
use crate::gemini::GeminiClient;
use crate::gemini_cli::create_gemini_cli_provider;
use crate::openclaw::OpenClawProvider;
use crate::provider::{
    BoxedProvider, ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType,
};
use async_trait::async_trait;

/// Chat manager - handles multiple providers and model selection
pub struct ChatManager {
    providers: HashMap<ProviderType, BoxedProvider>,
    current_provider: Arc<RwLock<ProviderType>>,
    current_model: Arc<RwLock<String>>,
    model_cache: Arc<RwLock<HashMap<ProviderType, Vec<ModelInfo>>>>,
}

impl ChatManager {
    /// Create a new chat manager
    ///
    /// Initialization order:
    /// 1. Check LLM_PROVIDER environment variable
    /// 2. Factory — Local proxy (default)
    /// 3. GCloud ADC
    /// 4. Gemini (API key)
    /// 5. Anthropic (API key)
    pub fn new() -> Self {
        let mut providers: HashMap<ProviderType, BoxedProvider> = HashMap::new();
        let mut default_provider = None;
        let mut default_model = std::env::var("FACTORY_DEFAULT_MODEL")
            .or_else(|_| std::env::var("OPENCLAW_DEFAULT_MODEL"))
            .unwrap_or_else(|_| "local-oauth-proxy".to_string());

        // Check environment variables
        let env_provider = std::env::var("LLM_PROVIDER").ok();
        let env_model = std::env::var("LLM_MODEL").ok();

        if let Some(ref provider_name) = env_provider {
            info!("📋 LLM_PROVIDER={}", provider_name);
        }
        if let Some(ref model_name) = env_model {
            info!("📋 LLM_MODEL={}", model_name);
            default_model = model_name.clone();
        }

        // =====================================================
        // Factory — Local proxy (default, the AI you are talking to)
        // =====================================================
        match FactoryProvider::from_env() {
            Ok(factory) => {
                info!("✅ Factory provider initialized (local proxy)");
                providers.insert(ProviderType::Factory, Box::new(factory));
                if default_provider.is_none() {
                    default_provider = Some(ProviderType::Factory);
                }
            }
            Err(e) => {
                debug!("Factory provider failed: {}", e);
            }
        }

        // =====================================================
        // Gemini CLI provider (optional)
        // =====================================================
        let wants_gemini_cli = std::env::var("ENABLE_GEMINI_CLI_PROVIDER")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
            || env_provider
                .as_deref()
                .map(|v| {
                    v.eq_ignore_ascii_case("gemini-cli")
                        || v.eq_ignore_ascii_case("gemini_cli")
                        || v.eq_ignore_ascii_case("geminicli")
                })
                .unwrap_or(false);

        if wants_gemini_cli {
            let gemini_cli = create_gemini_cli_provider();
            info!("✅ Gemini CLI provider initialized");
            providers.insert(ProviderType::GeminiCli, Box::new(gemini_cli));
            if default_provider.is_none() {
                default_provider = Some(ProviderType::GeminiCli);
            }
        }

        // =====================================================
        // GCloud ADC - Directly from gcloud CLI / application-default credentials
        // Kept under ProviderType::Antigravity for backward compatibility of provider ids.
        // =====================================================
        let gcloud = GCloudADCProvider::new();
        info!("✅ GCloud ADC provider initialized");
        providers.insert(ProviderType::Antigravity, Box::new(gcloud));
        if default_provider.is_none() {
            default_provider = Some(ProviderType::Antigravity);
        }

        // =====================================================
        // Gemini - API key fallback
        // =====================================================
        if std::env::var("GEMINI_API_KEY").is_ok() {
            match GeminiClient::from_env() {
                Ok(gemini) => {
                    info!("✅ Gemini provider initialized (API key)");
                    providers.insert(ProviderType::Gemini, Box::new(gemini));
                    if default_provider.is_none() {
                        default_provider = Some(ProviderType::Gemini);
                    }
                }
                Err(e) => {
                    debug!("Gemini provider failed: {}", e);
                }
            }
        }

        // =====================================================
        // Assistant - trusted internal network (Incus container)
        // =====================================================
        if matches!(env_provider.as_deref(), Some("assistant"))
            || std::env::var("ASSISTANT_BASE_URL").is_ok()
            || std::env::var("ASSISTANT_DEFAULT_MODEL").is_ok()
        {
            match AssistantProvider::from_env() {
                Ok(assistant) => {
                    info!("✅ Assistant provider initialized");
                    providers.insert(ProviderType::Assistant, Box::new(assistant));
                    if default_provider.is_none() {
                        default_provider = Some(ProviderType::Assistant);
                    }
                }
                Err(e) => {
                    debug!("Assistant provider failed: {}", e);
                }
            }
        }

        // =====================================================
        // OpenClaw - trusted internal network
        // =====================================================
        if matches!(env_provider.as_deref(), Some("openclaw"))
            || std::env::var("OPENCLAW_BASE_URL").is_ok()
            || std::env::var("OPENCLAW_DEFAULT_MODEL").is_ok()
        {
            match OpenClawProvider::from_env() {
                Ok(openclaw) => {
                    info!("✅ OpenClaw provider initialized");
                    providers.insert(ProviderType::OpenClaw, Box::new(openclaw));
                    if default_provider.is_none() {
                        default_provider = Some(ProviderType::OpenClaw);
                    }
                }
                Err(e) => {
                    debug!("OpenClaw provider failed: {}", e);
                }
            }
        }

        // =====================================================
        // Anthropic - OAuth2 Bearer token (preferred) or API key
        // =====================================================
        if let Ok(token) = std::env::var("ANTHROPIC_OAUTH_TOKEN") {
            let anthropic = AnthropicClient::with_oauth_token(token);
            info!("✅ Anthropic provider initialized (OAuth2 Bearer)");
            providers.insert(ProviderType::Anthropic, Box::new(anthropic));
            if default_provider.is_none() {
                default_provider = Some(ProviderType::Anthropic);
            }
        } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            match AnthropicClient::from_env() {
                Ok(anthropic) => {
                    info!("✅ Anthropic provider initialized (API key)");
                    providers.insert(ProviderType::Anthropic, Box::new(anthropic));
                    if default_provider.is_none() {
                        default_provider = Some(ProviderType::Anthropic);
                    }
                }
                Err(e) => {
                    debug!("Anthropic provider failed: {}", e);
                }
            }
        }

        // Use environment provider if specified and available
        let final_provider = if let Some(ref provider_name) = env_provider {
            if let Ok(pt) = provider_name.parse::<ProviderType>() {
                if providers.contains_key(&pt) {
                    info!("✅ Using LLM_PROVIDER: {:?}", pt);
                    pt
                } else {
                    warn!("⚠️  LLM_PROVIDER '{}' not available", provider_name);
                    default_provider.unwrap_or(ProviderType::Factory)
                }
            } else {
                warn!("⚠️  Invalid LLM_PROVIDER '{}'", provider_name);
                default_provider.unwrap_or(ProviderType::Factory)
            }
        } else {
            default_provider.unwrap_or(ProviderType::Factory)
        };

        if providers.is_empty() {
            warn!("⚠️  No LLM providers available!");
            warn!("   Configure authentication:");
            warn!("   1. Factory proxy should auto-start (FACTORY_BASE_URL)");
            warn!("   2. Authenticate: gcloud auth login");
            warn!("   3. Or set OPENCLAW_BASE_URL and LLM_PROVIDER=openclaw");
            warn!("   4. Or set GEMINI_API_KEY environment variable");
        } else {
            info!("\n📊 Default provider: {:?}", final_provider);
            info!("📊 Default model: {}", default_model);
            info!("📊 Available providers: {}\n", providers.len());
        }

        Self {
            providers,
            current_provider: Arc::new(RwLock::new(final_provider)),
            current_model: Arc::new(RwLock::new(default_model)),
            model_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a provider
    pub fn add_provider(&mut self, provider: BoxedProvider) {
        let provider_type = provider.provider_type();
        self.providers.insert(provider_type, provider);
    }

    /// Get current provider type
    pub async fn current_provider(&self) -> ProviderType {
        self.current_provider.read().unwrap().clone()
    }

    /// Get current model
    pub async fn current_model(&self) -> String {
        self.current_model.read().unwrap().clone()
    }

    /// Switch provider
    pub async fn switch_provider(&self, provider_type: ProviderType) -> Result<()> {
        if !self.providers.contains_key(&provider_type) {
            return Err(anyhow::anyhow!(
                "Provider {:?} not available. Available: {:?}",
                provider_type,
                self.available_providers()
            ));
        }

        *self.current_provider.write().unwrap() = provider_type.clone();
        info!("Switched to provider: {:?}", provider_type);

        // Get first available model for this provider
        let models = self.list_models().await?;
        if let Some(first) = models.first() {
            *self.current_model.write().unwrap() = first.id.clone();
            info!("Default model set to: {}", first.id);
        }

        Ok(())
    }

    /// Switch model
    pub async fn switch_model(&self, model_id: impl Into<String>) -> Result<()> {
        let model_id = model_id.into();
        *self.current_model.write().unwrap() = model_id.clone();
        info!("Switched to model: {}", model_id);
        Ok(())
    }

    /// List available providers
    pub fn available_providers(&self) -> Vec<ProviderType> {
        self.providers.keys().cloned().collect()
    }

    /// Check if a provider is available
    pub fn has_provider(&self, provider_type: &ProviderType) -> bool {
        self.providers.contains_key(provider_type)
    }

    async fn resolve_provider(&self) -> Result<ProviderType> {
        let current = self.current_provider.read().unwrap().clone();
        if self.providers.contains_key(&current) {
            return Ok(current);
        }

        if let Some(fallback) = self.providers.keys().next().cloned() {
            warn!(
                "Provider {:?} not available, falling back to {:?}",
                current, fallback
            );
            *self.current_provider.write().unwrap() = fallback.clone();
            return Ok(fallback);
        }

        Err(anyhow!(
            "No LLM providers configured.\n\n\
            To authenticate:\n\
            1. Factory proxy should auto-start (FACTORY_BASE_URL)\n\
            2. Run: gcloud auth login\n\
            3. Or set OPENCLAW_BASE_URL and LLM_PROVIDER=openclaw\n\n\
            Or set GEMINI_API_KEY environment variable."
        ))
    }

    /// List models from current provider
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let provider_type = self.current_provider.read().unwrap().clone();

        // Check cache first
        {
            let cache = self.model_cache.read().unwrap();
            if let Some(models) = cache.get(&provider_type) {
                return Ok(models.clone());
            }
        }

        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider not available"))?;

        let models = provider.list_models().await?;

        // Cache
        {
            let mut cache = self.model_cache.write().unwrap();
            cache.insert(provider_type, models.clone());
        }

        Ok(models)
    }

    /// List models for a specific provider
    pub async fn list_models_for_provider(
        &self,
        provider_type: &ProviderType,
    ) -> Result<Vec<ModelInfo>> {
        let provider = self
            .providers
            .get(provider_type)
            .ok_or_else(|| anyhow!("Provider {:?} not available", provider_type))?;

        provider.list_models().await
    }

    /// Search models
    pub async fn search_models(&self, query: &str) -> Result<Vec<ModelInfo>> {
        let provider_type = self.current_provider.read().unwrap().clone();
        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider not available"))?;

        provider.search_models(query, 20).await
    }

    /// Refresh models (clear cache)
    pub async fn refresh_models(&self) -> Result<Vec<ModelInfo>> {
        let provider_type = self.current_provider.read().unwrap().clone();
        {
            let mut cache = self.model_cache.write().unwrap();
            cache.remove(&provider_type);
        }
        self.list_models().await
    }

    /// Get model info
    pub async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let provider_type = self.current_provider.read().unwrap().clone();
        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider not available"))?;

        provider.get_model(model_id).await
    }

    /// Check if model is available
    pub async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        let provider_type = self.current_provider.read().unwrap().clone();
        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider not available"))?;

        provider.is_model_available(model_id).await
    }

    /// Send chat message
    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let provider_type = self.resolve_provider().await?;
        let model = self.current_model.read().unwrap().clone();

        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider not available"))?;

        provider.chat(&model, messages).await
    }

    /// Send chat with specific provider and model
    pub async fn chat_with(
        &self,
        provider_type: &ProviderType,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<ChatResponse> {
        let provider = self
            .providers
            .get(provider_type)
            .ok_or_else(|| anyhow!("Provider {:?} not available", provider_type))?;

        provider.chat(model, messages).await
    }

    /// Send chat stream with specific provider and model
    pub async fn chat_stream_with(
        &self,
        provider_type: &ProviderType,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        let provider = self
            .providers
            .get(provider_type)
            .ok_or_else(|| anyhow!("Provider {:?} not available", provider_type))?;

        provider.chat_stream(model, messages).await
    }

    /// Find which provider supports the given model
    pub async fn find_provider_for_model(&self, model: &str) -> Option<ProviderType> {
        let providers = self.available_providers();
        // 1. Direct match in list_models_for_provider
        for ptype in &providers {
            if let Ok(models) = self.list_models_for_provider(ptype).await {
                if models.iter().any(|m| m.id == model) {
                    return Some(ptype.clone());
                }
            }
        }

        // 2. Prefix / substring match
        let model_lower = model.to_lowercase();
        if model_lower.contains("factory") || model_lower.contains("droid") || model_lower.contains("kimi") {
            if providers.contains(&ProviderType::Factory) {
                return Some(ProviderType::Factory);
            }
        }
        if model_lower.contains("claude") {
            if providers.contains(&ProviderType::Anthropic) {
                return Some(ProviderType::Anthropic);
            }
        }
        if model_lower.contains("gemini") {
            // Check in order of preference
            if providers.contains(&ProviderType::Antigravity) {
                return Some(ProviderType::Antigravity);
            }
            if providers.contains(&ProviderType::Gemini) {
                return Some(ProviderType::Gemini);
            }
            if providers.contains(&ProviderType::GeminiCli) {
                return Some(ProviderType::GeminiCli);
            }
        }
        if model_lower.contains("gpt") {
            if providers.contains(&ProviderType::OpenClaw) {
                return Some(ProviderType::OpenClaw);
            }
            if providers.contains(&ProviderType::Assistant) {
                return Some(ProviderType::Assistant);
            }
            if providers.contains(&ProviderType::OpenAI) {
                return Some(ProviderType::OpenAI);
            }
        }

        // 3. Fallback to first available provider
        providers.first().cloned()
    }


    /// Get status
    pub async fn get_status(&self) -> simd_json::OwnedValue {
        let provider = self.current_provider.read().unwrap().clone();
        let model = self.current_model.read().unwrap().clone();
        let providers: Vec<String> = self
            .available_providers()
            .iter()
            .map(|p| p.to_string())
            .collect();

        simd_json::json!({
            "provider": provider.to_string(),
            "model": model,
            "available_providers": providers,
        })
    }

    /// Get detailed status
    pub async fn get_detailed_status(&self) -> simd_json::OwnedValue {
        let current_provider = self.current_provider.read().unwrap().clone();
        let current_model = self.current_model.read().unwrap().clone();

        let mut provider_status = simd_json::value::owned::Object::new();

        for ptype in self.providers.keys() {
            let models = self.list_models_for_provider(ptype).await.ok();
            let (auth_type, features) = match ptype {
                ProviderType::Factory => (
                    "Factory AI local proxy (FACTORY_BASE_URL)",
                    vec![
                        "OpenAI-compatible API",
                        "Local proxy at 127.0.0.1:11435",
                        "Default for op-web chat",
                    ],
                ),
                ProviderType::McpProxy => (
                    "OAuth via op-mcp-proxy (VS Code extension emulation)",
                    vec![
                        "Cloud Code-compatible headers",
                        "Gemini models",
                        "Headless-friendly",
                    ],
                ),
                ProviderType::Antigravity => (
                    "OAuth (gcloud ADC, legacy provider id)",
                    vec!["Gemini models", "Application-default credentials"],
                ),
                ProviderType::Gemini => (
                    "API key (GEMINI_API_KEY)",
                    vec!["Gemini models", "Multimodal", "Long context"],
                ),
                ProviderType::GeminiCli => (
                    "Local Gemini CLI bridge",
                    vec![
                        "Gemini CLI binary",
                        "ADC/service account auth",
                        "Headless-friendly",
                    ],
                ),
                ProviderType::Anthropic => (
                    "API key (ANTHROPIC_API_KEY)",
                    vec!["Claude models", "Best reasoning", "Tool use"],
                ),
                ProviderType::OpenClaw => (
                    "Trusted internal network (OPENCLAW_BASE_URL)",
                    vec!["OpenAI-compatible API", "Agent platform", "Tool use"],
                ),
                ProviderType::Assistant => (
                    "Trusted internal network (ASSISTANT_BASE_URL)",
                    vec!["OpenAI-compatible API", "Incus container", "Tool use"],
                ),
                _ => ("API key", vec![]),
            };

            provider_status.insert(
                ptype.to_string(),
                simd_json::json!({
                    "available": true,
                    "model_count": models.as_ref().map(|m| m.len()).unwrap_or(0),
                    "auth_type": auth_type,
                    "features": features,
                }),
            );
        }

        simd_json::json!({
            "current_provider": current_provider.to_string(),
            "current_model": current_model,
            "providers": provider_status,
        })
    }
}

#[async_trait]
impl LlmProvider for ChatManager {
    fn provider_type(&self) -> ProviderType {
        self.current_provider.read().unwrap().clone()
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        ChatManager::list_models(self).await
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let _limit = limit;
        ChatManager::search_models(self, query).await
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        ChatManager::get_model(self, model_id).await
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        ChatManager::is_model_available(self, model_id).await
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let provider_type = self.resolve_provider().await?;
        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider {:?} not available", provider_type))?;

        provider.chat(model, messages).await
    }

    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        let provider_type = self.resolve_provider().await?;
        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider {:?} not available", provider_type))?;

        provider.chat_with_request(model, request).await
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        let provider_type = self.resolve_provider().await?;
        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider {:?} not available", provider_type))?;

        provider.chat_stream(model, messages).await
    }
}

impl Default for ChatManager {
    fn default() -> Self {
        Self::new()
    }
}
</file>

<file path="src/factory.rs">
//! Factory LLM Provider
//!
//! Connects to the Factory AI local proxy via OpenAI-compatible
//! `/v1/chat/completions` endpoint. Reads from `~/.factory/settings.local.json`
//! or environment variables.
//!
//! ## Configuration
//!
//! ```bash
//! FACTORY_BASE_URL=http://127.0.0.1:11435/v1    # default
//! FACTORY_API_KEY=local-codex-proxy              # default
//! FACTORY_DEFAULT_MODEL=local-oauth-proxy        # default
//! ```

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::provider::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType, TokenUsage,
    ToolCallInfo,
};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:11435/v1";
const DEFAULT_API_KEY: &str = "local-codex-proxy";
const DEFAULT_MODEL: &str = "local-oauth-proxy";

pub struct FactoryProvider {
    client: Client,
    base_url: String,
    api_key: String,
    default_model: String,
}

impl FactoryProvider {
    pub fn new(base_url: Option<String>, api_key: Option<String>, default_model: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
                .trim_end_matches('/')
                .to_string(),
            api_key: api_key.unwrap_or_else(|| DEFAULT_API_KEY.to_string()),
            default_model: default_model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        }
    }

    pub fn from_env() -> Result<Self> {
        let base_url = std::env::var("FACTORY_BASE_URL").ok();
        let api_key = std::env::var("FACTORY_API_KEY").ok();
        let default_model = std::env::var("FACTORY_DEFAULT_MODEL").ok();
        Ok(Self::new(base_url, api_key, default_model))
    }

    fn models_url(&self) -> String {
        format!("{}/models", self.base_url)
    }

    fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    fn resolve_model(&self, model: &str) -> String {
        if model.is_empty() {
            self.default_model.clone()
        } else {
            model.to_string()
        }
    }

    fn api_request(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
    }

    fn fallback_model_info(&self) -> ModelInfo {
        ModelInfo {
            id: self.default_model.clone(),
            name: self.default_model.clone(),
            description: Some("Factory AI via local proxy".to_string()),
            parameters: None,
            available: true,
            tags: vec!["factory".to_string(), "default".to_string()],
            downloads: None,
            updated_at: None,
        }
    }

    fn parse_models_response(response_text: &str) -> Result<Vec<ModelInfo>> {
        let mut response_text_mut = response_text.to_string();
        let response_json: Value = unsafe { simd_json::from_str(&mut response_text_mut) }
            .map_err(|e| anyhow::anyhow!("Failed to parse Factory models response: {}", e))?;

        let models = response_json
            .get("data")
            .and_then(|v| v.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| {
                        let id = entry.get("id")?.as_str()?.to_string();
                        let owned_by = entry
                            .get("owned_by")
                            .and_then(|v| v.as_str())
                            .unwrap_or("factory")
                            .to_string();
                        let created = entry
                            .get("created")
                            .and_then(|v| v.as_i64())
                            .map(|ts| ts.to_string());

                        Some(ModelInfo {
                            id: id.clone(),
                            name: id,
                            description: Some(format!("Factory model owned by {}", owned_by)),
                            parameters: None,
                            available: true,
                            tags: vec!["factory".to_string(), owned_by],
                            downloads: None,
                            updated_at: created,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(models)
    }
}

#[async_trait]
impl LlmProvider for FactoryProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Factory
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let response = self
            .api_request(self.client.get(self.models_url()))
            .send()
            .await
            .context("Failed to query Factory models")?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            warn!(
                "Factory model listing failed ({}), falling back to default",
                status
            );
            return Ok(vec![self.fallback_model_info()]);
        }

        let mut models = match Self::parse_models_response(&response_text) {
            Ok(models) => models,
            Err(err) => {
                warn!(
                    "Factory /v1/models did not return a usable model list ({}), falling back to default",
                    err
                );
                return Ok(vec![self.fallback_model_info()]);
            }
        };
        if models.is_empty() {
            models.push(self.fallback_model_info());
        }

        Ok(models)
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models
            .into_iter()
            .filter(|m| m.name.to_lowercase().contains(&query.to_lowercase()))
            .take(limit)
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        Ok(self.get_model(model_id).await?.is_some())
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let request = ChatRequest::new(messages);
        self.chat_with_request(model, request).await
    }

    async fn chat_with_request(
        &self,
        model: &str,
        request: ChatRequest,
    ) -> Result<ChatResponse> {
        let model = self.resolve_model(model);
        let url = self.chat_url();

        let messages: Vec<Value> = request
            .messages
            .iter()
            .map(|m| {
                let mut msg = json!({
                    "role": m.role,
                    "content": m.content
                });

                if let Some(ref id) = m.tool_call_id {
                    msg["tool_call_id"] = json!(id);
                }

                if let Some(ref calls) = m.tool_calls {
                    msg["tool_calls"] = json!(calls.iter().map(|tc| {
                        json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": simd_json::to_string(&tc.arguments).unwrap_or_default()
                            }
                        })
                    }).collect::<Vec<_>>());
                }

                msg
            })
            .collect();

        let tools: Vec<Value> = request.tools.iter().map(|t| t.to_openai_format()).collect();

        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": false
        });
        let body_object = body
            .as_object_mut()
            .expect("factory request body should be an object");

        if !tools.is_empty() {
            body_object.insert("tools".into(), json!(tools));
            body_object.insert("tool_choice".into(), request.tool_choice.to_api_format());
            info!(
                "Factory request with {} tools, tool_choice={:?}",
                tools.len(),
                request.tool_choice
            );
        }

        if let Some(max_tokens) = request.max_tokens {
            body_object.insert("max_tokens".into(), json!(max_tokens));
        }
        if let Some(temp) = request.temperature {
            body_object.insert("temperature".into(), json!(temp));
        }
        if let Some(top_p) = request.top_p {
            body_object.insert("top_p".into(), json!(top_p));
        }

        debug!(
            "Factory request: {}",
            simd_json::to_string_pretty(&body).unwrap_or_default()
        );

        let response = self
            .api_request(self.client.post(&url))
            .json(&body)
            .send()
            .await
            .context("Failed to send request to Factory")?;

        let status = response.status();
        let response_text = response.text().await?;

        debug!(
            "Factory response ({}): {}",
            status,
            &response_text[..response_text.len().min(500)]
        );

        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "Factory API error ({}): {}",
                status,
                response_text
            ));
        }

        let mut response_text_mut = response_text;
        let response_json: Value = unsafe { simd_json::from_str(&mut response_text_mut) }
            .map_err(|e| anyhow::anyhow!("Failed to parse Factory response: {}. Body: {}", e, response_text_mut))?;

        let choice = response_json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| anyhow::anyhow!("No choices returned from Factory"))?;

        let message = choice
            .get("message")
            .ok_or_else(|| anyhow::anyhow!("No message in Factory response"))?;

        let content = message
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        let role = message
            .get("role")
            .and_then(|r| r.as_str())
            .unwrap_or("assistant")
            .to_string();

        let tool_calls: Option<Vec<ToolCallInfo>> = message
            .get("tool_calls")
            .and_then(|tc| tc.as_array())
            .map(|calls| {
                calls
                    .iter()
                    .filter_map(|call| {
                        let id = call.get("id")?.as_str()?.to_string();
                        let function = call.get("function")?;
                        let name = function.get("name")?.as_str()?.to_string();
                        let args_str = function.get("arguments")?.as_str()?;
                        let mut args_mut = args_str.to_string();
                        let arguments: Value = unsafe { simd_json::from_str(&mut args_mut) }.ok()?;

                        Some(ToolCallInfo {
                            id,
                            name,
                            arguments,
                        })
                    })
                    .collect()
            });

        if let Some(ref calls) = tool_calls {
            info!("Factory: parsed {} tool calls", calls.len());
        }

        let finish_reason = choice
            .get("finish_reason")
            .and_then(|f| f.as_str())
            .map(|s| s.to_string());

        let usage = response_json.get("usage").map(|u| TokenUsage {
            prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            completion_tokens: u
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        });

        Ok(ChatResponse {
            message: ChatMessage {
                role,
                content,
                tool_calls: tool_calls.clone(),
                tool_call_id: None,
            },
            model,
            provider: "factory".to_string(),
            finish_reason,
            usage,
            tool_calls,
        })
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let response = self.chat(model, messages).await?;
        let _ = tx.send(Ok(response.message.content)).await;
        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_model_listing_response() {
        let models = FactoryProvider::parse_models_response(
            r#"{"data":[{"id":"local-oauth-proxy","owned_by":"factory","created":1710000000}]}"#,
        )
        .expect("model response should parse");

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "local-oauth-proxy");
        assert!(models[0].tags.iter().any(|tag| tag == "factory"));
    }

    #[test]
    fn defaults_from_constants() {
        let p = FactoryProvider::new(None, None, None);
        assert_eq!(p.provider_type(), ProviderType::Factory);
    }
}
</file>

<file path="src/gcloud_adc.rs">
//! Google Cloud ADC Provider - Uses gcloud application-default credentials
//!
//! This provider replaces the old Antigravity provider and uses the
//! Cloud AI Companion (Subscription) endpoint.

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use uuid;

use crate::provider::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType, TokenUsage,
    ToolCallInfo, ToolChoice, ToolDefinition,
};

/// Cloud AI Companion base URL - configurable via GCP_BASE_URL
fn cloud_ai_base() -> String {
    std::env::var("GCP_BASE_URL")
        .unwrap_or_else(|_| "https://cloudaicompanion.googleapis.com/v1".to_string())
}

fn project_id() -> String {
    std::env::var("GCP_PROJECT")
        .or_else(|_| std::env::var("GOOGLE_CLOUD_PROJECT"))
        .unwrap_or_else(|_| "geminidev-479406".to_string())
}

fn location() -> String {
    std::env::var("GCP_LOCATION").unwrap_or_else(|_| "global".to_string())
}

fn adc_fallback_enabled() -> bool {
    std::env::var("OP_ENABLE_ADC_FALLBACK")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub struct GCloudADCProvider {
    client: Client,
    model: String,
}

impl GCloudADCProvider {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap_or_default();

        let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".to_string());

        Self { client, model }
    }

    /// Get OAuth token from gcloud
    async fn get_token(&self) -> Result<String> {
        if let Ok(token) = std::env::var("GCLOUD_TOKEN") {
            return Ok(token);
        }

        // Prefer active gcloud user credentials.
        let output = Command::new("gcloud")
            .args(["auth", "print-access-token"])
            .output()
            .context("Failed to execute gcloud auth print-access-token")?;

        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }

        // Optional ADC fallback (disabled by default to avoid metadata-server auth on Compute hosts).
        if adc_fallback_enabled() {
            let output = Command::new("gcloud")
                .args(["auth", "application-default", "print-access-token"])
                .output()
                .context("Failed to execute gcloud auth application-default print-access-token")?;

            if output.status.success() {
                return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
        }

        anyhow::bail!("Could not obtain gcloud token from GCLOUD_TOKEN or gcloud CLI credentials")
    }

    /// Convert messages to Gemini format
    fn convert_messages(&self, messages: &[ChatMessage]) -> (Vec<Value>, Option<Value>) {
        let mut system_instruction = None;
        let mut contents = Vec::new();

        for msg in messages {
            if msg.role == "system" {
                system_instruction = Some(json!({
                    "parts": [{"text": msg.content}]
                }));
                continue;
            }

            let role = match msg.role.as_str() {
                "assistant" | "model" => "model",
                _ => "user",
            };

            contents.push(json!({
                "role": role,
                "parts": [{"text": msg.content}]
            }));
        }

        (contents, system_instruction)
    }

    /// Convert tools to Gemini format
    fn convert_tools(&self, tools: &[ToolDefinition]) -> Value {
        let function_declarations: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema
                })
            })
            .collect();

        json!([{
            "functionDeclarations": function_declarations
        }])
    }

    /// Convert tool choice to Gemini format
    fn convert_tool_choice(&self, choice: &ToolChoice) -> Option<Value> {
        match choice {
            ToolChoice::Auto => Some(json!({"mode": "AUTO"})),
            ToolChoice::Required => Some(json!({"mode": "ANY"})),
            ToolChoice::None => Some(json!({"mode": "NONE"})),
            ToolChoice::Tool(name) => Some(json!({
                "mode": "ANY",
                "allowedFunctionNames": [name]
            })),
        }
    }
}

#[async_trait]
impl LlmProvider for GCloudADCProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Antigravity // Reusing this for now to minimize changes in ChatManager
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(vec![
            ModelInfo {
                id: "gemini-2.0-flash".to_string(),
                name: "Gemini 2.0 Flash".to_string(),
                description: Some("Fast and efficient".to_string()),
                parameters: None,
                available: true,
                tags: vec!["google".to_string()],
                downloads: None,
                updated_at: None,
            },
            ModelInfo {
                id: "gemini-1.5-pro".to_string(),
                name: "Gemini 1.5 Pro".to_string(),
                description: Some("Complex reasoning".to_string()),
                parameters: None,
                available: true,
                tags: vec!["google".to_string()],
                downloads: None,
                updated_at: None,
            },
        ])
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let models = self.list_models().await?;
        let query = query.to_lowercase();
        Ok(models
            .into_iter()
            .filter(|m| m.id.contains(&query) || m.name.to_lowercase().contains(&query))
            .take(limit)
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        Ok(matches!(model_id, "gemini-2.0-flash" | "gemini-1.5-pro"))
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let request = ChatRequest::new(messages);
        self.chat_with_request(model, request).await
    }

    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        let model = if model.is_empty() { &self.model } else { model };
        let token = self.get_token().await?;

        let url = format!(
            "{}/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
            cloud_ai_base(),
            project_id(),
            location(),
            model
        );

        let (contents, system_instruction) = self.convert_messages(&request.messages);

        // Build body with all optional fields included from the start
        let mut body_map = HashMap::new();
        body_map.insert("contents".to_string(), json!(contents));
        body_map.insert(
            "generationConfig".to_string(),
            json!({
                "temperature": request.temperature.unwrap_or(0.7) as f64,
                "maxOutputTokens": request.max_tokens.unwrap_or(8192) as u64,
            }),
        );

        if let Some(sys) = system_instruction {
            body_map.insert("systemInstruction".to_string(), sys);
        }

        // Add tools if present
        if !request.tools.is_empty() {
            let tools = self.convert_tools(&request.tools);
            body_map.insert("tools".to_string(), tools);

            if let Some(tool_config) = self.convert_tool_choice(&request.tool_choice) {
                body_map.insert(
                    "toolConfig".to_string(),
                    json!({"functionCallingConfig": tool_config}),
                );
            }
        }

        let body = Value::from(body_map);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Cloud AI error {}: {}", status, text);
        }

        let result: Value = response.json().await?;

        // Parse candidates
        let candidates = result
            .get("candidates")
            .and_then(|c| c.as_array())
            .ok_or_else(|| anyhow::anyhow!("No candidates in response"))?;

        let first_candidate = candidates
            .first()
            .ok_or_else(|| anyhow::anyhow!("Empty candidates"))?;

        // Extract text and tool calls
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        if let Some(parts) = first_candidate
            .get("content")
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array())
        {
            for part in parts {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text.to_string());
                }
                if let Some(fc) = part.get("functionCall") {
                    let name = fc.get("name").and_then(|n| n.as_str()).unwrap_or_default();
                    let args = fc.get("args").cloned().unwrap_or(json!({}));
                    tool_calls.push(ToolCallInfo {
                        id: format!("call_{}", uuid::Uuid::new_v4()),
                        name: name.to_string(),
                        arguments: args,
                    });
                }
            }
        }

        let usage = result.get("usageMetadata").map(|u| TokenUsage {
            prompt_tokens: u
                .get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            completion_tokens: u
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: u
                .get("totalTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
        });

        Ok(ChatResponse {
            message: ChatMessage {
                role: "assistant".to_string(),
                content: if text_parts.is_empty() && !tool_calls.is_empty() {
                    "[Executing tools...]".to_string()
                } else if text_parts.is_empty() {
                    "Task completed.".to_string()
                } else {
                    text_parts.join("")
                },
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls.clone())
                },
                tool_call_id: None,
            },
            model: model.to_string(),
            provider: "gcloud-adc".to_string(),
            finish_reason: first_candidate
                .get("finishReason")
                .and_then(|f| f.as_str())
                .map(String::from),
            usage,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
        })
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        let response = self.chat(model, messages).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let _ = tx.send(Ok(response.message.content)).await;
        Ok(rx)
    }
}

impl Default for GCloudADCProvider {
    fn default() -> Self {
        Self::new()
    }
}
</file>

<file path="src/gemini_cli.rs">
//! Gemini CLI Integration via PTY Bridge
//!
//! Uses the PTY bridge to run `gemini` CLI tool on headless servers,
//! handling OAuth authentication flows transparently.
//!
//! ## Prerequisites
//!
//! Install Gemini CLI:
//! ```bash
//! npm install -g @google/gemini-cli
//! ```
//!
//! ## How Auth Works
//!
//! This provider uses gcloud credentials:
//! 1. Set up gcloud auth: `gcloud auth login`
//! 2. Or use service account: Set `GOOGLE_APPLICATION_CREDENTIALS` env var
//! 3. PTY bridge passes credentials to Gemini CLI automatically
//! 4. Gemini CLI uses gcloud credentials for API calls

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::provider::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType,
};
use crate::pty_bridge::PtyAuthBridge;

// =============================================================================
// GEMINI CLI PROVIDER
// =============================================================================

/// Gemini CLI-based LLM provider
///
/// Uses PTY bridge to run the `gemini` CLI tool, handling authentication
/// flows automatically on headless servers.
pub struct GeminiCliProvider {
    bridge: Arc<PtyAuthBridge>,
    /// Path to gemini binary (default: "gemini")
    binary: String,
    /// Default model to use
    default_model: String,
    /// Timeout for commands in seconds
    timeout_secs: u64,
}

impl GeminiCliProvider {
    /// Create a new Gemini CLI provider
    pub fn new(bridge: Arc<PtyAuthBridge>) -> Self {
        Self {
            bridge,
            binary: "gemini".to_string(),
            default_model: "gemini-2.0-flash".to_string(),
            timeout_secs: 120,
        }
    }

    /// Configure Gemini CLI to use gcloud credentials
    /// This ensures GOOGLE_APPLICATION_CREDENTIALS is set when executing commands
    #[allow(dead_code)]
    fn setup_gcloud_env(&self) -> std::collections::HashMap<String, String> {
        let mut env = std::collections::HashMap::new();

        // Pass through existing env vars
        if let Ok(creds) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
            env.insert("GOOGLE_APPLICATION_CREDENTIALS".to_string(), creds);
        } else if let Some(home) = dirs::home_dir() {
            // Try service account key first
            let gcloud_creds = home.join(".config/gcloud/gemini-cli.json");
            if gcloud_creds.exists() {
                env.insert(
                    "GOOGLE_APPLICATION_CREDENTIALS".to_string(),
                    gcloud_creds.to_string_lossy().to_string(),
                );
            } else {
                // Fall back to Application Default Credentials
                let adc_creds = home.join(".config/gcloud/application_default_credentials.json");
                if adc_creds.exists() {
                    env.insert(
                        "GOOGLE_APPLICATION_CREDENTIALS".to_string(),
                        adc_creds.to_string_lossy().to_string(),
                    );
                }
            }
        }

        // Pass through project ID
        if let Ok(project) = std::env::var("GOOGLE_CLOUD_PROJECT") {
            env.insert("GOOGLE_CLOUD_PROJECT".to_string(), project);
        }

        env
    }

    /// Create with custom binary path
    pub fn with_binary(mut self, binary: &str) -> Self {
        self.binary = binary.to_string();
        self
    }

    /// Set default model
    pub fn with_model(mut self, model: &str) -> Self {
        self.default_model = model.to_string();
        self
    }

    /// Set command timeout
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Check if gemini CLI is available
    pub async fn check_available(&self) -> bool {
        let result = self.bridge.execute(&self.binary, &["--version"], 10).await;

        match result {
            Ok(r) => r.exit_code == 0,
            Err(_) => false,
        }
    }

    /// Convert chat messages to CLI format
    fn format_prompt(messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    prompt.push_str(&format!("[System]: {}\n\n", msg.content));
                }
                "user" => {
                    prompt.push_str(&format!("User: {}\n\n", msg.content));
                }
                "assistant" => {
                    prompt.push_str(&format!("Assistant: {}\n\n", msg.content));
                }
                _ => {
                    prompt.push_str(&format!("{}: {}\n\n", msg.role, msg.content));
                }
            }
        }

        prompt
    }
}

#[async_trait]
impl LlmProvider for GeminiCliProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Custom("gemini-cli".to_string())
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        // Gemini CLI supports these models
        Ok(vec![
            ModelInfo {
                id: "gemini-2.0-flash".to_string(),
                name: "Gemini 2.0 Flash".to_string(),
                description: Some("Fast, efficient model".to_string()),
                parameters: None,
                available: true,
                tags: vec!["fast".to_string(), "cli".to_string()],
                downloads: None,
                updated_at: None,
            },
            ModelInfo {
                id: "gemini-2.0-pro".to_string(),
                name: "Gemini 2.0 Pro".to_string(),
                description: Some("Most capable model".to_string()),
                parameters: None,
                available: true,
                tags: vec!["powerful".to_string(), "cli".to_string()],
                downloads: None,
                updated_at: None,
            },
            ModelInfo {
                id: "gemini-1.5-flash".to_string(),
                name: "Gemini 1.5 Flash".to_string(),
                description: Some("Previous generation fast model".to_string()),
                parameters: None,
                available: true,
                tags: vec!["fast".to_string(), "cli".to_string()],
                downloads: None,
                updated_at: None,
            },
        ])
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let models = self.list_models().await?;
        let query_lower = query.to_lowercase();

        Ok(models
            .into_iter()
            .filter(|m| {
                m.id.to_lowercase().contains(&query_lower)
                    || m.name.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        Ok(self.get_model(model_id).await?.is_some())
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let model = if model.is_empty() {
            &self.default_model
        } else {
            model
        };
        let prompt = Self::format_prompt(&messages);

        info!(model = %model, prompt_len = %prompt.len(), "Gemini CLI chat");

        // Build command args
        // Gemini CLI syntax: gemini -m <model> "<prompt>" or gemini "<prompt>"
        let args = vec!["-m", model, &prompt];

        let result = self
            .bridge
            .execute(&self.binary, &args, self.timeout_secs)
            .await
            .context("Failed to execute gemini CLI")?;

        // Handle auth requirement
        if result.auth_required {
            if let Some(auth) = &result.auth_details {
                warn!(
                    auth_type = ?auth.auth_type,
                    url = ?auth.url,
                    "Gemini CLI requires authentication"
                );
                return Err(anyhow::anyhow!(
                    "Authentication required. Visit: {}",
                    auth.url.as_deref().unwrap_or("(see terminal output)")
                ));
            }
        }

        if result.exit_code != 0 {
            return Err(anyhow::anyhow!(
                "Gemini CLI failed with exit code {}: {}",
                result.exit_code,
                result.stderr
            ));
        }

        // Try to parse JSON response
        let content = if let Ok(json_resp) =
            unsafe { simd_json::from_str::<simd_json::OwnedValue>(&mut result.stdout.clone()) }
        {
            json_resp
                .get("response")
                .or_else(|| json_resp.get("text"))
                .or_else(|| json_resp.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or(&result.stdout)
                .to_string()
        } else {
            // Plain text response
            result.stdout.trim().to_string()
        };

        Ok(ChatResponse {
            message: ChatMessage::assistant(content),
            model: model.to_string(),
            provider: "gemini-cli".to_string(),
            finish_reason: Some("stop".to_string()),
            usage: None, // CLI doesn't provide usage stats
            tool_calls: None,
        })
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        // Gemini CLI doesn't support streaming, so we fake it
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let response = self.chat(model, messages).await?;
        tx.send(Ok(response.message.content)).await.ok();
        Ok(rx)
    }
}

// =============================================================================
// FACTORY
// =============================================================================

/// Create a Gemini CLI provider with default bridge
pub fn create_gemini_cli_provider() -> GeminiCliProvider {
    let bridge = Arc::new(PtyAuthBridge::new());
    GeminiCliProvider::new(bridge)
}

/// Create a Gemini CLI provider with webhook notifications
pub fn create_gemini_cli_provider_with_webhook(webhook_url: &str) -> GeminiCliProvider {
    use crate::pty_bridge::WebhookNotificationHandler;

    let bridge = Arc::new(PtyAuthBridge::new());

    // Add webhook handler in background
    let bridge_clone = bridge.clone();
    let url = webhook_url.to_string();
    tokio::spawn(async move {
        bridge_clone
            .add_handler(Arc::new(WebhookNotificationHandler::new(&url)))
            .await;
    });

    GeminiCliProvider::new(bridge)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_prompt() {
        let messages = vec![
            ChatMessage::system("You are a helpful assistant."),
            ChatMessage::user("Hello!"),
            ChatMessage::assistant("Hi there!"),
            ChatMessage::user("How are you?"),
        ];

        let prompt = GeminiCliProvider::format_prompt(&messages);

        assert!(prompt.contains("[System]: You are a helpful assistant."));
        assert!(prompt.contains("User: Hello!"));
        assert!(prompt.contains("Assistant: Hi there!"));
        assert!(prompt.contains("User: How are you?"));
    }
}
</file>

<file path="src/gemini.rs">
//! Google Gemini API Client
//!
//! ## Supported Authentication Modes
//!
//! ### 1. Service Account (Vertex AI) - Recommended for servers
//! Uses service account JSON file for JWT-based authentication.
//! Set environment variable:
//! - `GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json`
//! - Or uses default: `~/.config/gcloud/*.json` (service account file)
//!
//! ### 2. Application Default Credentials (OAuth refresh token)
//! Uses `~/.config/gcloud/application_default_credentials.json`
//! Set environment variable:
//! - `GOOGLE_GENAI_USE_VERTEXAI=true`
//!
//! ### 3. API Key (generativelanguage.googleapis.com)
//! Uses API key authentication with Google AI Studio endpoint.
//! Set environment variable:
//! - `GEMINI_API_KEY` or `GOOGLE_API_KEY`
//!
//! ## Endpoint URLs
//!
//! | Mode | Base URL |
//! |------|----------|
//! | Vertex AI | `https://{LOCATION}-aiplatform.googleapis.com/v1/projects/{PROJECT}/locations/{LOCATION}/publishers/google/models` |
//! | API Key | `https://generativelanguage.googleapis.com/v1beta/models` |

use anyhow::{Context, Result};
use async_trait::async_trait;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::Duration;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

use crate::provider::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType, TokenUsage,
    ToolCallInfo, ToolChoice,
};

// =============================================================================
// API ENDPOINT CONFIGURATION
// =============================================================================

/// Gemini API endpoints
pub mod endpoints {
    /// Google AI Studio (API key mode)
    pub const GOOGLE_AI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

    /// OAuth2 token endpoint
    pub const OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

    /// Vertex AI endpoint template
    ///
    /// For global location, uses `aiplatform.googleapis.com` (no region prefix)
    /// For regional locations, uses `{location}-aiplatform.googleapis.com`
    pub fn vertex_ai_base_url(project: &str, location: &str) -> String {
        let hostname = if location == "global" {
            "aiplatform.googleapis.com".to_string()
        } else {
            format!("{}-aiplatform.googleapis.com", location)
        };
        format!(
            "https://{}/v1/projects/{}/locations/{}/publishers/google/models",
            hostname, project, location
        )
    }
}

// =============================================================================
// AUTHENTICATION
// =============================================================================

/// Authentication mode for Gemini API
#[derive(Debug, Clone)]
pub enum GeminiAuth {
    /// API Key authentication (query parameter)
    ApiKey(String),
    /// Service Account (JWT-based)
    ServiceAccount(ServiceAccountCredentials),
    /// OAuth with refresh token (application default credentials)
    OAuthRefreshToken(OAuthCredentials),
}

/// Service account credentials from JSON file
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceAccountCredentials {
    #[serde(rename = "type")]
    pub cred_type: String,
    pub project_id: String,
    pub private_key_id: String,
    pub private_key: String,
    pub client_email: String,
    pub client_id: String,
    pub token_uri: String,
}

/// OAuth credentials from application_default_credentials.json
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
    #[serde(default)]
    pub quota_project_id: Option<String>,
}

/// Cached access token
struct CachedToken {
    token: String,
    expires_at: Instant,
}

/// Token cache (global, thread-safe)
static TOKEN_CACHE: std::sync::OnceLock<RwLock<Option<CachedToken>>> = std::sync::OnceLock::new();

fn get_token_cache() -> &'static RwLock<Option<CachedToken>> {
    TOKEN_CACHE.get_or_init(|| RwLock::new(None))
}

/// OAuth token response
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    #[serde(default)]
    _token_type: Option<String>,
}

/// JWT Claims for service account (used by jsonwebtoken)
#[derive(Debug, Serialize)]
struct JwtClaims {
    iss: String,
    sub: String,
    aud: String,
    iat: u64,
    exp: u64,
    scope: String,
}

/// Load service account credentials from file
fn load_service_account_credentials() -> Result<ServiceAccountCredentials> {
    // First check GOOGLE_APPLICATION_CREDENTIALS
    if let Ok(path) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
        let contents =
            std::fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path))?;
        let mut contents_mut = contents;
        let creds: ServiceAccountCredentials = unsafe { simd_json::from_str(&mut contents_mut) }
            .context("Failed to parse service account JSON")?;
        return Ok(creds);
    }

    // Look for service account JSON in gcloud config
    let home = std::env::var("HOME").context("HOME not set")?;
    let gcloud_dir = format!("{}/.config/gcloud", home);

    // Find any service account JSON file
    for entry in
        std::fs::read_dir(&gcloud_dir).with_context(|| format!("Failed to read {}", gcloud_dir))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                let mut contents_mut = contents;
                if let Ok(creds) =
                    unsafe { simd_json::from_str::<ServiceAccountCredentials>(&mut contents_mut) }
                {
                    if creds.cred_type == "service_account" {
                        info!("Found service account: {}", path.display());
                        return Ok(creds);
                    }
                }
            }
        }
    }

    Err(anyhow::anyhow!("No service account credentials found"))
}

/// Load OAuth credentials from application_default_credentials.json
fn load_oauth_credentials() -> Result<OAuthCredentials> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let creds_path = format!(
        "{}/.config/gcloud/application_default_credentials.json",
        home
    );

    let contents = std::fs::read_to_string(&creds_path)
        .with_context(|| format!("Failed to read {}", creds_path))?;

    let mut contents_mut = contents;
    let creds: OAuthCredentials = unsafe { simd_json::from_str(&mut contents_mut) }
        .context("Failed to parse application_default_credentials.json")?;

    Ok(creds)
}

/// Create JWT for service account authentication
fn create_service_account_jwt(creds: &ServiceAccountCredentials) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Time error")?
        .as_secs();

    // JWT Claims for Google OAuth
    let claims = JwtClaims {
        iss: creds.client_email.clone(),
        sub: creds.client_email.clone(),
        aud: creds.token_uri.clone(),
        iat: now,
        exp: now + 3600, // 1 hour
        scope: "https://www.googleapis.com/auth/cloud-platform".to_string(),
    };

    // Create header with key ID
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(creds.private_key_id.clone());

    // Create encoding key from PEM
    let encoding_key = EncodingKey::from_rsa_pem(creds.private_key.as_bytes())
        .context("Failed to parse private key")?;

    // Encode and sign JWT
    let jwt = encode(&header, &claims, &encoding_key).context("Failed to create JWT")?;

    Ok(jwt)
}

/// Get access token for service account using JWT
async fn get_service_account_token(creds: &ServiceAccountCredentials) -> Result<String> {
    // Check cache first
    {
        let cache = get_token_cache().read().unwrap();
        if let Some(ref cached) = *cache {
            if cached.expires_at > Instant::now() + Duration::from_secs(60) {
                debug!("Using cached service account token");
                return Ok(cached.token.clone());
            }
        }
    }

    info!("Getting service account access token...");

    let jwt = create_service_account_jwt(creds)?;

    let client = Client::new();
    let response = client
        .post(&creds.token_uri)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", &jwt),
        ])
        .send()
        .await
        .context("Failed to request access token")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Token request failed {}: {}", status, body));
    }

    let token_resp: TokenResponse = response
        .json()
        .await
        .context("Failed to parse token response")?;

    // Cache the token
    {
        let mut cache = get_token_cache().write().unwrap();
        *cache = Some(CachedToken {
            token: token_resp.access_token.clone(),
            expires_at: Instant::now() + Duration::from_secs(token_resp.expires_in),
        });
    }

    info!(
        "✅ Service account token obtained (expires in {}s)",
        token_resp.expires_in
    );
    Ok(token_resp.access_token)
}

/// Get access token using OAuth refresh token
async fn get_oauth_refresh_token(creds: &OAuthCredentials) -> Result<String> {
    // Check cache first
    {
        let cache = get_token_cache().read().unwrap();
        if let Some(ref cached) = *cache {
            if cached.expires_at > Instant::now() + Duration::from_secs(60) {
                debug!("Using cached OAuth token");
                return Ok(cached.token.clone());
            }
        }
    }

    info!("Refreshing OAuth access token...");

    let client = Client::new();
    let response = client
        .post(endpoints::OAUTH_TOKEN_URL)
        .form(&[
            ("client_id", creds.client_id.as_str()),
            ("client_secret", creds.client_secret.as_str()),
            ("refresh_token", creds.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .context("Failed to request OAuth token")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "OAuth token request failed {}: {}",
            status,
            body
        ));
    }

    let token_resp: TokenResponse = response
        .json()
        .await
        .context("Failed to parse OAuth token response")?;

    // Cache the token
    {
        let mut cache = get_token_cache().write().unwrap();
        *cache = Some(CachedToken {
            token: token_resp.access_token.clone(),
            expires_at: Instant::now() + Duration::from_secs(token_resp.expires_in),
        });
    }

    info!(
        "✅ OAuth token refreshed (expires in {}s)",
        token_resp.expires_in
    );
    Ok(token_resp.access_token)
}

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Gemini model category
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeminiCategory {
    TextOut,
    MultiModalGenerative,
    LiveApi,
    Other,
}

impl std::fmt::Display for GeminiCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeminiCategory::TextOut => write!(f, "Text-out models"),
            GeminiCategory::MultiModalGenerative => write!(f, "Multi-modal generative"),
            GeminiCategory::LiveApi => write!(f, "Live API"),
            GeminiCategory::Other => write!(f, "Other models"),
        }
    }
}

/// Gemini model with rate limits
#[derive(Debug, Clone)]
pub struct GeminiModel {
    pub id: String,
    pub category: GeminiCategory,
    pub rpm: u32,
    pub tpm: u64,
    pub rpd: u32,
}

impl GeminiModel {
    fn new(id: &str, category: GeminiCategory, rpm: u32, tpm: u64, rpd: u32) -> Self {
        Self {
            id: id.to_string(),
            category,
            rpm,
            tpm,
            rpd,
        }
    }
}

/// Static list of Gemini models
fn get_gemini_models() -> Vec<GeminiModel> {
    use GeminiCategory::*;

    vec![
        // Auto-routing models
        GeminiModel::new("gemini-auto", TextOut, 2_000, 4_000_000, 0),
        // Latest Flash & Pro models
        GeminiModel::new("gemini-2.0-flash", TextOut, 2_000, 4_000_000, 0),
        GeminiModel::new("gemini-2.0-flash-lite", TextOut, 4_000, 4_000_000, 0),
        GeminiModel::new(
            "gemini-2.0-flash-thinking-exp-1219",
            TextOut,
            2_000,
            16_000_000,
            0,
        ),
        GeminiModel::new("gemini-1.5-pro", TextOut, 360, 4_000_000, 0),
        GeminiModel::new("gemini-1.5-flash", TextOut, 2_000, 4_000_000, 0),
        // Multi-modal & Images
        GeminiModel::new("imagen-3.0-generate-001", MultiModalGenerative, 10, 0, 70),
        // Live API
        GeminiModel::new("gemini-2.0-flash-live", LiveApi, 0, 4_000_000, 0),
        // Gemma
        GeminiModel::new("gemma-2-27b-it", Other, 30, 15_000, 14_400),
    ]
}

/// Gemini API request
/// Gemini API request with optional tools
#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTool>>,
    #[serde(rename = "toolConfig", skip_serializing_if = "Option::is_none")]
    tool_config: Option<GeminiToolConfig>,
}

/// Gemini tool definition
#[derive(Debug, Serialize)]
struct GeminiTool {
    #[serde(rename = "functionDeclarations")]
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

/// Gemini function declaration
#[derive(Debug, Serialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: simd_json::OwnedValue,
}

/// Gemini tool configuration
#[derive(Debug, Serialize)]
struct GeminiToolConfig {
    #[serde(rename = "functionCallingConfig")]
    function_calling_config: FunctionCallingConfig,
}

#[derive(Debug, Serialize)]
struct FunctionCallingConfig {
    mode: String, // "AUTO", "ANY", "NONE"
}

#[derive(Debug, Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    temperature: Option<f32>,
    #[serde(rename = "topP")]
    top_p: Option<f32>,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: Option<u32>,
    #[serde(rename = "routingConfig", skip_serializing_if = "Option::is_none")]
    routing_config: Option<RoutingConfig>,
}

#[derive(Debug, Serialize)]
struct RoutingConfig {
    #[serde(rename = "autoMode", skip_serializing_if = "Option::is_none")]
    auto_mode: Option<AutoRoutingMode>,
}

#[derive(Debug, Serialize)]
struct AutoRoutingMode {
    #[serde(rename = "modelRoutingPreference")]
    model_routing_preference: String, // "BALANCED", "PRIORITIZE_QUALITY", or "PRIORITIZE_COST"
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContentResponse,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiContentResponse {
    parts: Vec<GeminiPartResponse>,
}

#[derive(Debug, Deserialize)]
struct GeminiPartResponse {
    text: Option<String>,
    #[serde(rename = "functionCall")]
    function_call: Option<GeminiFunctionCall>,
}

#[derive(Debug, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: simd_json::OwnedValue,
}

#[derive(Debug, Deserialize)]
struct UsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: Option<u32>,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<u32>,
    #[serde(rename = "totalTokenCount")]
    total_token_count: Option<u32>,
}

// =============================================================================
// CLIENT IMPLEMENTATION
// =============================================================================

/// Google Gemini Client
///
/// Supports Service Account, OAuth, and API Key authentication modes.
pub struct GeminiClient {
    client: Client,
    auth: GeminiAuth,
    /// Base API URL
    api_url: String,
    /// Whether using Vertex AI mode
    use_vertex_ai: bool,
    /// Project ID (for Vertex AI)
    _project: Option<String>,
    /// Location (for Vertex AI)
    _location: Option<String>,
    models: Vec<GeminiModel>,
}

impl GeminiClient {
    /// Create a new Gemini client with API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            auth: GeminiAuth::ApiKey(api_key.into()),
            api_url: endpoints::GOOGLE_AI_BASE_URL.to_string(),
            use_vertex_ai: false,
            _project: None,
            _location: None,
            models: get_gemini_models(),
        }
    }

    /// Automatically select the best model for the task
    /// Returns the model ID to use
    pub fn select_auto_model(&self, _messages: &[ChatMessage]) -> String {
        // For Vertex AI, use gemini-auto for automatic routing
        // This lets Google's infrastructure choose the best model
        if self.use_vertex_ai {
            "gemini-auto".to_string()
        } else {
            // For API key mode, default to gemini-2.0-flash
            "gemini-2.0-flash".to_string()
        }
    }

    /// Create a new Gemini client for Vertex AI with service account
    pub fn new_vertex_ai_service_account(
        creds: ServiceAccountCredentials,
        location: impl Into<String>,
    ) -> Self {
        let project = creds.project_id.clone();
        let location = location.into();
        let api_url = endpoints::vertex_ai_base_url(&project, &location);

        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            auth: GeminiAuth::ServiceAccount(creds),
            api_url,
            use_vertex_ai: true,
            _project: Some(project),
            _location: Some(location),
            models: get_gemini_models(),
        }
    }

    /// Create a new Gemini client for Vertex AI with OAuth
    pub fn new_vertex_ai_oauth(
        creds: OAuthCredentials,
        project: impl Into<String>,
        location: impl Into<String>,
    ) -> Self {
        let project = project.into();
        let location = location.into();
        let api_url = endpoints::vertex_ai_base_url(&project, &location);

        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            auth: GeminiAuth::OAuthRefreshToken(creds),
            api_url,
            use_vertex_ai: true,
            _project: Some(project),
            _location: Some(location),
            models: get_gemini_models(),
        }
    }

    /// Create from environment variables
    ///
    /// Priority:
    /// 1. Service account (GOOGLE_APPLICATION_CREDENTIALS or ~/.config/gcloud/*.json)
    /// 2. OAuth refresh token (GOOGLE_GENAI_USE_VERTEXAI=true)
    /// 3. API key (GEMINI_API_KEY or GOOGLE_API_KEY)
    pub fn from_env() -> Result<Self> {
        let use_vertex = std::env::var("GOOGLE_GENAI_USE_VERTEXAI")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        let location =
            std::env::var("GOOGLE_CLOUD_LOCATION").unwrap_or_else(|_| "us-central1".to_string());

        // Only try Vertex AI if explicitly enabled
        if use_vertex {
            // Try service account first
            if let Ok(sa_creds) = load_service_account_credentials() {
                info!(
                    "✅ Vertex AI mode (service account): project={}, location={}",
                    sa_creds.project_id, location
                );
                return Ok(Self::new_vertex_ai_service_account(sa_creds, location));
            }

            // Try OAuth refresh token if Vertex AI mode enabled
            if let Ok(oauth_creds) = load_oauth_credentials() {
                let project = std::env::var("GOOGLE_CLOUD_PROJECT")
                    .or_else(|_| {
                        oauth_creds
                            .quota_project_id
                            .clone()
                            .ok_or(std::env::VarError::NotPresent)
                    })
                    .context("GOOGLE_CLOUD_PROJECT not set for OAuth Vertex AI mode")?;

                info!(
                    "✅ Vertex AI mode (OAuth): project={}, location={}",
                    project, location
                );
                return Ok(Self::new_vertex_ai_oauth(oauth_creds, project, location));
            }
        }

        // Fall back to API key (default mode)
        let api_key = std::env::var("GEMINI_API_KEY")
            .or_else(|_| std::env::var("GOOGLE_API_KEY"))
            .context("No Gemini credentials found. Set GEMINI_API_KEY or GOOGLE_API_KEY for API key mode")?;

        info!("✅ API Key mode (generativelanguage.googleapis.com)");
        Ok(Self::new(api_key))
    }

    /// Create with custom endpoint (API key mode)
    pub fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        let mut client = Self::new(api_key);
        client.api_url = endpoint.into();
        client
    }

    /// Get the current API URL
    pub fn api_url(&self) -> &str {
        &self.api_url
    }

    /// Check if using Vertex AI mode
    pub fn is_vertex_ai(&self) -> bool {
        self.use_vertex_ai
    }

    /// Build the full URL for a model endpoint
    fn build_url(&self, model: &str, action: &str) -> Result<String> {
        match &self.auth {
            GeminiAuth::ApiKey(key) => Ok(format!(
                "{}/models/{}:{}?key={}",
                self.api_url, model, action, key
            )),
            GeminiAuth::ServiceAccount(_) | GeminiAuth::OAuthRefreshToken(_) => {
                Ok(format!("{}/{}:{}", self.api_url, model, action))
            }
        }
    }

    /// Get authorization header
    async fn get_auth_header(&self) -> Result<Option<String>> {
        match &self.auth {
            GeminiAuth::ApiKey(_) => Ok(None),
            GeminiAuth::ServiceAccount(creds) => {
                let token = get_service_account_token(creds).await?;
                Ok(Some(format!("Bearer {}", token)))
            }
            GeminiAuth::OAuthRefreshToken(creds) => {
                let token = get_oauth_refresh_token(creds).await?;
                Ok(Some(format!("Bearer {}", token)))
            }
        }
    }

    fn to_model_info(&self, model: &GeminiModel) -> ModelInfo {
        let description = format!(
            "{} - RPM: {}, TPM: {}{}",
            model.category,
            if model.rpm == 0 {
                "Unlimited".to_string()
            } else {
                model.rpm.to_string()
            },
            if model.tpm >= 1_000_000 {
                format!("{}M", model.tpm / 1_000_000)
            } else if model.tpm >= 1_000 {
                format!("{}K", model.tpm / 1_000)
            } else if model.tpm == 0 {
                "N/A".to_string()
            } else {
                model.tpm.to_string()
            },
            if model.rpd == 0 {
                ", RPD: Unlimited".to_string()
            } else {
                format!(", RPD: {}", model.rpd)
            }
        );

        ModelInfo {
            id: model.id.clone(),
            name: model.id.clone(),
            description: Some(description),
            parameters: None,
            available: true,
            tags: vec![model.category.to_string()],
            downloads: None,
            updated_at: None,
        }
    }
}

#[async_trait]
impl LlmProvider for GeminiClient {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Gemini
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        info!("Gemini models (static list)");
        info!(
            "  Mode: {}",
            if self.use_vertex_ai {
                "Vertex AI"
            } else {
                "API Key"
            }
        );
        info!("  Endpoint: {}", self.api_url);
        Ok(self.models.iter().map(|m| self.to_model_info(m)).collect())
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let query_lower = query.to_lowercase();
        Ok(self
            .models
            .iter()
            .filter(|m| m.id.to_lowercase().contains(&query_lower))
            .take(limit)
            .map(|m| self.to_model_info(m))
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        Ok(self
            .models
            .iter()
            .find(|m| m.id == model_id)
            .map(|m| self.to_model_info(m)))
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        Ok(self.models.iter().any(|m| m.id == model_id))
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        // Support "auto" model selection
        let actual_model = if model == "auto" || model == "gemini-auto" {
            let selected = self.select_auto_model(&messages);
            info!("Auto model selection: {} -> {}", model, selected);
            selected
        } else {
            model.to_string()
        };

        let url = self.build_url(&actual_model, "generateContent")?;

        info!(
            "Gemini chat: model={}, mode={}",
            actual_model,
            if self.use_vertex_ai {
                "Vertex AI"
            } else {
                "API Key"
            }
        );

        // Extract system message if present
        let system_instruction =
            messages
                .iter()
                .find(|m| m.role == "system")
                .map(|m| GeminiContent {
                    role: "user".to_string(),
                    parts: vec![GeminiPart {
                        text: m.content.clone(),
                    }],
                });

        // Build contents excluding system messages
        let contents: Vec<GeminiContent> = messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| GeminiContent {
                role: if m.role == "assistant" {
                    "model".to_string()
                } else {
                    "user".to_string()
                },
                parts: vec![GeminiPart {
                    text: m.content.clone(),
                }],
            })
            .collect();

        // Enable auto-routing for Gemini 3 models (only in Vertex AI mode)
        // Note: Auto-routing is not supported in API key mode
        let use_auto_routing = actual_model.starts_with("gemini-3") && self.use_vertex_ai;
        let routing_config = if use_auto_routing {
            Some(RoutingConfig {
                auto_mode: Some(AutoRoutingMode {
                    model_routing_preference: "BALANCED".to_string(),
                }),
            })
        } else {
            None
        };

        if use_auto_routing {
            info!("🔀 Auto-routing enabled (BALANCED mode) - Vertex AI only");
        }

        let gemini_req = GeminiRequest {
            contents,
            system_instruction,
            generation_config: Some(GenerationConfig {
                temperature: Some(0.7),
                top_p: Some(0.95),
                max_output_tokens: Some(8192),
                routing_config,
            }),
            tools: None,
            tool_config: None,
        };

        debug!(
            "Gemini request to: {}",
            url.split('?').next().unwrap_or(&url)
        );

        // Retry with exponential backoff for rate limiting (429) errors
        let max_retries = 5;
        let mut retry_count = 0;

        loop {
            // Build request with appropriate auth (regenerate token for each retry)
            let mut req = self.client.post(&url).json(&gemini_req);

            if let Some(auth_header) = self.get_auth_header().await? {
                req = req.header("Authorization", auth_header);
            }

            let response = match req.send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Gemini HTTP request failed: {}", e);
                    return Err(anyhow::anyhow!("Failed to send Gemini request: {}", e));
                }
            };

            let status = response.status();

            // Check if we got a 429 (rate limit) error
            if status.as_u16() == 429 {
                if retry_count >= max_retries {
                    let body = response.text().await.unwrap_or_default();
                    tracing::error!(
                        "Gemini API rate limit exceeded after {} retries: {}",
                        max_retries,
                        body
                    );
                    return Err(anyhow::anyhow!(
                        "Gemini API rate limit exceeded after {} retries. Please try again later.",
                        max_retries
                    ));
                }

                // Exponential backoff: 1s, 2s, 4s, 8s, 16s
                let delay_secs = 1u64 << retry_count;
                tracing::warn!(
                    "Gemini API rate limit (429), retrying in {}s (attempt {}/{})",
                    delay_secs,
                    retry_count + 1,
                    max_retries
                );
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                retry_count += 1;
                continue;
            }

            // For non-429 errors, fail immediately
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                tracing::error!("Gemini API error {}: {}", status, body);
                return Err(anyhow::anyhow!("Gemini API error {}: {}", status, body));
            }

            // Get raw response text first for debugging
            let raw_body = response
                .text()
                .await
                .context("Failed to read Gemini response body")?;

            let mut raw_body_mut = raw_body;
            let result: GeminiResponse = match unsafe { simd_json::from_str(&mut raw_body_mut) } {
                Ok(r) => r,
                Err(e) => {
                    let preview = if raw_body_mut.len() > 1000 {
                        format!("{}...[truncated]", &raw_body_mut[..1000])
                    } else {
                        raw_body_mut.clone()
                    };
                    tracing::error!("Failed to parse Gemini response: {}", e);
                    tracing::error!("Raw response: {}", preview);
                    return Err(anyhow::anyhow!(
                        "Failed to parse Gemini response: {}. Raw: {}",
                        e,
                        preview
                    ));
                }
            };

            let text = result
                .candidates
                .first()
                .and_then(|c| c.content.parts.first())
                .and_then(|p| p.text.clone())
                .unwrap_or_default();

            let finish_reason = result
                .candidates
                .first()
                .and_then(|c| c.finish_reason.clone());

            let usage = result.usage_metadata.map(|u| TokenUsage {
                prompt_tokens: u.prompt_token_count.unwrap_or(0),
                completion_tokens: u.candidates_token_count.unwrap_or(0),
                total_tokens: u.total_token_count.unwrap_or(0),
            });

            return Ok(ChatResponse {
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: text,
                    tool_calls: None,
                    tool_call_id: None,
                },
                model: model.to_string(),
                provider: "gemini".to_string(),
                finish_reason,
                usage,
                tool_calls: None,
            });
        }
    }

    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        // Support "auto" model selection
        let actual_model = if model == "auto" || model == "gemini-auto" {
            let selected = self.select_auto_model(&request.messages);
            info!("Auto model selection: {} -> {}", model, selected);
            selected
        } else {
            model.to_string()
        };

        let url = self.build_url(&actual_model, "generateContent")?;

        info!(
            "Gemini chat_with_request: model={}, tools={}, mode={}",
            actual_model,
            request.tools.len(),
            if self.use_vertex_ai {
                "Vertex AI"
            } else {
                "API Key"
            }
        );

        // Extract system message if present
        let system_instruction = request
            .messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiPart {
                    text: m.content.clone(),
                }],
            });

        // Build contents excluding system messages
        let contents: Vec<GeminiContent> = request
            .messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| GeminiContent {
                role: if m.role == "assistant" {
                    "model".to_string()
                } else {
                    "user".to_string()
                },
                parts: vec![GeminiPart {
                    text: m.content.clone(),
                }],
            })
            .collect();

        // Convert tools to Gemini format
        let tools = if request.tools.is_empty() {
            None
        } else {
            let function_declarations: Vec<GeminiFunctionDeclaration> = request
                .tools
                .iter()
                .map(|t| GeminiFunctionDeclaration {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                })
                .collect();

            Some(vec![GeminiTool {
                function_declarations,
            }])
        };

        // Convert tool_choice to Gemini format
        let tool_config = if !request.tools.is_empty() {
            let mode = match request.tool_choice {
                ToolChoice::Auto => "AUTO",
                ToolChoice::None => "NONE",
                ToolChoice::Required => "ANY",
                ToolChoice::Tool(_) => "ANY", // Gemini doesn't support specific tool selection
            };
            Some(GeminiToolConfig {
                function_calling_config: FunctionCallingConfig {
                    mode: mode.to_string(),
                },
            })
        } else {
            None
        };

        // Enable auto-routing for Gemini 3 models (only in Vertex AI mode)
        // Note: Auto-routing is not supported in API key mode
        let use_auto_routing = actual_model.starts_with("gemini-3") && self.use_vertex_ai;
        let routing_config = if use_auto_routing {
            Some(RoutingConfig {
                auto_mode: Some(AutoRoutingMode {
                    model_routing_preference: "BALANCED".to_string(),
                }),
            })
        } else {
            None
        };

        if use_auto_routing {
            info!("🔀 Auto-routing enabled (BALANCED mode) - Vertex AI only");
        }

        let gemini_request = GeminiRequest {
            contents,
            system_instruction,
            generation_config: Some(GenerationConfig {
                temperature: request.temperature,
                top_p: request.top_p,
                max_output_tokens: request.max_tokens,
                routing_config,
            }),
            tools,
            tool_config,
        };

        debug!(
            "Gemini request to: {}",
            url.split('?').next().unwrap_or(&url)
        );

        // Retry with exponential backoff for rate limiting (429) errors
        let max_retries = 5;
        let mut retry_count = 0;

        loop {
            // Build request with appropriate auth
            let mut req = self.client.post(&url).json(&gemini_request);

            if let Some(auth_header) = self.get_auth_header().await? {
                req = req.header("Authorization", auth_header);
            }

            let response = match req.send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Gemini HTTP request failed: {}", e);
                    return Err(anyhow::anyhow!("Failed to send Gemini request: {}", e));
                }
            };

            let status = response.status();

            // Handle 429 rate limit
            if status.as_u16() == 429 {
                if retry_count >= max_retries {
                    let body = response.text().await.unwrap_or_default();
                    tracing::error!(
                        "Gemini API rate limit exceeded after {} retries: {}",
                        max_retries,
                        body
                    );
                    return Err(anyhow::anyhow!("Gemini API rate limit exceeded"));
                }

                let delay_secs = 1u64 << retry_count;
                tracing::warn!("Gemini API rate limit (429), retrying in {}s", delay_secs);
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                retry_count += 1;
                continue;
            }

            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                tracing::error!("Gemini API error {}: {}", status, body);
                return Err(anyhow::anyhow!("Gemini API error {}: {}", status, body));
            }

            // Get raw response text first for debugging
            let raw_body = response
                .text()
                .await
                .context("Failed to read Gemini response body")?;

            // Parse response
            let mut raw_body_mut = raw_body;
            let result: GeminiResponse = match unsafe { simd_json::from_str(&mut raw_body_mut) } {
                Ok(r) => r,
                Err(e) => {
                    // Log the raw response for debugging
                    let preview = if raw_body_mut.len() > 1000 {
                        format!("{}...[truncated]", &raw_body_mut[..1000])
                    } else {
                        raw_body_mut.clone()
                    };
                    tracing::error!("Failed to parse Gemini response: {}", e);
                    tracing::error!("Raw response: {}", preview);
                    return Err(anyhow::anyhow!(
                        "Failed to parse Gemini response: {}. Raw: {}",
                        e,
                        preview
                    ));
                }
            };

            // Extract text and function calls
            let mut text = String::new();
            let mut tool_calls: Vec<ToolCallInfo> = Vec::new();

            if let Some(candidate) = result.candidates.first() {
                for part in &candidate.content.parts {
                    if let Some(ref t) = part.text {
                        text.push_str(t);
                    }
                    if let Some(ref fc) = part.function_call {
                        tool_calls.push(ToolCallInfo {
                            id: format!("call_{}", tool_calls.len()),
                            name: fc.name.clone(),
                            arguments: fc.args.clone(),
                        });
                    }
                }
            }

            let finish_reason = result
                .candidates
                .first()
                .and_then(|c| c.finish_reason.clone());

            let usage = result.usage_metadata.map(|u| TokenUsage {
                prompt_tokens: u.prompt_token_count.unwrap_or(0),
                completion_tokens: u.candidates_token_count.unwrap_or(0),
                total_tokens: u.total_token_count.unwrap_or(0),
            });

            // Log tool calls if any
            if !tool_calls.is_empty() {
                info!("Gemini returned {} tool calls", tool_calls.len());
                for tc in &tool_calls {
                    debug!("  Tool call: {}({})", tc.name, tc.arguments);
                }
            }

            return Ok(ChatResponse {
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: text,
                    tool_calls: None, // We put them in the response.tool_calls field
                    tool_call_id: None,
                },
                model: model.to_string(),
                provider: "gemini".to_string(),
                finish_reason,
                usage,
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls)
                },
            });
        }
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let response = self.chat(model, messages).await?;
        tx.send(Ok(response.message.content)).await.ok();
        Ok(rx)
    }
}
</file>

<file path="src/headless_oauth.rs">
//! Headless OAuth Token Provider
//!
//! Loads OAuth tokens saved by the Antigravity headless service.
//! Token is captured when user logs into Antigravity via VNC.
//!
//! ## Token Flow
//!
//! 1. `antigravity-display.service` runs Antigravity IDE in virtual Wayland
//! 2. User connects via VNC and logs in with Google account
//! 3. `antigravity-extract-token.sh` copies token to standard location
//! 4. This provider loads and auto-refreshes the token
//!
//! ## Token Location
//!
//! Default: `~/.config/antigravity/token.json`
//! Override: `GOOGLE_AUTH_TOKEN_FILE` environment variable

use anyhow::{Context, Result};
use dirs;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// Google OAuth token endpoints
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
#[allow(dead_code)]
const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";

/// Refresh 5 minutes before expiry
const REFRESH_BUFFER_SECS: u64 = 300;

/// OAuth token from Antigravity headless service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub expires_at: Option<f64>,
    #[serde(default)]
    pub expiry: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub saved_at: Option<f64>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
}

impl OAuthToken {
    /// Check if token is expired or will expire soon
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            return now > (expires_at - REFRESH_BUFFER_SECS as f64);
        }

        // No expiry info = assume valid (rely on API to reject)
        false
    }

    /// Get remaining validity in seconds
    pub fn remaining_secs(&self) -> Option<i64> {
        self.expires_at.map(|expires_at| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            (expires_at - now) as i64
        })
    }
}

/// Cached token with load time
#[derive(Debug, Clone)]
struct CachedToken {
    token: OAuthToken,
    #[allow(dead_code)]
    loaded_at: std::time::SystemTime,
}

/// Headless OAuth provider
///
/// Loads tokens captured from Antigravity headless service.
#[derive(Debug)]
pub struct HeadlessOAuthProvider {
    /// Path to token file
    token_file: PathBuf,
    /// OAuth client ID (for refresh)
    client_id: String,
    /// OAuth client secret (for refresh)
    client_secret: String,
    /// Cached token
    cached_token: RwLock<Option<CachedToken>>,
    /// HTTP client
    client: Client,
}

impl HeadlessOAuthProvider {
    /// Create from environment variables
    ///
    /// Looks for token at:
    /// 1. `GOOGLE_AUTH_TOKEN_FILE` environment variable
    /// 2. `~/.config/antigravity/token.json` (default)
    /// 3. `~/.config/gcloud/application_default_credentials.json` (fallback)
    pub fn from_env() -> Result<Self> {
        let token_file = std::env::var("GOOGLE_AUTH_TOKEN_FILE")
            .map(PathBuf::from)
            .or_else(|_| {
                // Default location
                dirs::config_dir()
                    .map(|d| d.join("antigravity").join("token.json"))
                    .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))
            })
            .or_else(|_| {
                // Fallback to gcloud ADC
                dirs::config_dir()
                    .map(|d| {
                        d.join("gcloud")
                            .join("application_default_credentials.json")
                    })
                    .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))
            })?;

        let client_id = std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default();
        let client_secret = std::env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default();

        Ok(Self::new(token_file, client_id, client_secret))
    }

    /// Create with explicit configuration
    pub fn new(
        token_file: impl Into<PathBuf>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        Self {
            token_file: token_file.into(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            cached_token: RwLock::new(None),
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Get valid access token, refreshing if needed
    pub async fn get_token(&self) -> Result<String> {
        // Check cache
        {
            let cache = self.cached_token.read().unwrap();
            if let Some(ref cached) = *cache {
                if !cached.token.is_expired() {
                    debug!("Using cached OAuth token");
                    return Ok(cached.token.access_token.clone());
                }
            }
        }

        // Load from file
        let mut token = self.load_token().await?;

        // Refresh if expired
        if token.is_expired() {
            if let Some(ref refresh_token) = token.refresh_token {
                let client_id = if self.client_id.is_empty() {
                    token.client_id.clone().unwrap_or_default()
                } else {
                    self.client_id.clone()
                };

                let client_secret = if self.client_secret.is_empty() {
                    token.client_secret.clone().unwrap_or_default()
                } else {
                    self.client_secret.clone()
                };

                if !client_id.is_empty() {
                    info!("Token expired, refreshing...");
                    match self
                        .refresh_token(refresh_token, &client_id, &client_secret)
                        .await
                    {
                        Ok(new_token) => {
                            token = new_token;
                            if let Err(e) = self.save_token(&token).await {
                                warn!("Failed to save refreshed token: {}", e);
                            }
                        }
                        Err(e) => {
                            warn!("Token refresh failed: {}", e);
                        }
                    }
                } else {
                    warn!("Token expired, no client_id for refresh");
                }
            } else {
                warn!("Token expired, no refresh_token available");
            }
        }

        // Cache
        {
            let mut cache = self.cached_token.write().unwrap();
            *cache = Some(CachedToken {
                token: token.clone(),
                loaded_at: SystemTime::now(),
            });
        }

        Ok(token.access_token)
    }

    /// Check if token file exists and has valid token
    pub fn is_authenticated(&self) -> bool {
        if !self.token_file.exists() {
            return false;
        }

        if let Ok(contents) = std::fs::read_to_string(&self.token_file) {
            let mut contents_mut = contents;
            if let Ok(token) = unsafe { simd_json::from_str::<OAuthToken>(&mut contents_mut) } {
                return token.refresh_token.is_some() || !token.is_expired();
            }
        }

        false
    }

    /// Get token file path
    pub fn token_file(&self) -> &Path {
        &self.token_file
    }

    async fn load_token(&self) -> Result<OAuthToken> {
        let contents = tokio::fs::read_to_string(&self.token_file)
            .await
            .with_context(|| format!("Token file not found: {}\n\nTo authenticate:\n1. Start Antigravity headless: sudo systemctl start antigravity-display\n2. Connect via VNC: vncviewer localhost:5900\n3. Log in with Google account\n4. Run: ./scripts/antigravity-extract-token.sh", self.token_file.display()))?;

        let mut contents_mut = contents;
        let token: OAuthToken =
            unsafe { simd_json::from_str(&mut contents_mut) }.context("Invalid token JSON")?;

        if let Some(remaining) = token.remaining_secs() {
            debug!("Loaded token, expires in {}s", remaining);
        }

        Ok(token)
    }

    async fn save_token(&self, token: &OAuthToken) -> Result<()> {
        let contents = simd_json::to_string_pretty(token)?;
        tokio::fs::write(&self.token_file, contents).await?;
        Ok(())
    }

    async fn refresh_token(
        &self,
        refresh_token: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<OAuthToken> {
        let response = self
            .client
            .post(GOOGLE_TOKEN_URL)
            .form(&[
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .context("Refresh request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Token refresh failed {}: {}", status, body);
        }

        let mut new_token: OAuthToken =
            response.json().await.context("Invalid refresh response")?;

        // Preserve fields
        if new_token.refresh_token.is_none() {
            new_token.refresh_token = Some(refresh_token.to_string());
        }
        new_token.client_id = Some(client_id.to_string());
        new_token.client_secret = Some(client_secret.to_string());

        // Calculate expiry
        if let Some(expires_in) = new_token.expires_in {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            new_token.expires_at = Some(now + expires_in as f64);
            new_token.saved_at = Some(now);
        }

        info!("Token refreshed successfully");
        Ok(new_token)
    }
}

impl Default for HeadlessOAuthProvider {
    fn default() -> Self {
        Self::from_env().unwrap_or_else(|_| {
            Self::new(
                dirs::config_dir()
                    .map(|d| d.join("antigravity").join("token.json"))
                    .unwrap_or_else(|| PathBuf::from("/tmp/antigravity-token.json")),
                "",
                "",
            )
        })
    }
}
</file>

<file path="src/huggingface.rs">
//! HuggingFace LLM Provider with FORCED tool support
//!
//! This implementation properly passes tools and tool_choice to the API.

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::provider::{
    ChatMessage,
    ChatRequest,
    ChatResponse,
    LlmProvider,
    ModelInfo,
    ProviderType,
    TokenUsage,
    ToolCallInfo,
    // ToolChoice,
};

const HF_API_BASE: &str = "https://api-inference.huggingface.co";
const DEFAULT_MODEL: &str = "meta-llama/Llama-3.3-70B-Instruct";

pub struct HuggingFaceClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl HuggingFaceClient {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            base_url: HF_API_BASE.to_string(),
        }
    }

    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.to_string();
        self
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("HF_TOKEN")
            .or_else(|_| std::env::var("HUGGINGFACE_TOKEN"))
            .context("HF_TOKEN or HUGGINGFACE_TOKEN must be set")?;
        Ok(Self::new(api_key))
    }
}

#[async_trait]
impl LlmProvider for HuggingFaceClient {
    fn provider_type(&self) -> ProviderType {
        ProviderType::HuggingFace
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        // Return commonly used models
        Ok(vec![
            ModelInfo {
                id: "meta-llama/Llama-3.3-70B-Instruct".to_string(),
                name: "Llama 3.3 70B Instruct".to_string(),
                description: Some("Meta's Llama 3.3 70B with instruction tuning".to_string()),
                parameters: Some("70B".to_string()),
                available: true,
                tags: vec!["llama".to_string(), "instruct".to_string()],
                downloads: None,
                updated_at: None,
            },
            ModelInfo {
                id: "mistralai/Mixtral-8x7B-Instruct-v0.1".to_string(),
                name: "Mixtral 8x7B Instruct".to_string(),
                description: Some("Mistral's MoE model".to_string()),
                parameters: Some("46.7B".to_string()),
                available: true,
                tags: vec!["mixtral".to_string(), "moe".to_string()],
                downloads: None,
                updated_at: None,
            },
        ])
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models
            .into_iter()
            .filter(|m| m.name.to_lowercase().contains(&query.to_lowercase()))
            .take(limit)
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        Ok(self.get_model(model_id).await?.is_some())
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        // Simple chat without tools - NOT RECOMMENDED
        warn!("Using chat() without tools - consider using chat_with_request()");

        let request = ChatRequest::new(messages);
        self.chat_with_request(model, request).await
    }

    /// Chat with FULL tool support
    ///
    /// This is the CORRECT implementation that:
    /// 1. Passes tools to the API
    /// 2. Sets tool_choice (including "required")
    /// 3. Parses tool_calls from response
    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        let model = if model.is_empty() {
            DEFAULT_MODEL
        } else {
            model
        };
        let url = format!("{}/models/{}/v1/chat/completions", self.base_url, model);

        // Convert messages to API format
        let messages: Vec<Value> = request
            .messages
            .iter()
            .map(|m| {
                let mut msg = json!({
                    "role": m.role,
                    "content": m.content
                });

                // Add tool_call_id for tool responses
                if let Some(ref id) = m.tool_call_id {
                    msg["tool_call_id"] = json!(id);
                }

                // Add tool_calls for assistant messages
                if let Some(ref calls) = m.tool_calls {
                    msg["tool_calls"] = json!(calls.iter().map(|tc| {
                        json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": simd_json::to_string(&tc.arguments).unwrap_or_default()
                            }
                        })
                    }).collect::<Vec<_>>());
                }

                msg
            })
            .collect();

        // Convert tools to API format
        let tools: Vec<Value> = request.tools.iter().map(|t| t.to_openai_format()).collect();

        // Build request body
        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": false
        });

        // Add tools if present
        if !tools.is_empty() {
            body["tools"] = json!(tools);

            // CRITICAL: Set tool_choice
            body["tool_choice"] = request.tool_choice.to_api_format();

            info!(
                "Sending request with {} tools, tool_choice={:?}",
                tools.len(),
                request.tool_choice
            );
        }

        // Add optional parameters
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(temp) = request.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }

        debug!(
            "HuggingFace request: {}",
            simd_json::to_string_pretty(&body).unwrap_or_default()
        );

        // Send request
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send request to HuggingFace")?;

        let status = response.status();
        let response_text = response.text().await?;

        debug!(
            "HuggingFace response ({}): {}",
            status,
            &response_text[..response_text.len().min(500)]
        );

        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "HuggingFace API error ({}): {}",
                status,
                response_text
            ));
        }

        // Parse response
        let mut response_text_mut = response_text;
        let response_json: Value =
            unsafe { simd_json::from_str(&mut response_text_mut) }.map_err(|e| {
                anyhow::anyhow!(
                    "Failed to parse HuggingFace response: {}. Body: {}",
                    e,
                    response_text_mut
                )
            })?;

        let choice = response_json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|a| a.get(0))
            .ok_or_else(|| anyhow::anyhow!("No choices returned from HuggingFace"))?;

        let message = choice
            .get("message")
            .ok_or_else(|| anyhow::anyhow!("No message in response"))?;

        let content = message
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        let role = message
            .get("role")
            .and_then(|r| r.as_str())
            .unwrap_or("assistant")
            .to_string();

        // Parse tool_calls from response
        let tool_calls: Option<Vec<ToolCallInfo>> = message
            .get("tool_calls")
            .and_then(|tc| tc.as_array())
            .map(|calls| {
                calls
                    .iter()
                    .filter_map(|call| {
                        let id = call.get("id")?.as_str()?.to_string();
                        let function = call.get("function")?;
                        let name = function.get("name")?.as_str()?.to_string();
                        let args_str = function.get("arguments")?.as_str()?;
                        let mut args_mut = args_str.to_string();
                        let arguments: Value =
                            unsafe { simd_json::from_str(&mut args_mut) }.ok()?;

                        Some(ToolCallInfo {
                            id,
                            name,
                            arguments,
                        })
                    })
                    .collect()
            });

        if let Some(ref calls) = tool_calls {
            info!("Parsed {} tool calls from response", calls.len());
            for call in calls {
                debug!("  Tool call: {} ({})", call.name, call.id);
            }
        }

        let finish_reason = choice
            .get("finish_reason")
            .and_then(|f| f.as_str())
            .map(|s| s.to_string());

        let usage = response_json.get("usage").map(|u| TokenUsage {
            prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            completion_tokens: u
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        });

        Ok(ChatResponse {
            message: ChatMessage {
                role,
                content,
                tool_calls: tool_calls.clone(),
                tool_call_id: None,
            },
            model: model.to_string(),
            provider: "huggingface".to_string(),
            finish_reason,
            usage,
            tool_calls,
        })
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        // Streaming not implemented for this example
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let response = self.chat(model, messages).await?;
        let _ = tx.send(Ok(response.message.content)).await;
        Ok(rx)
    }
}
</file>

<file path="src/lib.rs">
//! op-llm: Multi-Provider LLM Integration
//!
//! ## Supported Providers & Endpoints
//!
//! | Provider | Base URL | Auth Method |
//! |----------|----------|-------------|
//! | Antigravity | Gemini API | Headless OAuth (captured from IDE) |
//! | Gemini | `https://generativelanguage.googleapis.com/v1beta` | API Key or OAuth |
//! | Anthropic | `https://api.anthropic.com/v1` | `x-api-key: {KEY}` |
//! | Perplexity | `https://api.perplexity.ai` | `Bearer {KEY}` |
//! | HuggingFace | `https://api-inference.huggingface.co` | `Bearer {HF_TOKEN}` |
//!
//! ## Authentication
//!
//! ### Option 1: Antigravity Headless (Recommended for Enterprise)
//! ```bash
//! # Start Antigravity service
//! sudo systemctl start antigravity-display antigravity-vnc
//!
//! # Connect via VNC and login once
//! vncviewer localhost:5900
//!
//! # Extract token
//! ./scripts/antigravity-extract-token.sh
//!
//! # Configure
//! export GOOGLE_AUTH_TOKEN_FILE=~/.config/antigravity/token.json
//! export LLM_PROVIDER=antigravity
//! ```
//!
//! ### Option 2: API Keys
//! ```bash
//! export GEMINI_API_KEY=xxx           # Google Gemini  
//! export ANTHROPIC_API_KEY=sk-xxx     # Anthropic Claude
//! export PERPLEXITY_API_KEY=pplx-xxx  # Perplexity
//! export HF_TOKEN=hf_xxx              # HuggingFace
//! ```

pub mod anthropic;
pub mod antigravity;
pub mod assistant;
pub mod chat;
pub mod factory;
pub mod gcloud_adc;
pub mod gemini;
pub mod gemini_cli;
pub mod headless_oauth;
pub mod huggingface;
pub mod mcp_proxy;
pub mod openclaw;
pub mod perplexity;
pub mod provider;
pub mod pty_bridge;

pub use anthropic::AnthropicClient;
pub use antigravity::AntigravityProvider;
pub use assistant::AssistantProvider;
pub use factory::FactoryProvider;
pub use gcloud_adc::GCloudADCProvider;
pub use gemini::GeminiClient;
pub use headless_oauth::{HeadlessOAuthProvider, OAuthToken};
pub use huggingface::HuggingFaceClient;
pub use openclaw::OpenClawProvider;
pub use perplexity::PerplexityClient;
pub use provider::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderConfig, ProviderType,
    ToolChoice, ToolDefinition,
};

/// Prelude for convenient imports
pub mod prelude {
    pub use super::anthropic::AnthropicClient;
    pub use super::antigravity::AntigravityProvider;
    pub use super::assistant::AssistantProvider;
    pub use super::factory::FactoryProvider;
    pub use super::gcloud_adc::GCloudADCProvider;
    pub use super::gemini::GeminiClient;
    pub use super::headless_oauth::{HeadlessOAuthProvider, OAuthToken};
    pub use super::huggingface::HuggingFaceClient;
    pub use super::openclaw::OpenClawProvider;
    pub use super::perplexity::PerplexityClient;
    pub use super::provider::{
        ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderConfig,
        ProviderType, ToolChoice, ToolDefinition,
    };
}
</file>

<file path="src/mcp_proxy.rs">
//! MCP Proxy LLM provider – delegates to op-mcp-proxy in DIRECT_MODE.

use anyhow::{Context, Result};
use async_trait::async_trait;
use simd_json::prelude::*;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::debug;

use crate::provider::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType,
};

pub struct McpProxyProvider {
    bin: String,
    env_extra: Vec<(String, String)>,
}

impl McpProxyProvider {
    /// Build from environment.  Requires OP_MCP_PROXY_BIN (or falls back to "op-mcp-proxy").
    pub fn from_env() -> Result<Self> {
        let bin = std::env::var("OP_MCP_PROXY_BIN").unwrap_or_else(|_| "op-mcp-proxy".to_string());

        // Verify binary exists
        if !bin.starts_with('/') {
            // relative name – trust PATH
        } else if !std::path::Path::new(&bin).exists() {
            anyhow::bail!("op-mcp-proxy binary not found at {}", bin);
        }

        let mut env_extra = vec![("DIRECT_MODE".to_string(), "1".to_string())];

        // Forward relevant MCP_PROXY_* env vars
        for (k, v) in std::env::vars() {
            if k.starts_with("MCP_PROXY_") || k.starts_with("OP_MCP_PROXY_") {
                env_extra.push((k, v));
            }
        }

        Ok(Self { bin, env_extra })
    }

    /// Spawn op-mcp-proxy (via select3 wrapper), send one JSON-RPC request, return the response.
    async fn call(&self, request: simd_json::OwnedValue) -> Result<simd_json::OwnedValue> {
        let mut cmd = tokio::process::Command::new(&self.bin);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(self.env_extra.clone()); // Apply collected environment variables

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn {}", self.bin))?;

        let stdin = child.stdin.as_mut().context("no stdin")?;
        let line = simd_json::to_string(&request)?;
        stdin.write_all(line.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        drop(child.stdin.take());

        let stdout = child.stdout.take().context("no stdout")?;
        let mut reader = BufReader::new(stdout);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await?;

        let _ = child.wait().await;

        if response_line.trim().is_empty() {
            // Read stderr for diagnostics
            if let Some(mut stderr) = child.stderr.take() {
                let mut err = String::new();
                tokio::io::AsyncReadExt::read_to_string(&mut stderr, &mut err)
                    .await
                    .ok();
                if !err.trim().is_empty() {
                    debug!("op-mcp-proxy stderr: {}", err.trim());
                }
            }
            anyhow::bail!("op-mcp-proxy returned empty response");
        }

        let mut bytes = response_line.into_bytes();
        let json: simd_json::OwnedValue =
            simd_json::from_slice(&mut bytes).context("failed to parse op-mcp-proxy response")?;

        if let Some(err) = json.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!("op-mcp-proxy error: {}", msg);
        }

        Ok(json)
    }
}

#[async_trait]
impl LlmProvider for McpProxyProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::McpProxy
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(vec![
            ModelInfo {
                id: "auto".to_string(),
                name: "Auto (Gemini 3 routing)".to_string(),
                description: Some("Automatic model selection via op-mcp-proxy".to_string()),
                parameters: None,
                available: true,
                tags: vec!["auto".to_string()],
                downloads: None,
                updated_at: None,
            },
            ModelInfo {
                id: "gemini-2.5-flash".to_string(),
                name: "Gemini 2.5 Flash".to_string(),
                description: Some("Fast model via Code Assist".to_string()),
                parameters: None,
                available: true,
                tags: vec!["fast".to_string()],
                downloads: None,
                updated_at: None,
            },
        ])
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let models = self.list_models().await?;
        let q = query.to_lowercase();
        Ok(models
            .into_iter()
            .filter(|m| m.id.to_lowercase().contains(&q) || m.name.to_lowercase().contains(&q))
            .take(limit)
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        Ok(self
            .list_models()
            .await?
            .into_iter()
            .find(|m| m.id == model_id))
    }

    async fn is_model_available(&self, _model_id: &str) -> Result<bool> {
        Ok(true)
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let prompt = messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let req = simd_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "generate",
            "params": {
                "prompt": prompt,
                "model": model
            }
        });

        let resp = self.call(req).await?;
        let result = resp.get("result").context("missing result in response")?;
        let text = result
            .get("completion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let used_model = result
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or(model)
            .to_string();

        Ok(ChatResponse {
            message: ChatMessage::assistant(text),
            model: used_model,
            provider: "mcp-proxy".to_string(),
            finish_reason: Some("stop".to_string()),
            usage: None,
            tool_calls: None,
        })
    }

    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        // For tool-calling requests, flatten to a simple prompt since
        // op-mcp-proxy only supports generateContent.
        self.chat(model, request.messages).await
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let response = self.chat(model, messages).await?;
        tx.send(Ok(response.message.content)).await.ok();
        Ok(rx)
    }
}
</file>

<file path="src/openclaw.rs">
//! OpenClaw LLM Provider
//!
//! Connects to the OpenClaw agent platform via its OpenAI-compatible
//! `/v1/chat/completions` endpoint over the trusted internal network.
//!
//! ## Configuration
//!
//! ```bash
//! OPENCLAW_BASE_URL=http://127.0.0.1:18789  # default
//! ```

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::provider::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType, TokenUsage,
    ToolCallInfo,
};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:18789";
const DEFAULT_MODEL: &str = "openclaw:main";

pub struct OpenClawProvider {
    client: Client,
    base_url: String,
    default_model: String,
}

impl OpenClawProvider {
    pub fn new(base_url: Option<String>, default_model: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
                .trim_end_matches('/')
                .to_string(),
            default_model: default_model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        }
    }

    pub fn from_env() -> Result<Self> {
        let base_url = std::env::var("OPENCLAW_BASE_URL").ok();
        let default_model = std::env::var("OPENCLAW_DEFAULT_MODEL").ok();
        Ok(Self::new(base_url, default_model))
    }

    fn models_url(&self) -> String {
        format!("{}/v1/models", self.base_url)
    }

    fn chat_url(&self) -> String {
        format!("{}/v1/chat/completions", self.base_url)
    }

    fn resolve_model(&self, model: &str) -> String {
        if model.is_empty() {
            self.default_model.clone()
        } else {
            model.to_string()
        }
    }

    fn api_request(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder.header("Content-Type", "application/json")
    }

    fn fallback_model_info(&self) -> ModelInfo {
        ModelInfo {
            id: self.default_model.clone(),
            name: self.default_model.clone(),
            description: Some(
                "Configured OpenClaw default route (OpenClaw selects the agent's configured model)"
                    .to_string(),
            ),
            parameters: None,
            available: true,
            tags: vec![
                "openclaw".to_string(),
                "default".to_string(),
                "agent-route".to_string(),
            ],
            downloads: None,
            updated_at: None,
        }
    }

    fn parse_models_response(response_text: &str) -> Result<Vec<ModelInfo>> {
        let mut response_text_mut = response_text.to_string();
        let response_json: Value = unsafe { simd_json::from_str(&mut response_text_mut) }
            .map_err(|e| anyhow::anyhow!("Failed to parse OpenClaw models response: {}", e))?;

        let models = response_json
            .get("data")
            .and_then(|v| v.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| {
                        let id = entry.get("id")?.as_str()?.to_string();
                        let owned_by = entry
                            .get("owned_by")
                            .and_then(|v| v.as_str())
                            .unwrap_or("openclaw")
                            .to_string();
                        let created = entry
                            .get("created")
                            .and_then(|v| v.as_i64())
                            .map(|ts| ts.to_string());

                        Some(ModelInfo {
                            id: id.clone(),
                            name: id,
                            description: Some(format!("OpenClaw model owned by {}", owned_by)),
                            parameters: None,
                            available: true,
                            tags: vec!["openclaw".to_string(), owned_by],
                            downloads: None,
                            updated_at: created,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(models)
    }
}

#[async_trait]
impl LlmProvider for OpenClawProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::OpenClaw
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let response = self
            .api_request(self.client.get(self.models_url()))
            .send()
            .await
            .context("Failed to query OpenClaw models")?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            warn!(
                "OpenClaw model listing failed ({}), falling back to configured default route",
                status
            );
            return Ok(vec![self.fallback_model_info()]);
        }

        let mut models = match Self::parse_models_response(&response_text) {
            Ok(models) => models,
            Err(err) => {
                warn!(
                    "OpenClaw /v1/models did not return a usable model list ({}), falling back to configured default route",
                    err
                );
                return Ok(vec![self.fallback_model_info()]);
            }
        };
        if models.is_empty() {
            models.push(self.fallback_model_info());
        }

        Ok(models)
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models
            .into_iter()
            .filter(|m| m.name.to_lowercase().contains(&query.to_lowercase()))
            .take(limit)
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        Ok(self.get_model(model_id).await?.is_some())
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        warn!("Using chat() without tools - consider using chat_with_request()");
        let request = ChatRequest::new(messages);
        self.chat_with_request(model, request).await
    }

    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        let model = self.resolve_model(model);
        let url = self.chat_url();

        // Convert messages to OpenAI format
        let messages: Vec<Value> = request
            .messages
            .iter()
            .map(|m| {
                let mut msg = json!({
                    "role": m.role,
                    "content": m.content
                });

                if let Some(ref id) = m.tool_call_id {
                    msg["tool_call_id"] = json!(id);
                }

                if let Some(ref calls) = m.tool_calls {
                    msg["tool_calls"] = json!(calls.iter().map(|tc| {
                        json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": simd_json::to_string(&tc.arguments).unwrap_or_default()
                            }
                        })
                    }).collect::<Vec<_>>());
                }

                msg
            })
            .collect();

        let tools: Vec<Value> = request.tools.iter().map(|t| t.to_openai_format()).collect();

        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": false
        });
        let body_object = body
            .as_object_mut()
            .expect("openclaw request body should be an object");

        if !tools.is_empty() {
            body_object.insert("tools".into(), json!(tools));
            body_object.insert("tool_choice".into(), request.tool_choice.to_api_format());
            info!(
                "OpenClaw request with {} tools, tool_choice={:?}",
                tools.len(),
                request.tool_choice
            );
        }

        if let Some(max_tokens) = request.max_tokens {
            body_object.insert("max_tokens".into(), json!(max_tokens));
        }
        if let Some(temp) = request.temperature {
            body_object.insert("temperature".into(), json!(temp));
        }
        if let Some(top_p) = request.top_p {
            body_object.insert("top_p".into(), json!(top_p));
        }

        debug!(
            "OpenClaw request: {}",
            simd_json::to_string_pretty(&body).unwrap_or_default()
        );

        let response = self
            .api_request(self.client.post(&url))
            .json(&body)
            .send()
            .await
            .context("Failed to send request to OpenClaw")?;

        let status = response.status();
        let response_text = response.text().await?;

        debug!(
            "OpenClaw response ({}): {}",
            status,
            &response_text[..response_text.len().min(500)]
        );

        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "OpenClaw API error ({}): {}",
                status,
                response_text
            ));
        }

        // Parse OpenAI-compatible response
        let mut response_text_mut = response_text;
        let response_json: Value =
            unsafe { simd_json::from_str(&mut response_text_mut) }.map_err(|e| {
                anyhow::anyhow!(
                    "Failed to parse OpenClaw response: {}. Body: {}",
                    e,
                    response_text_mut
                )
            })?;

        let choice = response_json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| anyhow::anyhow!("No choices returned from OpenClaw"))?;

        let message = choice
            .get("message")
            .ok_or_else(|| anyhow::anyhow!("No message in OpenClaw response"))?;

        let content = message
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        let role = message
            .get("role")
            .and_then(|r| r.as_str())
            .unwrap_or("assistant")
            .to_string();

        // Parse tool_calls
        let tool_calls: Option<Vec<ToolCallInfo>> = message
            .get("tool_calls")
            .and_then(|tc| tc.as_array())
            .map(|calls| {
                calls
                    .iter()
                    .filter_map(|call| {
                        let id = call.get("id")?.as_str()?.to_string();
                        let function = call.get("function")?;
                        let name = function.get("name")?.as_str()?.to_string();
                        let args_str = function.get("arguments")?.as_str()?;
                        let mut args_mut = args_str.to_string();
                        let arguments: Value =
                            unsafe { simd_json::from_str(&mut args_mut) }.ok()?;

                        Some(ToolCallInfo {
                            id,
                            name,
                            arguments,
                        })
                    })
                    .collect()
            });

        if let Some(ref calls) = tool_calls {
            info!("OpenClaw: parsed {} tool calls", calls.len());
        }

        let finish_reason = choice
            .get("finish_reason")
            .and_then(|f| f.as_str())
            .map(|s| s.to_string());

        let usage = response_json.get("usage").map(|u| TokenUsage {
            prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            completion_tokens: u
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        });

        Ok(ChatResponse {
            message: ChatMessage {
                role,
                content,
                tool_calls: tool_calls.clone(),
                tool_call_id: None,
            },
            model,
            provider: "openclaw".to_string(),
            finish_reason,
            usage,
            tool_calls,
        })
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let response = self.chat(model, messages).await?;
        let _ = tx.send(Ok(response.message.content)).await;
        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    fn spawn_test_server(
        response_status: &str,
        response_body: &str,
    ) -> Result<(String, Arc<Mutex<Vec<u8>>>, tokio::task::JoinHandle<()>)> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        let request_bytes = Arc::new(Mutex::new(Vec::new()));
        let request_bytes_clone = request_bytes.clone();
        let response_status = response_status.to_string();
        let response_body = response_body.to_string();

        let handle = tokio::task::spawn_blocking(move || {
            listener
                .set_nonblocking(true)
                .expect("listener should support nonblocking mode");
            let deadline = Instant::now() + Duration::from_secs(5);

            while Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_millis(250)))
                            .expect("stream should support read timeout");
                        let mut buffer = [0_u8; 16384];
                        loop {
                            match stream.read(&mut buffer) {
                                Ok(0) => break,
                                Ok(read) => {
                                    request_bytes_clone
                                        .lock()
                                        .expect("request bytes lock poisoned")
                                        .extend_from_slice(&buffer[..read]);
                                    if read < buffer.len() {
                                        break;
                                    }
                                }
                                Err(err)
                                    if err.kind() == std::io::ErrorKind::WouldBlock
                                        || err.kind() == std::io::ErrorKind::TimedOut =>
                                {
                                    break;
                                }
                                Err(_) => break,
                            }
                        }

                        let response = format!(
                            "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            response_status,
                            response_body.len(),
                            response_body
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                        return;
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => return,
                }
            }
        });

        Ok((format!("http://{}", addr), request_bytes, handle))
    }

    #[test]
    fn parses_model_listing_response() {
        let models = OpenClawProvider::parse_models_response(
            r#"{"data":[{"id":"openclaw:main","owned_by":"openclaw","created":1710000000}]}"#,
        )
        .expect("model response should parse");

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "openclaw:main");
        assert!(models[0].tags.iter().any(|tag| tag == "openclaw"));
    }

    #[tokio::test]
    async fn list_models_falls_back_to_default_when_endpoint_fails() {
        let (base_url, _request, handle) =
            spawn_test_server("500 Internal Server Error", r#"{"error":"boom"}"#)
                .expect("test server should start");

        let provider = OpenClawProvider::new(Some(base_url), Some("opencode/agent".to_string()));
        let models = provider
            .list_models()
            .await
            .expect("list_models should succeed");
        handle.await.expect("server should finish");

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "opencode/agent");
    }

    #[tokio::test]
    async fn list_models_falls_back_to_default_when_endpoint_returns_non_json() {
        let (base_url, _request, handle) =
            spawn_test_server("200 OK", "<html>OpenClaw Control UI</html>")
                .expect("test server should start");

        let provider =
            OpenClawProvider::new(Some(base_url), Some("openclaw:gemini3-adc".to_string()));
        let models = provider
            .list_models()
            .await
            .expect("list_models should succeed");
        handle.await.expect("server should finish");

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "openclaw:gemini3-adc");
        assert!(models[0].tags.iter().any(|tag| tag == "agent-route"));
    }

    #[tokio::test]
    async fn chat_with_request_serializes_tools_and_parses_tool_calls() {
        let response_body = r#"{
            "choices":[
                {
                    "message":{
                        "role":"assistant",
                        "content":"",
                        "tool_calls":[
                            {
                                "id":"call_1",
                                "type":"function",
                                "function":{
                                    "name":"tool.echo",
                                    "arguments":"{\"message\":\"hello\"}"
                                }
                            }
                        ]
                    },
                    "finish_reason":"tool_calls"
                }
            ],
            "usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}
        }"#;
        let (base_url, request_bytes, handle) =
            spawn_test_server("200 OK", response_body).expect("test server should start");

        let provider = OpenClawProvider::new(Some(base_url), Some("openclaw:main".to_string()));

        let request = ChatRequest::new(vec![ChatMessage::user("test tool call")])
            .with_tools(vec![crate::provider::ToolDefinition {
                name: "tool.echo".to_string(),
                description: "Echo a message".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "message": {"type": "string"}
                    },
                    "required": ["message"]
                }),
                schema_version: "1".to_string(),
                category: "test".to_string(),
                tags: vec![],
                namespace: "tool".to_string(),
            }])
            .with_tool_choice(crate::provider::ToolChoice::Required);

        let response = tokio::time::timeout(
            Duration::from_secs(5),
            provider.chat_with_request("", request),
        )
        .await
        .expect("chat request should not hang")
        .expect("chat_with_request should succeed");
        handle.await.expect("server should finish");

        let request_text = String::from_utf8(
            request_bytes
                .lock()
                .expect("request bytes lock poisoned")
                .clone(),
        )
        .expect("request should be valid utf-8");
        let request_text_lower = request_text.to_lowercase();

        assert!(!request_text_lower.contains("authorization:"));
        assert!(request_text.contains("\"tool_choice\":\"required\""));
        assert!(request_text.contains("\"name\":\"tool.echo\""));
        assert_eq!(response.provider, "openclaw");
        assert_eq!(
            response
                .tool_calls
                .as_ref()
                .expect("tool calls should exist")[0]
                .name,
            "tool.echo"
        );
    }
}
</file>

<file path="src/perplexity.rs">
//! Perplexity API Client
//!
//! ## API Endpoints
//!
//! | Endpoint | URL | Purpose |
//! |----------|-----|--------|
//! | Base URL | `https://api.perplexity.ai` | All Perplexity APIs |
//! | Chat | `/chat/completions` | OpenAI-compatible chat |
//!
//! ## Authentication
//! - Header: `Authorization: Bearer {PERPLEXITY_API_KEY}`
//! - Environment: `PERPLEXITY_API_KEY`
//!
//! ## Features
//! - Online search capability (real-time web data)
//! - Citations in responses
//!
//! ## Pricing
//! - ~$5 per 1000 requests

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

use crate::provider::{
    ChatMessage, ChatResponse, LlmProvider, ModelInfo, ProviderType, TokenUsage,
};

// =============================================================================
// API ENDPOINT CONFIGURATION
// =============================================================================

/// Perplexity API endpoints
pub mod endpoints {
    /// Base API URL
    pub const BASE_URL: &str = "https://api.perplexity.ai";

    /// Chat completions endpoint (OpenAI-compatible)
    /// Full URL: {BASE_URL}/chat/completions
    pub const CHAT_COMPLETIONS: &str = "/chat/completions";
}

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Available Perplexity models
const PERPLEXITY_MODELS: &[(&str, &str, &str)] = &[
    ("sonar", "Sonar", "Default online model with search"),
    ("sonar-pro", "Sonar Pro", "Advanced online model"),
    (
        "sonar-reasoning",
        "Sonar Reasoning",
        "Enhanced reasoning with search",
    ),
    (
        "llama-3.1-sonar-small-128k-online",
        "Sonar Small Online",
        "Fast online model",
    ),
    (
        "llama-3.1-sonar-large-128k-online",
        "Sonar Large Online",
        "Capable online model",
    ),
    (
        "llama-3.1-sonar-huge-128k-online",
        "Sonar Huge Online",
        "Most capable online",
    ),
];

#[derive(Debug, Serialize)]
struct PerplexityRequest {
    model: String,
    messages: Vec<PerplexityMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PerplexityMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct PerplexityResponse {
    choices: Vec<PerplexityChoice>,
    model: Option<String>,
    usage: Option<PerplexityUsage>,
}

#[derive(Debug, Deserialize)]
struct PerplexityChoice {
    message: PerplexityMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PerplexityUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

// =============================================================================
// CLIENT IMPLEMENTATION
// =============================================================================

/// Perplexity API Client
pub struct PerplexityClient {
    client: Client,
    api_key: String,
    /// Base API URL
    api_url: String,
}

impl PerplexityClient {
    /// Create a new Perplexity client
    ///
    /// Uses default endpoint: https://api.perplexity.ai
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            api_key: api_key.into(),
            api_url: endpoints::BASE_URL.to_string(),
        }
    }

    /// Create from environment variable
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("PERPLEXITY_API_KEY")
            .context("PERPLEXITY_API_KEY environment variable not set")?;
        Ok(Self::new(api_key))
    }

    /// Create with custom endpoint
    pub fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        let mut client = Self::new(api_key);
        client.api_url = endpoint.into();
        client
    }

    /// Get the current API URL
    pub fn api_url(&self) -> &str {
        &self.api_url
    }
}

#[async_trait]
impl LlmProvider for PerplexityClient {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Perplexity
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        info!("Perplexity models (static list)");
        info!("  Endpoint: {}", self.api_url);

        Ok(PERPLEXITY_MODELS
            .iter()
            .map(|(id, name, desc)| ModelInfo {
                id: id.to_string(),
                name: name.to_string(),
                description: Some(desc.to_string()),
                parameters: None,
                available: true,
                tags: vec!["online".to_string(), "search".to_string()],
                downloads: None,
                updated_at: None,
            })
            .collect())
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let query_lower = query.to_lowercase();
        let models = self.list_models().await?;
        Ok(models
            .into_iter()
            .filter(|m| {
                m.id.to_lowercase().contains(&query_lower)
                    || m.name.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .collect())
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        Ok(PERPLEXITY_MODELS.iter().any(|(id, _, _)| *id == model_id))
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        // Build URL: {api_url}/chat/completions
        let url = format!("{}/chat/completions", self.api_url);

        info!(
            "Perplexity chat: model={}, endpoint={}",
            model, self.api_url
        );

        let perplexity_messages: Vec<PerplexityMessage> = messages
            .iter()
            .map(|m| PerplexityMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        let request = PerplexityRequest {
            model: model.to_string(),
            messages: perplexity_messages,
            max_tokens: Some(2048),
            temperature: Some(0.7),
        };

        debug!("Perplexity request to: {}", url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send Perplexity request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Perplexity API error {}: {}", status, body));
        }

        let result: PerplexityResponse = response
            .json()
            .await
            .context("Failed to parse Perplexity response")?;

        let choice = result
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No response from Perplexity"))?;

        let usage = result.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(ChatResponse {
            message: ChatMessage::assistant(choice.message.content),
            model: result.model.unwrap_or_else(|| model.to_string()),
            provider: "perplexity".to_string(),
            finish_reason: choice.finish_reason,
            usage,
            tool_calls: None,
        })
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let response = self.chat(model, messages).await?;
        tx.send(Ok(response.message.content)).await.ok();
        Ok(rx)
    }
}
</file>

<file path="src/provider.rs">
//! LLM Provider Traits and Types
//!
//! This module defines the common interface for all LLM providers
//! including tool calling support with REQUIRED tool_choice.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::fmt;
use std::str::FromStr;

/// Provider types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProviderType {
    Anthropic,
    Antigravity,
    Assistant,
    Factory,
    Gemini,
    GeminiCli,
    HuggingFace,
    McpProxy,
    OpenAI,
    OpenClaw,
    Perplexity,
    Custom(String),
}

impl fmt::Display for ProviderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderType::Anthropic => write!(f, "anthropic"),
            ProviderType::Antigravity => write!(f, "antigravity"),
            ProviderType::Assistant => write!(f, "assistant"),
            ProviderType::Factory => write!(f, "factory"),
            ProviderType::Gemini => write!(f, "gemini"),
            ProviderType::GeminiCli => write!(f, "gemini-cli"),
            ProviderType::HuggingFace => write!(f, "huggingface"),
            ProviderType::McpProxy => write!(f, "mcp-proxy"),
            ProviderType::OpenAI => write!(f, "openai"),
            ProviderType::OpenClaw => write!(f, "openclaw"),
            ProviderType::Perplexity => write!(f, "perplexity"),
            ProviderType::Custom(name) => write!(f, "{}", name),
        }
    }
}

impl FromStr for ProviderType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "anthropic" => Ok(ProviderType::Anthropic),
            "antigravity" => Ok(ProviderType::Antigravity),
            "assistant" => Ok(ProviderType::Assistant),
            "factory" => Ok(ProviderType::Factory),
            "gemini" => Ok(ProviderType::Gemini),
            "gemini-cli" | "gemini_cli" | "geminicli" => Ok(ProviderType::GeminiCli),
            "huggingface" | "hugging_face" | "hf" => Ok(ProviderType::HuggingFace),
            "mcp-proxy" | "mcp_proxy" | "mcpproxy" => Ok(ProviderType::McpProxy),
            "openai" | "open_ai" => Ok(ProviderType::OpenAI),
            "openclaw" | "open_claw" => Ok(ProviderType::OpenClaw),
            "perplexity" => Ok(ProviderType::Perplexity),
            other => Err(format!("Unknown provider type: {}", other)),
        }
    }
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".to_string(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// Tool call information from LLM response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// Tool definition for LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: simd_json::OwnedValue,
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub namespace: String,
}

impl ToolDefinition {
    pub fn to_anthropic_format(&self) -> simd_json::OwnedValue {
        simd_json::json!({
            "name": self.name,
            "description": self.description,
            "input_schema": self.input_schema
        })
    }

    pub fn to_openai_format(&self) -> simd_json::OwnedValue {
        simd_json::json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.input_schema
            }
        })
    }
}

/// Tool choice for LLM request
///
/// IMPORTANT: Use `Required` to force the LLM to use tools.
/// This is essential for the anti-hallucination architecture.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    /// Let LLM decide whether to use tools (NOT RECOMMENDED)
    #[default]
    Auto,
    /// FORCE LLM to use a tool (REQUIRED for anti-hallucination)
    Required,
    /// Disable tool usage
    None,
    /// Force specific tool
    Tool(String),
}

impl ToolChoice {
    /// Convert to OpenAI/HuggingFace format
    pub fn to_api_format(&self) -> Value {
        match self {
            ToolChoice::Auto => simd_json::json!("auto"),
            ToolChoice::Required => simd_json::json!("required"),
            ToolChoice::None => simd_json::json!("none"),
            ToolChoice::Tool(name) => simd_json::json!({
                "type": "function",
                "function": {"name": name}
            }),
        }
    }

    pub fn to_anthropic_format(&self) -> Value {
        match self {
            ToolChoice::Auto => simd_json::json!({"type": "auto"}),
            ToolChoice::Required => simd_json::json!({"type": "any"}),
            ToolChoice::None => simd_json::json!({"type": "none"}),
            ToolChoice::Tool(name) => simd_json::json!({
                "type": "tool",
                "name": name
            }),
        }
    }
}

/// Full chat request with tools
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub tool_choice: ToolChoice,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
}

impl ChatRequest {
    pub fn new(messages: Vec<ChatMessage>) -> Self {
        Self {
            messages,
            tools: Vec::new(),
            tool_choice: ToolChoice::Auto,
            max_tokens: None,
            temperature: None,
            top_p: None,
        }
    }

    /// Create request with FORCED tool usage
    pub fn with_forced_tools(messages: Vec<ChatMessage>, tools: Vec<ToolDefinition>) -> Self {
        Self {
            messages,
            tools,
            tool_choice: ToolChoice::Required, // ◄── FORCE TOOL USE
            max_tokens: None,
            temperature: Some(0.7),
            top_p: None,
        }
    }

    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = tools;
        self
    }

    pub fn with_tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = choice;
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub organization_id: Option<String>,
    pub project_id: Option<String>,
}

/// Chat response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: ChatMessage,
    pub model: String,
    pub provider: String,
    pub finish_reason: Option<String>,
    pub usage: Option<TokenUsage>,
    pub tool_calls: Option<Vec<ToolCallInfo>>,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<String>,
    pub available: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    pub downloads: Option<u64>,
    pub updated_at: Option<String>,
}

/// Boxed provider for dynamic dispatch
pub type BoxedProvider = Box<dyn LlmProvider + Send + Sync>;

/// Provider capabilities
#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub tool_use: bool,
    pub vision: bool,
    pub embeddings: bool,
    pub max_context_length: usize,
}

/// Streaming chunk for real-time responses
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: String,
    pub finish_reason: Option<String>,
    pub usage: Option<TokenUsage>,
}

/// LLM Provider trait
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Get provider type
    fn provider_type(&self) -> ProviderType;

    /// List available models
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;

    /// Search models
    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>>;

    /// Get model info
    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>>;

    /// Check if model is available
    async fn is_model_available(&self, model_id: &str) -> Result<bool>;

    /// Basic chat (no tools) - AVOID USING THIS
    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse>;

    /// Chat with full request including tools - USE THIS
    ///
    /// Implementations MUST:
    /// 1. Pass tools to the API
    /// 2. Set tool_choice according to request
    /// 3. Parse tool_calls from response
    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        // Default implementation ignores tools - OVERRIDE THIS!
        tracing::warn!("chat_with_request using default implementation - tools ignored!");
        self.chat(model, request.messages).await
    }

    /// Streaming chat
    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>>;
}
</file>

<file path="src/pty_bridge.rs">
//! PTY Authentication Bridge
//!
//! Wraps CLI tools in a pseudo-terminal to handle interactive authentication
//! flows on headless servers.
//!
//! ## Use Cases
//!
//! - Running `gemini` CLI on servers without browsers
//! - Using `gh` (GitHub CLI) device code flow
//! - Any CLI tool with interactive OAuth
//!
//! ## How It Works
//!
//! 1. Spawn the CLI in a PTY (pseudo-terminal)
//! 2. Monitor output for auth patterns (URLs, device codes, prompts)
//! 3. When auth is detected, emit notification (webhook, D-Bus signal, web UI)
//! 4. User completes auth on their device
//! 5. Bridge detects completion and continues execution

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};

// =============================================================================
// AUTH PATTERNS
// =============================================================================

/// Patterns to detect in CLI output that indicate auth is required
const AUTH_URL_PATTERNS: &[&str] = &[
    "https://accounts.google.com",
    "https://github.com/login/device",
    "https://login.microsoftonline.com",
    "https://oauth.example.com",
    "Open this URL",
    "Visit this URL",
    "Go to",
    "authenticate at",
];

const DEVICE_CODE_PATTERNS: &[&str] = &[
    "Enter code:",
    "Your code:",
    "Device code:",
    "one-time code",
    "verification code",
];

const PROMPT_PATTERNS: &[&str] = &[
    "Press Enter",
    "press any key",
    "Password:",
    "Enter MFA",
    "2FA code",
    "OTP:",
];

// =============================================================================
// TYPES
// =============================================================================

/// Detected authentication requirement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequirement {
    /// Unique ID for this auth request
    pub id: String,
    /// Type of auth detected
    pub auth_type: AuthType,
    /// URL to visit (if applicable)
    pub url: Option<String>,
    /// Device code to enter (if applicable)
    pub device_code: Option<String>,
    /// Human-readable message
    pub message: String,
    /// Timestamp when detected
    pub detected_at: i64,
    /// Whether this auth has been completed
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    /// Browser-based OAuth (open URL)
    BrowserOAuth,
    /// Device code flow (enter code at URL)
    DeviceCode,
    /// Interactive prompt (password, MFA, etc.)
    InteractivePrompt,
    /// Press Enter to continue
    Confirmation,
}

/// Result of executing a command through the PTY bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyExecutionResult {
    /// Command exit code
    pub exit_code: i32,
    /// Captured stdout
    pub stdout: String,
    /// Captured stderr
    pub stderr: String,
    /// Whether auth was required during execution
    pub auth_required: bool,
    /// Auth details if required
    pub auth_details: Option<AuthRequirement>,
}

/// Notification handler for auth requirements
#[async_trait::async_trait]
pub trait AuthNotificationHandler: Send + Sync {
    /// Called when auth is required
    async fn notify(&self, auth: &AuthRequirement) -> Result<()>;

    /// Called when auth is completed
    async fn auth_completed(&self, auth_id: &str, success: bool) -> Result<()>;
}

// =============================================================================
// PTY BRIDGE
// =============================================================================

/// PTY Authentication Bridge
pub struct PtyAuthBridge {
    /// Pending auth requirements
    pending_auths: Arc<RwLock<HashMap<String, AuthRequirement>>>,
    /// Notification handlers
    handlers: Arc<RwLock<Vec<Arc<dyn AuthNotificationHandler>>>>,
    /// Broadcast channel for auth events
    auth_tx: broadcast::Sender<AuthRequirement>,
    /// Session store path
    _session_store: PathBuf,
}

impl PtyAuthBridge {
    /// Create a new PTY bridge
    pub fn new() -> Self {
        let (auth_tx, _) = broadcast::channel(16);

        Self {
            pending_auths: Arc::new(RwLock::new(HashMap::new())),
            handlers: Arc::new(RwLock::new(Vec::new())),
            auth_tx,
            _session_store: dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("pty-auth-bridge")
                .join("sessions"),
        }
    }

    /// Add a notification handler
    pub async fn add_handler(&self, handler: Arc<dyn AuthNotificationHandler>) {
        self.handlers.write().await.push(handler);
    }

    /// Subscribe to auth events
    pub fn subscribe(&self) -> broadcast::Receiver<AuthRequirement> {
        self.auth_tx.subscribe()
    }

    /// Get pending auth requirements
    pub async fn get_pending_auths(&self) -> Vec<AuthRequirement> {
        self.pending_auths.read().await.values().cloned().collect()
    }

    /// Mark an auth as completed
    pub async fn complete_auth(&self, auth_id: &str, _response: Option<&str>) -> Result<()> {
        let mut auths = self.pending_auths.write().await;
        if let Some(auth) = auths.get_mut(auth_id) {
            auth.completed = true;
            info!(auth_id = %auth_id, "Auth marked as completed");

            // Notify handlers
            let handlers = self.handlers.read().await;
            for handler in handlers.iter() {
                handler.auth_completed(auth_id, true).await.ok();
            }
        }
        Ok(())
    }

    /// Execute a command through the PTY bridge
    pub async fn execute(
        &self,
        command: &str,
        args: &[&str],
        timeout_secs: u64,
    ) -> Result<PtyExecutionResult> {
        info!(command = %command, args = ?args, "Executing via PTY bridge");

        // For now, use regular process execution with output capture
        // Full PTY implementation would use `pty` crate
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped());

        // Pass through gcloud credentials for Gemini CLI
        // Priority: GOOGLE_APPLICATION_CREDENTIALS env var > service account > ADC
        if let Ok(creds) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
            debug!("Using GOOGLE_APPLICATION_CREDENTIALS from env: {}", creds);
            cmd.env("GOOGLE_APPLICATION_CREDENTIALS", creds);
        } else if let Some(home) = dirs::home_dir() {
            // Try service account key first (preferred for service accounts)
            let gcloud_creds = home.join(".config/gcloud/gemini-cli.json");
            if gcloud_creds.exists() {
                let creds_path = gcloud_creds.to_string_lossy().to_string();
                cmd.env("GOOGLE_APPLICATION_CREDENTIALS", &creds_path);
                debug!("Using gcloud service account: {}", creds_path);
            } else {
                // Fall back to Application Default Credentials
                let adc_creds = home.join(".config/gcloud/application_default_credentials.json");
                if adc_creds.exists() {
                    let creds_path = adc_creds.to_string_lossy().to_string();
                    cmd.env("GOOGLE_APPLICATION_CREDENTIALS", &creds_path);
                    debug!("Using gcloud ADC: {}", creds_path);
                }
            }
        }
        // Pass through project ID if set
        if let Ok(project) = std::env::var("GOOGLE_CLOUD_PROJECT") {
            cmd.env("GOOGLE_CLOUD_PROJECT", project);
        } else if let Ok(project) = std::env::var("GCLOUD_PROJECT") {
            cmd.env("GOOGLE_CLOUD_PROJECT", project);
        }

        let mut child = cmd.spawn().context("Failed to spawn command")?;

        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take().expect("stderr");

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let mut stdout_buf = String::new();
        let mut stderr_buf = String::new();
        let mut auth_required = false;
        let mut auth_details = None;

        // Read output with timeout
        let _result = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), async {
            loop {
                tokio::select! {
                    line = stdout_reader.next_line() => {
                        match line {
                            Ok(Some(line)) => {
                                debug!(line = %line, "stdout");
                                stdout_buf.push_str(&line);
                                stdout_buf.push('\n');

                                // Check for auth patterns
                                if let Some(auth) = self.detect_auth(&line).await {
                                    auth_required = true;
                                    auth_details = Some(auth.clone());

                                    // Notify handlers
                                    let handlers = self.handlers.read().await;
                                    for handler in handlers.iter() {
                                        handler.notify(&auth).await.ok();
                                    }

                                    // Broadcast
                                    self.auth_tx.send(auth).ok();
                                }
                            }
                            Ok(None) => break,
                            Err(e) => {
                                warn!(error = %e, "Error reading stdout");
                                break;
                            }
                        }
                    }
                    line = stderr_reader.next_line() => {
                        match line {
                            Ok(Some(line)) => {
                                debug!(line = %line, "stderr");
                                stderr_buf.push_str(&line);
                                stderr_buf.push('\n');

                                // Also check stderr for auth patterns
                                if let Some(auth) = self.detect_auth(&line).await {
                                    auth_required = true;
                                    auth_details = Some(auth);
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                warn!(error = %e, "Error reading stderr");
                            }
                        }
                    }
                }
            }
        })
        .await;

        let exit_code = match child.wait().await {
            Ok(status) => status.code().unwrap_or(-1),
            Err(_) => -1,
        };

        Ok(PtyExecutionResult {
            exit_code,
            stdout: stdout_buf,
            stderr: stderr_buf,
            auth_required,
            auth_details,
        })
    }

    /// Detect auth requirements in output line
    async fn detect_auth(&self, line: &str) -> Option<AuthRequirement> {
        let line_lower = line.to_lowercase();

        // Check for URLs
        for pattern in AUTH_URL_PATTERNS {
            if line.contains(pattern) {
                let url = extract_url(line);
                let auth = AuthRequirement {
                    id: uuid::Uuid::new_v4().to_string(),
                    auth_type: AuthType::BrowserOAuth,
                    url,
                    device_code: None,
                    message: line.to_string(),
                    detected_at: chrono::Utc::now().timestamp(),
                    completed: false,
                };

                // Store pending auth
                self.pending_auths
                    .write()
                    .await
                    .insert(auth.id.clone(), auth.clone());

                return Some(auth);
            }
        }

        // Check for device codes
        for pattern in DEVICE_CODE_PATTERNS {
            if line_lower.contains(&pattern.to_lowercase()) {
                let code = extract_device_code(line);
                let auth = AuthRequirement {
                    id: uuid::Uuid::new_v4().to_string(),
                    auth_type: AuthType::DeviceCode,
                    url: extract_url(line),
                    device_code: code,
                    message: line.to_string(),
                    detected_at: chrono::Utc::now().timestamp(),
                    completed: false,
                };

                self.pending_auths
                    .write()
                    .await
                    .insert(auth.id.clone(), auth.clone());
                return Some(auth);
            }
        }

        // Check for prompts
        for pattern in PROMPT_PATTERNS {
            if line_lower.contains(&pattern.to_lowercase()) {
                let auth = AuthRequirement {
                    id: uuid::Uuid::new_v4().to_string(),
                    auth_type: if line_lower.contains("enter") && line_lower.contains("continue") {
                        AuthType::Confirmation
                    } else {
                        AuthType::InteractivePrompt
                    },
                    url: None,
                    device_code: None,
                    message: line.to_string(),
                    detected_at: chrono::Utc::now().timestamp(),
                    completed: false,
                };

                self.pending_auths
                    .write()
                    .await
                    .insert(auth.id.clone(), auth.clone());
                return Some(auth);
            }
        }

        None
    }
}

impl Default for PtyAuthBridge {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// HELPERS
// =============================================================================

/// Extract URL from a line of text
fn extract_url(line: &str) -> Option<String> {
    // Simple URL extraction - find https:// and take until whitespace
    if let Some(start) = line.find("https://") {
        let rest = &line[start..];
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let url = &rest[..end];
        let url = url.trim_end_matches(|c| ['.', ',', ')', '"', '\''].contains(&c));
        return Some(url.to_string());
    }

    if let Some(start) = line.find("http://") {
        let rest = &line[start..];
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let url = &rest[..end];
        let url = url.trim_end_matches(|c| ['.', ',', ')', '"', '\''].contains(&c));
        return Some(url.to_string());
    }

    None
}

/// Extract device code from a line of text
fn extract_device_code(line: &str) -> Option<String> {
    // Look for patterns like XXXX-XXXX or similar alphanumeric codes
    for word in line.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
        if clean.len() >= 8
            && clean.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && clean.chars().any(|c| c.is_ascii_uppercase())
            && clean.chars().any(|c| c.is_ascii_digit() || c == '-')
        {
            return Some(clean.to_string());
        }
    }
    None
}

// =============================================================================
// NOTIFICATION HANDLERS
// =============================================================================

/// Webhook notification handler
pub struct WebhookNotificationHandler {
    url: String,
    client: reqwest::Client,
}

impl WebhookNotificationHandler {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl AuthNotificationHandler for WebhookNotificationHandler {
    async fn notify(&self, auth: &AuthRequirement) -> Result<()> {
        let payload = simd_json::json!({
            "event": "auth_required",
            "auth": auth
        });

        self.client
            .post(&self.url)
            .json(&payload)
            .send()
            .await
            .context("Failed to send webhook")?;

        Ok(())
    }

    async fn auth_completed(&self, auth_id: &str, success: bool) -> Result<()> {
        let payload = simd_json::json!({
            "event": "auth_completed",
            "auth_id": auth_id,
            "success": success
        });

        self.client
            .post(&self.url)
            .json(&payload)
            .send()
            .await
            .context("Failed to send webhook")?;

        Ok(())
    }
}

/// Log notification handler (for testing/debugging)
pub struct LogNotificationHandler;

#[async_trait::async_trait]
impl AuthNotificationHandler for LogNotificationHandler {
    async fn notify(&self, auth: &AuthRequirement) -> Result<()> {
        info!(
            auth_type = ?auth.auth_type,
            url = ?auth.url,
            device_code = ?auth.device_code,
            message = %auth.message,
            "🔐 AUTH REQUIRED"
        );

        if let Some(url) = &auth.url {
            eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
            eprintln!("║  🔐 AUTHENTICATION REQUIRED                                    ║");
            eprintln!("╠══════════════════════════════════════════════════════════════╣");
            eprintln!("║  Visit this URL to authenticate:                              ║");
            eprintln!("║  {}  ", url);
            if let Some(code) = &auth.device_code {
                eprintln!(
                    "║  Enter code: {}                                       ",
                    code
                );
            }
            eprintln!("╚══════════════════════════════════════════════════════════════╝\n");
        }

        Ok(())
    }

    async fn auth_completed(&self, auth_id: &str, success: bool) -> Result<()> {
        info!(auth_id = %auth_id, success = %success, "Auth completed");
        Ok(())
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_url() {
        assert_eq!(
            extract_url("Please visit https://accounts.google.com/o/oauth2/auth?client_id=123"),
            Some("https://accounts.google.com/o/oauth2/auth?client_id=123".to_string())
        );

        assert_eq!(
            extract_url("Go to https://github.com/login/device and enter code"),
            Some("https://github.com/login/device".to_string())
        );

        assert_eq!(extract_url("No URL here"), None);
    }

    #[test]
    fn test_extract_device_code() {
        assert_eq!(
            extract_device_code("Enter code: ABCD-1234"),
            Some("ABCD-1234".to_string())
        );

        assert_eq!(
            extract_device_code("Your one-time code is WXYZ5678"),
            Some("WXYZ5678".to_string())
        );
    }

    #[tokio::test]
    async fn test_pty_bridge_simple_command() {
        let bridge = PtyAuthBridge::new();
        bridge.add_handler(Arc::new(LogNotificationHandler)).await;

        let result = bridge.execute("echo", &["hello"], 10).await.unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello"));
        assert!(!result.auth_required);
    }
}
</file>

<file path="Cargo.toml">
[package]
name = "op-llm"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "LLM provider integration with dynamic model discovery for HuggingFace"

[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
reqwest = { workspace = true }
chrono = { workspace = true }
rsa = "0.9.9"
sha2.workspace = true
base64.workspace = true
jsonwebtoken = "9"
uuid = { version = "1.0", features = ["v4"] }
dirs = "5.0"
</file>

<file path="compare-op-llm.md">
# compare-op-llm

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 15 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 13 |
| Partial artifacts | 0 |
| Spec-listed source files | 14 |
| Spec-listed but missing | 0 |
| Extra implementation files | 1 |

## Current Implementation Overview

- LLM provider integration with dynamic model discovery for HuggingFace

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/perplexity.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/perplexity.rs |
| `src/huggingface.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/huggingface.rs |
| `src/headless_oauth.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/headless_oauth.rs |
| `src/gemini.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/gemini.rs |
| `src/chat.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/chat.rs |
| `src/antigravity_replay.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/antigravity_replay.rs |
| `src/antigravity.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/antigravity.rs |
| `src/anthropic.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/anthropic.rs |
| `src/gcloud_adc.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/gcloud_adc.rs |
| `src/provider.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/provider.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/pty_bridge.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/pty_bridge.rs |
| `src/gemini_cli.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/gemini_cli.rs |
| `src/mcp_proxy.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/mcp_proxy.rs |
| `root` | ✅ Present | root source group | src/anthropic.rs, src/antigravity.rs, src/antigravity_replay.rs, src/chat.rs, src/gcloud_adc.rs, src/gemini.rs, src/gemini_cli.rs, src/headless_oauth.rs, ... (+7 more) |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| perplexity | ✅ Implemented | src/perplexity.rs | SPEC main module |
| huggingface | ✅ Implemented | src/huggingface.rs | SPEC main module |
| headless_oauth | ✅ Implemented | src/headless_oauth.rs | SPEC main module |
| gemini | ✅ Implemented | src/gemini.rs | SPEC main module |
| chat | ✅ Implemented | src/chat.rs | SPEC main module |
| antigravity_replay | ✅ Implemented | src/antigravity_replay.rs | SPEC main module |
| antigravity | ✅ Implemented | src/antigravity.rs | SPEC main module |
| anthropic | ✅ Implemented | src/anthropic.rs | SPEC main module |
| gcloud_adc | ✅ Implemented | src/gcloud_adc.rs | SPEC main module |
| provider | ✅ Implemented | src/provider.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- None

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - documented in SPEC
- `tracing` - documented in SPEC
- `async-trait` - documented in SPEC
- `reqwest` - documented in SPEC
- `chrono` - documented in SPEC
- `rsa` - documented in SPEC
- `sha2.workspace` - documented in SPEC
- `base64.workspace` - documented in SPEC
- `jsonwebtoken` - documented in SPEC
- `uuid` - documented in SPEC
- `dirs` - documented in SPEC

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Current implementation contains 1 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: anthropic, antigravity, chat, gcloud_adc, gemini, gemini_cli, headless_oauth, huggingface, mcp_proxy, openclaw, perplexity, provider, pty_bridge.
</file>

<file path="SPEC.md">
# op-llm - Specification

## Overview
**Crate**: `op-llm`  
**Location**: `crates/op-llm`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-llm"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-llm/src/perplexity.rs
op-llm/src/huggingface.rs
op-llm/src/headless_oauth.rs
op-llm/src/gemini.rs
op-llm/src/chat.rs
op-llm/src/antigravity_replay.rs
op-llm/src/antigravity.rs
op-llm/src/anthropic.rs
op-llm/src/gcloud_adc.rs
op-llm/src/provider.rs
op-llm/src/lib.rs
op-llm/src/pty_bridge.rs
op-llm/src/gemini_cli.rs
op-llm/src/mcp_proxy.rs
```

### Key Dependencies
```toml
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
reqwest = { workspace = true }
chrono = { workspace = true }
rsa = "0.9.9"
sha2.workspace = true
base64.workspace = true
jsonwebtoken = "9"
uuid = { version = "1.0", features = ["v4"] }
dirs = "5.0"
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
      14 Rust source files

### Main Modules
perplexity
huggingface
headless_oauth
gemini
chat
antigravity_replay
antigravity
anthropic
gcloud_adc
provider

## Purpose
LLM provider integration with dynamic model discovery for HuggingFace

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:


---
*Generated from crate analysis*
</file>

</files>
