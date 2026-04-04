#!/usr/bin/env bash
set -euo pipefail

# Zenflow BYOM profile helper
# Source this script before launching Zenflow so the OpenClaw/Gemini 3 stack
# runs with your ADC-backed credentials and streams through the schema pipeline.
#
# The OpenClaw gateway handles Gemini auth internally via ADC.
# OPENCLAW_TOKEN is the gateway bearer token, not the ADC access token.

ZENFLOW_ADC_ACCOUNT=${ZENFLOW_ADC_ACCOUNT:-jeremy@3tched.com}
ZENFLOW_OPENCLAW_MODEL=${ZENFLOW_OPENCLAW_MODEL:-openclaw:gemini3-adc}
# Base URL only — op-llm appends /v1/chat/completions itself
ZENFLOW_OPENCLAW_BASE_URL=${ZENFLOW_OPENCLAW_BASE_URL:-http://127.0.0.1:18789}
# Gateway auth token (from openclaw.json gateway.auth.token)
ZENFLOW_OPENCLAW_TOKEN=${ZENFLOW_OPENCLAW_TOKEN:-${OPENCLAW_GATEWAY_TOKEN:-b6fb8429d8fd9e615305261050aa67d97489d566842a8cec}}

if ! command -v gcloud >/dev/null; then
  echo "gcloud is required to verify the ADC account; install Cloud SDK first."
  exit 1
fi

# Verify ADC is active for the configured account
gcloud auth print-access-token --account="$ZENFLOW_ADC_ACCOUNT" > /dev/null 2>&1 || {
  echo "ADC account $ZENFLOW_ADC_ACCOUNT is not authenticated. Run: gcloud auth login $ZENFLOW_ADC_ACCOUNT"
  exit 1
}

export LLM_PROVIDER=openclaw
export OPENCLAW_BASE_URL="$ZENFLOW_OPENCLAW_BASE_URL"
export OPENCLAW_MODEL="$ZENFLOW_OPENCLAW_MODEL"
export OPENCLAW_TOKEN="$ZENFLOW_OPENCLAW_TOKEN"
export OPENCLAW_DEFAULT_MODEL="$ZENFLOW_OPENCLAW_MODEL"
export ZENFLOW_PROFILE=byom
export ZENFLOW_ADC_ACCOUNT

cat <<EOF
Zenflow BYOM profile enabled:
  LLM_PROVIDER=$LLM_PROVIDER
  OPENCLAW_BASE_URL=$OPENCLAW_BASE_URL
  OPENCLAW_MODEL=$OPENCLAW_MODEL
  ADC account=$ZENFLOW_ADC_ACCOUNT
EOF
