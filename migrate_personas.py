import os
import glob
import re
import yaml

dirs = ["language", "infrastructure", "analysis", "security", "architecture", "business", "content", "database", "mobile", "seo", "specialty", "webframeworks"]

personas = []

for d in dirs:
    path = f"crates/op-agents/src/agents/{d}/*.rs"
    files = glob.glob(path)
    for fpath in files:
        with open(fpath, "r") as f:
            content = f.read()

        agent_type = re.search(r'fn agent_type\(&self\) -> &str {\s*"([^"]+)"', content)
        if not agent_type:
            continue
        agent_type = agent_type.group(1)

        name = re.search(r'fn name\(&self\) -> &str {\s*"([^"]+)"', content)
        name = name.group(1) if name else agent_type

        desc = re.search(r'fn description\(&self\) -> &str {\s*"([^"]+)"', content)
        desc = desc.group(1) if desc else ""

        # Extract operations properly handling newlines and .to_string()
        ops_match = re.search(r'fn operations\(&self\) -> Vec<String> {([^}]+)\}', content)
        operations = []
        if ops_match:
            ops_str = ops_match.group(1)
            operations = [m.group(1) for m in re.finditer(r'"([^"]+)"', ops_str)]

        # Try to find a profile
        profile = re.search(r'profile:\s*SecurityProfile::[a-zA-Z_]+"([^"]+)"', content)
        if not profile:
            profile = re.search(r'profile:\s*presets::([a-zA-Z_]+)\(\)', content)
        security_profile = profile.group(1) if profile else "default"

        # Try to extract system prompt from analyze or execute or constants
        system_prompt = ""
        recs = re.findall(r'recommendations\.push\("([^"]+)"\)', content)
        if recs:
            system_prompt += "Recommendations:\n" + "\n".join(recs)
        
        if not system_prompt:
            system_prompt = f"You are the {name}. {desc}"

        personas.append({
            "agent_type": agent_type,
            "name": name,
            "description": desc,
            "operations": operations,
            "system_prompt": system_prompt,
            "capabilities": ["llm", "tool_use"],
            "security_profile": security_profile,
        })

os.makedirs("config/agents", exist_ok=True)
with open("config/agents/personas.yaml", "w") as f:
    yaml.dump({"personas": personas}, f, default_flow_style=False, sort_keys=False)
    
print(f"Migrated {len(personas)} personas.")