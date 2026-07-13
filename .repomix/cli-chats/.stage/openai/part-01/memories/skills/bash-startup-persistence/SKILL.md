---
name: bash-startup-persistence
description: Use when the user wants a shell change to be persistent in Bash, such as env vars, completion wiring, or making a command work in both user and root shells.
argument-hint: "[variable/command] [persistence goal]"
disable-model-invocation: true
user-invocable: false
allowed-tools:
  - Read
  - Grep
  - Bash
---

# Bash startup persistence

## When to use

Use this when the user says things like "make it persistent in bash", "put it in bashrc", "completion should work automatically", or "put it in my user and root".

Do not use this for:
- repo-local config files that are not shell startup files
- one-shot current-session exports
- shell-agnostic profile work where the active shell is not Bash

## Inputs / context to gather

1. Confirm the target behavior.
   - exported env var
   - completion loading
   - alias/function/path update
2. Inspect the actual Bash startup chain on this host.
   - `~/.bashrc`
   - `~/.bash_profile`
   - `/root/.bashrc` when root behavior matters
3. Check whether the requested behavior must work for:
   - the current user only
   - login shells
   - child processes
   - root interactive shells too
4. For completion work, check whether Bash completion auto-loading exists.
   - On this host, `/usr/share/bash-completion/bash_completion` may be absent.

## Procedure

1. Read the exact startup files first instead of running broad recursive searches.
2. Identify the minimal persistent edit surface.
   - env var persistence: prefer a single `export NAME="value"` line
   - completion persistence: ensure the completion file is sourced and `complete -F ...` is declared
3. Remove duplicates or conflicting plain assignments when they would mask the intended behavior.
4. If login-shell behavior matters, verify whether `~/.bash_profile` sources `~/.bashrc`.
5. If root behavior matters, inspect and patch `/root/.bashrc` separately rather than assuming a system install is enough.
6. For completion work on this host, do not assume `/etc/bash_completion.d/<name>` will auto-load by itself.
7. Verify in a fresh shell without printing secrets.
   - env var example:
   - `bash -lc 'case ${NAME+x} in x) printf "present\n";; *) printf "missing\n"; exit 1;; esac; env | rg "^NAME=" >/dev/null && printf "exported\n"'`
   - completion example:
   - `bash -ic 'complete -p <cmd>'`

## Efficiency plan

1. Start with `~/.bashrc`, `~/.bash_profile`, and `/root/.bashrc` when relevant.
2. Avoid broad `rg` over home directories unless the startup path is genuinely unclear.
3. Reuse the existing shell-init pattern already present in the file instead of inventing a new layout.
4. Stop after fresh-shell verification succeeds for the requested scope.

## Pitfalls and fixes

- Symptom: the variable exists in `~/.bashrc` but child processes still do not see it
  - Likely cause: plain `NAME=value` assignment instead of `export NAME="value"`.
  - Fix: consolidate to one exported line and verify with `bash -lc` plus an `env` check.

- Symptom: completion file exists under `/etc/bash_completion.d` but the command still falls back to filename completion
  - Likely cause: the bash-completion framework is not installed or not sourced.
  - Fix: add an explicit source line plus `complete -F <fn> <cmd>` in the relevant startup file.

- Symptom: it works for the user but not for root
  - Likely cause: only the user startup files were patched.
  - Fix: inspect `/root/.bashrc` and wire the root shell separately when the user asked for both.

- Symptom: search output is huge and the run gets interrupted before the actual shell files are checked
  - Likely cause: broad recursive search started too early.
  - Fix: inspect the exact startup files first and widen only if the init chain is unclear.

## Verification checklist

- The intended startup files contain the expected final lines and no conflicting duplicate assignment remains.
- Login-shell behavior is verified when requested.
- Child-process visibility is verified for env vars when requested.
- User and root interactive shells are both verified when the task requires both.
- No secret value was printed in the transcript.

## Minimal examples

```bash
sed -n '1,140p' ~/.bashrc
sed -n '1,80p' ~/.bash_profile
bash -lc 'case ${XAI_API_KEY+x} in x) printf "XAI_API_KEY present\n";; *) exit 1;; esac; env | rg "^XAI_API_KEY=" >/dev/null'
bash -ic 'complete -p zcall'
```
