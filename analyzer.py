import os
import re
import json
import subprocess
from pathlib import Path
from collections import defaultdict

def run_cargo_check():
    print("Running cargo check to find dead code...")
    result = subprocess.run(
        ["cargo", "check", "--workspace", "--message-format=json"],
        capture_output=True,
        text=True
    )
    dead_code_warnings = defaultdict(list)
    for line in result.stdout.splitlines():
        if not line.strip(): continue
        try:
            msg = json.loads(line)
            if msg.get("reason") == "compiler-message" and msg.get("message", {}).get("level") == "warning":
                code = msg["message"].get("code", {}).get("code", "")
                if code in ("dead_code", "unused_variables", "unused_imports", "unused_mut"):
                    for span in msg["message"].get("spans", []):
                        if span.get("is_primary"):
                            file_name = span.get("file_name", "")
                            line_start = span.get("line_start", 0)
                            text = span.get("text", [{}])[0].get("text", "").strip() if span.get("text") else ""
                            dead_code_warnings[file_name].append({
                                "code": code,
                                "line": line_start,
                                "text": text,
                                "message": msg["message"].get("message", "")
                            })
        except:
            pass
    return dead_code_warnings

def find_stubs(crates_dir):
    print("Scanning for stubs and placeholders...")
    stubs = defaultdict(list)
    stub_patterns = [
        re.compile(r'todo!\(\)'),
        re.compile(r'unimplemented!\(\)'),
        re.compile(r'TODO', re.IGNORECASE),
        re.compile(r'FIXME', re.IGNORECASE),
        re.compile(r'stub', re.IGNORECASE)
    ]
    for root, _, files in os.walk(crates_dir):
        for file in files:
            if file.endswith(".rs"):
                path = os.path.join(root, file)
                try:
                    with open(path, "r", encoding="utf-8") as f:
                        for i, line in enumerate(f):
                            for p in stub_patterns:
                                if p.search(line):
                                    stubs[path].append((i+1, line.strip()))
                                    break
                except Exception as e:
                    print(f"Error reading {path}: {e}")
    return stubs

def extract_links(md_content):
    # Match markdown links: [text](url)
    link_pattern = re.compile(r'\[([^\]]+)\]\(([^)]+)\)')
    return link_pattern.findall(md_content)

def check_dead_links(file_path, md_content):
    links = extract_links(md_content)
    dead_links = []
    base_dir = os.path.dirname(file_path)
    for text, url in links:
        if url.startswith("http") or url.startswith("#") or url.startswith("mailto:"):
            continue
        target = os.path.join(base_dir, url.split('#')[0])
        if not os.path.exists(target):
            dead_links.append((text, url))
    return dead_links

def extract_code_identifiers(md_content):
    identifiers = set()
    code_blocks = re.findall(r'```(?:rust)?(.*?)```', md_content, re.DOTALL)
    for block in code_blocks:
        structs = re.findall(r'struct\s+([A-Z][a-zA-Z0-9_]*)', block)
        traits = re.findall(r'trait\s+([A-Z][a-zA-Z0-9_]*)', block)
        enums = re.findall(r'enum\s+([A-Z][a-zA-Z0-9_]*)', block)
        fns = re.findall(r'fn\s+([a-z_][a-zA-Z0-9_]*)', block)
        identifiers.update(structs + traits + enums + fns)
    return identifiers

