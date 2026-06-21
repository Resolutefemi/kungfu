#!/usr/bin/env python3
"""Push files to GitHub via the Contents API, one file at a time.
This bypasses git-over-HTTPS auth issues and works with fine-grained PATs
that have Contents:Read-and-write on a single repo.

Usage:
    GITHUB_TOKEN=github_pat_xxx python3 push-via-api.py
"""
import base64
import os
import subprocess
import sys
import time
import urllib.request
import json

REPO = "Resolutefemi/kungfu"
TOKEN = os.environ.get("GITHUB_TOKEN")
if not TOKEN:
    print("ERROR: GITHUB_TOKEN not set")
    sys.exit(1)

def gh(method, path, data=None):
    url = f"https://api.github.com/repos/{REPO}/{path}"
    body = json.dumps(data).encode() if data else None
    req = urllib.request.Request(
        url,
        method=method,
        data=body,
        headers={
            "Authorization": f"Bearer {TOKEN}",
            "Accept": "application/vnd.github+json",
            "Content-Type": "application/json",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    try:
        with urllib.request.urlopen(req) as r:
            return json.loads(r.read()) if r.status != 204 else {}
    except urllib.error.HTTPError as e:
        return {"error": e.read().decode(), "status": e.code}

# Get the list of files we want to push, in commit order.
# We use git log to get the order: each commit's diff is one file.
result = subprocess.run(
    ["git", "log", "--reverse", "--name-only", "--pretty=format:COMMIT:%s"],
    capture_output=True, text=True, check=True,
)

# Parse: each block is "COMMIT:<msg>\n<file>\n\n"
commits = []
current_msg = None
current_files = []
for line in result.stdout.split("\n"):
    if line.startswith("COMMIT:"):
        if current_msg and current_files:
            commits.append((current_msg, current_files))
        current_msg = line[len("COMMIT:"):]
        current_files = []
    elif line.strip():
        current_files.append(line.strip())
if current_msg and current_files:
    commits.append((current_msg, current_files))

print(f"Pushing {len(commits)} commits via GitHub Contents API...")
print(f"(Each commit pushes its file(s) individually — slow but works with restricted tokens.)")
print()

for i, (msg, files) in enumerate(commits, 1):
    for path in files:
        if not os.path.exists(path):
            continue
        with open(path, "rb") as f:
            content = base64.b64encode(f.read()).decode()

        # Check if file exists (to get its SHA for update).
        existing = gh("GET", f"contents/{path}")
        sha = existing.get("sha") if "sha" in existing else None

        payload = {
            "message": msg,
            "content": content,
            "branch": "main",
        }
        if sha:
            payload["sha"] = sha

        resp = gh("PUT", f"contents/{path}", payload)
        if "error" in resp:
            print(f"  ✗ [{i}/{len(commits)}] {msg} ({path}): {resp['status']}")
            print(f"     {resp['error'][:200]}")
            sys.exit(1)
        print(f"  ✓ [{i}/{len(commits)}] {msg} ({path})")
        time.sleep(0.1)  # avoid rate limit

print()
print(f"✓ Done. Pushed {len(commits)} commits to https://github.com/{REPO}")
