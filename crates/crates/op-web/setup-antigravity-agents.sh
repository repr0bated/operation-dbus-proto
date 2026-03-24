#!/bin/bash
set -e

echo "🚀 Setting up Google Antigravity IDE with ALL Agents"
echo ""

# Check if Antigravity config directory exists
ANTIGRAVITY_CONFIG_DIR="$HOME/.config/antigravity"
echo "📁 Antigravity config directory: $ANTIGRAVITY_CONFIG_DIR"

# Create directory if it doesn't exist
mkdir -p "$ANTIGRAVITY_CONFIG_DIR"

# Copy the full agent configuration
CONFIG_FILE="$ANTIGRAVITY_CONFIG_DIR/mcp.json"
cp antigravity-mcp-agents-config.json "$CONFIG_FILE"

echo "✅ Full agent configuration copied to: $CONFIG_FILE"

# Set proper permissions
chmod 644 "$CONFIG_FILE"

echo ""
echo "🤖 Your Antigravity IDE now has access to ALL agents:"
echo ""

# Language Agents
echo "💻 LANGUAGE AGENTS:"
echo "  ✅ rust-pro        - Rust development (cargo, clippy, fmt)"
echo "  ✅ python-pro      - Python development (pip, ruff, pytest)"
echo "  ✅ javascript-pro  - JavaScript/Node.js development"
echo "  ✅ typescript-pro  - TypeScript development"
echo "  ✅ golang-pro      - Go development"
echo "  ✅ java-pro        - Java development"
echo "  ✅ csharp-pro      - C# development"
echo "  ✅ cpp-pro         - C++ development"
echo "  ✅ c-pro           - C development"
echo "  ✅ php-pro         - PHP development"
echo "  ✅ ruby-pro        - Ruby development"
echo "  ✅ elixir-pro      - Elixir development"
echo "  ✅ scala-pro       - Scala development"
echo "  ✅ julia-pro       - Julia development"
echo "  ✅ bash-pro        - Shell scripting"
echo ""

# Infrastructure Agents
echo "🏗️  INFRASTRUCTURE AGENTS:"
echo "  ✅ kubernetes      - K8s operations"
echo "  ✅ terraform       - Infrastructure as Code"
echo "  ✅ cloud           - Multi-cloud operations"
echo "  ✅ deployment      - CI/CD and deployment"
echo "  ✅ network         - Network configuration"
echo ""

# Analysis Agents
echo "🔍 ANALYSIS AGENTS:"
echo "  ✅ code-reviewer   - Code review and analysis"
echo "  ✅ security-auditor- Security vulnerability scanning"
echo "  ✅ debugger        - Code debugging"
echo "  ✅ performance     - Performance analysis"
echo ""

# Database Agents
echo "🗄️  DATABASE AGENTS:"
echo "  ✅ sql-pro         - SQL development"
echo "  ✅ database-architect - Database design"
echo "  ✅ database-optimizer - Query optimization"
echo ""

# Content Agents
echo "📝 CONTENT AGENTS:"
echo "  ✅ docs-architect  - Documentation architecture"
echo "  ✅ tutorial-engineer - Tutorial creation"
echo "  ✅ api-documenter  - API documentation"
echo "  ✅ mermaid-expert  - Diagram generation"
echo ""

# Orchestration Agents
echo "🎯 ORCHESTRATION AGENTS:"
echo "  ✅ tdd-orchestrator - Test-driven development"
echo "  ✅ context-manager  - Context management"
echo "  ✅ dx-optimizer     - Developer experience"
echo ""

# Architecture Agents
echo "🏛️  ARCHITECTURE AGENTS:"
echo "  ✅ backend-architect  - Backend architecture"
echo "  ✅ frontend-developer - Frontend development"
echo "  ✅ graphql-architect  - GraphQL API design"
echo ""

# Operations Agents
echo "⚙️  OPERATIONS AGENTS:"
echo "  ✅ devops-troubleshooter - DevOps troubleshooting"
echo "  ✅ incident-responder    - Incident response"
echo "  ✅ test-automator        - Test automation"
echo ""

# AI/ML Agents
echo "🤖 AI/ML AGENTS:"
echo "  ✅ ai-engineer      - AI engineering"
echo "  ✅ ml-engineer      - Machine learning"
echo "  ✅ mlops-engineer   - MLOps"
echo "  ✅ data-engineer    - Data engineering"
echo "  ✅ data-scientist   - Data science"
echo "  ✅ prompt-engineer  - Prompt engineering"
echo ""

# Web Framework Agents
echo "🌐 WEB FRAMEWORK AGENTS:"
echo "  ✅ django-pro       - Django development"
echo "  ✅ fastapi-pro      - FastAPI development"
echo "  ✅ temporal-python-pro - Temporal workflows"
echo ""

# Mobile Agents
echo "📱 MOBILE AGENTS:"
echo "  ✅ flutter-expert   - Flutter development"
echo "  ✅ ios-developer    - iOS development"
echo "  ✅ mobile-developer - Cross-platform mobile"
echo ""

# Security Agents
echo "🔒 SECURITY AGENTS:"
echo "  ✅ backend-security-coder  - Backend security"
echo "  ✅ frontend-security-coder - Frontend security"
echo "  ✅ mobile-security-coder   - Mobile security"
echo ""

# Business Agents
echo "💼 BUSINESS AGENTS:"
echo "  ✅ business-analyst  - Business analysis"
echo "  ✅ sales-automator   - Sales automation"
echo "  ✅ customer-support  - Customer support"
echo "  ✅ hr-pro           - HR operations"
echo "  ✅ legal-advisor    - Legal advice"
echo "  ✅ payment-integration - Payment systems"
echo ""

# SEO Agents
echo "🎯 SEO AGENTS:"
echo "  ✅ seo-keyword-strategist - Keyword strategy"
echo "  ✅ seo-content-writer     - SEO content"
echo "  ✅ seo-meta-optimizer     - Meta optimization"
echo "  ✅ search-specialist      - Search optimization"
echo "  ✅ content-marketer       - Content marketing"
echo ""

# Specialty Agents
echo "🎨 SPECIALTY AGENTS:"
echo "  ✅ blockchain-developer  - Blockchain development"
echo "  ✅ unity-developer       - Unity game development"
echo "  ✅ quant-analyst         - Quantitative analysis"
echo "  ✅ arm-cortex-expert     - ARM development"
echo "  ✅ ui-ux-designer        - UI/UX design"
echo "  ✅ legacy-modernizer     - Legacy code modernization"
echo "  ✅ error-detective       - Error analysis"
echo "  ✅ observability-engineer- Observability"
echo "  ✅ hybrid-cloud-architect- Hybrid cloud architecture"
echo ""

echo "🎯 Next steps:"
echo "1. Start the op-web server (handles Chat UI + MCP Agents):"
echo "   cargo run --bin op-web-server"
echo "   # OR if using systemd:"
echo "   # sudo systemctl start op-web"
echo ""
echo "2. Open Google Antigravity IDE"
echo ""
echo "3. Use any of the 80+ specialized agents!"
echo ""
echo "4. Example commands in Antigravity:"
echo "   - 'Run rust-pro check on my project'"
echo "   - 'Use python-pro to format my code'"
echo "   - 'Have code-reviewer analyze this function'"
echo "   - 'Ask security-auditor to scan for vulnerabilities'"
echo ""

echo "📚 Documentation: See ANTIGRAVITY-AGENTS-README.md for detailed usage"
echo ""
echo "🎉 Welcome to the future of coding with 80+ specialized AI agents!"