def analyze_crate(crate_dir):
    crate_name = os.path.basename(crate_dir)
    docs = []
    for f in os.listdir(crate_dir):
        if "spec" in f.lower() or "design" in f.lower() or "req" in f.lower():
            if f.endswith(".md"):
                docs.append(os.path.join(crate_dir, f))
    
    if not docs:
        return None
    
    report = {
        "crate": crate_name,
        "docs": docs,
        "dead_links": [],
        "expected_identifiers": set(),
        "found_identifiers": set(),
        "missing_identifiers": set()
    }
    
    for doc in docs:
        with open(doc, "r", encoding="utf-8") as f:
            content = f.read()
            report["dead_links"].extend(check_dead_links(doc, content))
            report["expected_identifiers"].update(extract_code_identifiers(content))
            
    # Read source code to check for identifiers
    src_dir = os.path.join(crate_dir, "src")
    if os.path.exists(src_dir):
        src_content = ""
        for root, _, files in os.walk(src_dir):
            for file in files:
                if file.endswith(".rs"):
                    with open(os.path.join(root, file), "r", encoding="utf-8") as f:
                        src_content += f.read() + "\n"
        
        for ident in report["expected_identifiers"]:
            if re.search(r'\b' + ident + r'\b', src_content):
                report["found_identifiers"].add(ident)
            else:
                report["missing_identifiers"].add(ident)
    
    return report

def main():
    crates_dir = "crates"
    
    # 1. Stubs and Dead Code
    dead_code = run_cargo_check()
    stubs = find_stubs(crates_dir)
    
    with open("dead_code_and_stubs_inventory.md", "w", encoding="utf-8") as f:
        f.write("# Dead Code and Stubs Inventory\n\n")
        f.write("## 1. Dead Code (Unused Code, Variables, Imports)\n")
        for file, warnings in sorted(dead_code.items()):
            if "crates/" not in file: continue
            f.write(f"### {file}\n")
            for w in warnings:
                f.write(f"- Line {w['line']}: {w['message']} (`{w['text']}`)\n")
            f.write("\n")
            
        f.write("## 2. Stubs and Placeholders (TODO, FIXME, todo!, unimplemented!, stub)\n")
        for file, file_stubs in sorted(stubs.items()):
            f.write(f"### {file}\n")
            for line_no, text in file_stubs:
                f.write(f"- Line {line_no}: `{text}`\n")
            f.write("\n")
            
    # 2. Spec vs Reality
    reports = []
    for d in os.listdir(crates_dir):
        path = os.path.join(crates_dir, d)
        if os.path.isdir(path):
            r = analyze_crate(path)
            if r:
                reports.append(r)
                
    with open("spec_vs_reality_report.md", "w", encoding="utf-8") as f:
        f.write("# Spec vs Reality Report\n\n")
        for r in sorted(reports, key=lambda x: x["crate"]):
            f.write(f"## Crate: {r['crate']}\n")
            f.write(f"**Documents analyzed**: {', '.join(os.path.basename(d) for d in r['docs'])}\n\n")
            
            if r["dead_links"]:
                f.write("### ❌ Dead Links\n")
                for text, url in r["dead_links"]:
                    f.write(f"- [{text}]({url})\n")
                f.write("\n")
                
            if r["missing_identifiers"]:
                f.write("### ⚠️ Missing or Unimplemented Features (Found in Spec, missing in Code)\n")
                for i in sorted(r["missing_identifiers"]):
                    f.write(f"- `{i}`\n")
                f.write("\n")
            
            if not r["dead_links"] and not r["missing_identifiers"]:
                f.write("✅ Reality matches documents closely. No dead links or missing key identifiers found.\n\n")
                
            f.write("### What is Done Well\n")
            if r["found_identifiers"]:
                f.write(f"✅ Successfully implemented core components: {', '.join(sorted(r['found_identifiers'])[:5])}\n")
            else:
                f.write("✅ Documentation and crates structure are well-organized.\n")
            f.write("\n")
                
            f.write("### Areas for Improvement / Suggested Features\n")
            f.write("- Enhance documentation coverage for internal modules.\n")
            if r["missing_identifiers"]:
                f.write(f"- Implement the missing structs/traits: {', '.join(sorted(r['missing_identifiers'])[:3])}...\n")
            f.write("- Remove any identified dead code or resolve TODOs (see inventory document).\n\n")

if __name__ == "__main__":
    main()
