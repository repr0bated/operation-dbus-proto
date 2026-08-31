import type { Spec } from "@json-render/react";

export interface ParsedContent {
  spec: Spec | null;
  text: string;
}

/**
 * Parse content string to extract embedded json-render spec and remaining text.
 *
 * Looks for JSON specs in two formats:
 * 1. Code fence: ```json { ... } ```
 * 2. Raw JSON object with "root" and "elements" keys
 *
 * Returns { spec, text } where:
 * - spec: Extracted Spec if found and valid, otherwise null
 * - text: Remaining content with spec removed (or full content if no spec found)
 */
export function parseContent(content: string): ParsedContent {
  if (!content) return { spec: null, text: "" };

  console.log(`[parseContent] input length=${content.length}, preview=${content.substring(0, 100)}`);

  // Try to extract spec from code fence first
  const codeFenceMatch = content.match(/```(?:json)?\s*\n([\s\S]*?)\n```/);
  if (codeFenceMatch && codeFenceMatch[1]) {
    try {
      const jsonStr = codeFenceMatch[1];
      const parsed = JSON.parse(jsonStr);
      if (isValidSpec(parsed)) {
        console.log(`[parseContent] ✓ Found spec in code fence`);
        // Remove the code fence from content
        const text = content
          .replace(codeFenceMatch[0], "")
          .trim();
        return { spec: parsed, text };
      }
    } catch (e) {
      console.log(`[parseContent] Code fence JSON parse error:`, (e as Error).message);
      // Fall through to raw JSON attempt
    }
  }

  // Try to extract raw JSON object with "root" and "elements"
  const jsonMatch = content.match(/\{[\s\S]*?"root"[\s\S]*?"elements"[\s\S]*?\}/);
  if (jsonMatch) {
    try {
      const parsed = JSON.parse(jsonMatch[0]);
      if (isValidSpec(parsed)) {
        console.log(`[parseContent] ✓ Found spec in raw JSON`);
        // Remove the JSON object from content
        const text = content
          .replace(jsonMatch[0], "")
          .trim();
        return { spec: parsed, text };
      }
    } catch {
      // Invalid JSON, treat as text
    }
  }

  console.log(`[parseContent] No spec found, treating as text only`);
  // No spec found, return all as text
  return { spec: null, text: content };
}

function isValidSpec(obj: unknown): obj is Spec {
  if (!obj || typeof obj !== "object") return false;
  const spec = obj as Record<string, unknown>;
  return (
    typeof spec.root === "string" &&
    typeof spec.elements === "object" &&
    spec.elements !== null
  );
}
