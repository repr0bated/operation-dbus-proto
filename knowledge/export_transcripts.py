import json, os, glob, datetime
SRC="/home/jeremy/.claude/projects/-home-jeremy-git-operation-dbus-proto"
OUT="/home/jeremy/git/operation-dbus-proto/knowledge/transcripts"
def text_of(msg):
    c=msg.get("content")
    if isinstance(c,str): return c
    if isinstance(c,list):
        out=[]
        for b in c:
            if isinstance(b,dict) and b.get("type")=="text":
                out.append(b.get("text",""))
        return "\n".join(out)
    return ""
total=0
for f in sorted(glob.glob(f"{SRC}/*.jsonl")):
    sid=os.path.basename(f)[:8]
    rows=[]
    first_ts=None
    with open(f) as fh:
        for line in fh:
            try: o=json.loads(line)
            except: continue
            m=o.get("message") or {}
            role=m.get("role") or o.get("type")
            if role not in ("user","assistant"): continue
            t=text_of(m).strip()
            if not t: continue
            # skip pure system-reminder / local-command noise
            if t.startswith("<system-reminder>") and t.endswith("</system-reminder>"): continue
            if t.startswith("Caveat:") or t.startswith("<local-command"): continue
            ts=o.get("timestamp","")
            if first_ts is None and ts: first_ts=ts
            rows.append((role,t))
    if not rows: continue
    date=(first_ts or "")[:10]
    out=f"{OUT}/{date}_{sid}.md"
    with open(out,"w") as w:
        w.write(f"# Transcript {sid} — {date}\n\n_Operation D-Bus / Ghostbridge Live design session._\n\n")
        for role,t in rows:
            who="**User:**" if role=="user" else "**Assistant:**"
            w.write(f"\n{who}\n\n{t}\n\n---\n")
    total+=1
print(f"exported {total} transcripts")
