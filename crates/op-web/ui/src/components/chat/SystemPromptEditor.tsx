import { useState } from "react";
import { Card, Callout } from "@/components/shell/Primitives";
import { cn } from "@/lib/utils";
import { Lock, Pencil, RotateCcw, Copy, Check } from "lucide-react";

/** Immutable core rules — loaded from backend, never editable in UI */
const IMMUTABLE_PROMPT = `## CRITICAL RULES (Immutable)

1. **ALWAYS USE TOOLS** — For ANY system operation, you MUST call the appropriate tool.
   Never describe steps for the operator to run manually.

2. **NEVER SUGGEST CLI COMMANDS** — Do NOT mention or suggest commands like:
   - ovs-vsctl, ovs-ofctl → use ovs_* tools
   - systemctl, service, dinitctl → use dbus_dinit_* tools
   - ip, ifconfig, nmcli → use network tools

3. **NATIVE PROTOCOLS ONLY**:
   - D-Bus for dinit, NetworkManager, PackageKit
   - OVSDB JSON-RPC for Open vSwitch
   - rtnetlink for kernel networking
   - NEVER shell out to CLI tools

4. **TOOL CALL FORMAT**:
   <tool_call>tool_name({"arg1": "value1"})</tool_call>

5. **SECURITY**: Never expose secrets, keys, or credentials in responses.
   Validate all inputs before tool execution.

6. **EXPLAIN → ACT → REPORT**: Always explain what you will do,
   execute the tool, then report the result.`;

const DEFAULT_TUNABLE = `## Operator Preferences (Tunable)

# Response style
response_verbosity: concise
explanation_depth: brief
confirm_destructive: true

# Tool behavior
auto_chain_tools: true
max_tool_calls_per_turn: 5
show_raw_tool_output: false

# Context
environment: production
default_bus: system
preferred_service_manager: dinit

# Custom instructions
custom_instructions: |
  Prioritize safety over speed.
  Always check service status before restart.
  Log all destructive operations.`;

export function SystemPromptEditor() {
  const [tunable, setTunable] = useState(DEFAULT_TUNABLE);
  const [savedTunable, setSavedTunable] = useState(DEFAULT_TUNABLE);
  const [copied, setCopied] = useState<"immutable" | "tunable" | null>(null);
  const isDirty = tunable !== savedTunable;

  const handleSave = () => {
    setSavedTunable(tunable);
    // TODO: POST to /api/config/system-prompt with tunable section
  };

  const handleReset = () => {
    setTunable(DEFAULT_TUNABLE);
  };

  const handleCopy = (text: string, section: "immutable" | "tunable") => {
    navigator.clipboard.writeText(text);
    setCopied(section);
    setTimeout(() => setCopied(null), 1500);
  };

  return (
    <div className="flex flex-col h-full overflow-auto">
      <div className="px-4 py-3 border-b border-border shrink-0">
        <h2 className="text-lg font-semibold text-foreground">System Prompt</h2>
        <p className="text-xs text-muted-foreground mt-0.5">
          Core rules are immutable. Operator preferences can be tuned per session.
        </p>
      </div>

      <div className="flex-1 overflow-auto px-4 py-4 space-y-4">
        {/* Immutable Section */}
        <Card
          title="Core Rules"
          actions={
            <div className="flex items-center gap-2">
              <span className="inline-flex items-center gap-1 text-[10px] font-medium text-muted-foreground bg-muted/50 border border-border rounded px-2 py-0.5">
                <Lock className="h-3 w-3" /> Read-only
              </span>
              <button
                onClick={() => handleCopy(IMMUTABLE_PROMPT, "immutable")}
                className="p-1.5 rounded hover:bg-muted/30 text-muted-foreground hover:text-foreground transition-colors"
                title="Copy to clipboard"
              >
                {copied === "immutable" ? <Check className="h-3.5 w-3.5 text-ok" /> : <Copy className="h-3.5 w-3.5" />}
              </button>
            </div>
          }
        >
          <pre className="font-mono text-[11px] leading-relaxed text-muted-foreground whitespace-pre-wrap bg-background/50 rounded-md border border-border p-3 max-h-[300px] overflow-auto select-text">
            {IMMUTABLE_PROMPT}
          </pre>
          <Callout variant="warn" className="mt-3 text-xs">
            These rules are enforced by the backend and cannot be modified from the UI.
            They ensure the LLM always uses native tool calls instead of suggesting CLI commands.
          </Callout>
        </Card>

        {/* Tunable Section */}
        <Card
          title="Operator Preferences"
          subtitle="Editable per-session. Changes apply to the next message sent."
          actions={
            <div className="flex items-center gap-2">
              <span className="inline-flex items-center gap-1 text-[10px] font-medium text-primary bg-primary/10 border border-primary/20 rounded px-2 py-0.5">
                <Pencil className="h-3 w-3" /> Editable
              </span>
              <button
                onClick={() => handleCopy(tunable, "tunable")}
                className="p-1.5 rounded hover:bg-muted/30 text-muted-foreground hover:text-foreground transition-colors"
                title="Copy to clipboard"
              >
                {copied === "tunable" ? <Check className="h-3.5 w-3.5 text-ok" /> : <Copy className="h-3.5 w-3.5" />}
              </button>
            </div>
          }
        >
          <textarea
            value={tunable}
            onChange={(e) => setTunable(e.target.value)}
            className="w-full font-mono text-[11px] leading-relaxed text-foreground bg-background/50 rounded-md border border-border p-3 min-h-[260px] max-h-[400px] resize-y outline-none focus:border-ring focus:ring-1 focus:ring-ring transition-colors"
            spellCheck={false}
          />
          <div className="flex items-center justify-between mt-3">
            <div className="flex items-center gap-2">
              {isDirty && (
                <span className="text-[10px] text-warn font-medium">Unsaved changes</span>
              )}
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={handleReset}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md border border-border text-xs font-medium text-muted-foreground hover:text-foreground hover:bg-muted/30 transition-colors"
              >
                <RotateCcw className="h-3 w-3" /> Reset defaults
              </button>
              <button
                onClick={handleSave}
                disabled={!isDirty}
                className="px-3 py-1.5 rounded-md bg-primary text-primary-foreground text-xs font-medium hover:bg-primary/90 transition-colors disabled:opacity-50"
              >
                Apply to session
              </button>
            </div>
          </div>
        </Card>

        {/* Combined preview */}
        <Card title="Effective Prompt Preview" subtitle="What gets sent to the LLM as the system message.">
          <pre className="font-mono text-[10px] leading-relaxed text-muted-foreground whitespace-pre-wrap bg-background/50 rounded-md border border-border p-3 max-h-[200px] overflow-auto select-text">
            {IMMUTABLE_PROMPT + "\n\n" + tunable}
          </pre>
          <div className="mt-2 text-[10px] text-muted-foreground">
            {(IMMUTABLE_PROMPT + "\n\n" + tunable).length.toLocaleString()} characters · {(IMMUTABLE_PROMPT + "\n\n" + tunable).split("\n").length} lines
          </div>
        </Card>
      </div>
    </div>
  );
}
