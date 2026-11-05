#!/usr/bin/env python3
"""
import_issues_to_github.py
Recreates issues and labels from issues/ onto a target GitHub repository using GitHub CLI (`gh`).

Usage:
  python3 scripts/import_issues_to_github.py --dry-run
  python3 scripts/import_issues_to_github.py --target-repo devenvoy/open-download-manager
"""

import argparse
import json
import os
import subprocess
import sys
import time

def parse_args():
    parser = argparse.ArgumentParser(description="Import issues from issues/ directory to a GitHub repository.")
    parser.add_argument("--target-repo", help="Target GitHub repo (e.g. devenvoy/open-download-manager). Defaults to origin remote.")
    parser.add_argument("--dry-run", action="store_true", help="Preview what would be created without making changes.")
    parser.add_argument("--state", choices=["open", "closed", "all"], default="all", help="Which issues to import (default: all)")
    parser.add_argument("--yes", "-y", action="store_true", help="Skip confirmation prompt")
    return parser.parse_args()

def get_default_repo():
    try:
        url = subprocess.check_output(["git", "remote", "get-url", "origin"]).decode("utf-8").strip()
        # Parse github.com:owner/repo.git or https://github.com/owner/repo.git
        if "github.com" in url:
            part = url.split("github.com")[-1].lstrip("/:").rstrip(".git")
            return part
    except Exception:
        pass
    return None

def main():
    args = parse_args()
    target_repo = args.target_repo or get_default_repo()

    if not target_repo:
        print("Error: Could not determine target repository. Please specify --target-repo <owner/repo>", file=sys.stderr)
        sys.exit(1)

    issues_file = os.path.join("issues", "issues.json")
    labels_file = os.path.join("issues", "labels.json")

    if not os.path.isfile(issues_file):
        print(f"Error: {issues_file} not found. Run scripts/fetch_and_export_issues.py first.", file=sys.stderr)
        sys.exit(1)

    with open(issues_file, "r", encoding="utf-8") as f:
        issues = json.load(f)

    labels = []
    if os.path.isfile(labels_file):
        with open(labels_file, "r", encoding="utf-8") as f:
            labels = json.load(f)

    # Filter issues by state
    if args.state == "open":
        issues = [i for i in issues if i.get("state") == "OPEN"]
    elif args.state == "closed":
        issues = [i for i in issues if i.get("state") == "CLOSED"]

    print(f"Target repository: {target_repo}")
    print(f"Total issues to import: {len(issues)}")
    print(f"Total labels to ensure: {len(labels)}")
    print(f"Dry run mode: {args.dry_run}")

    if not args.dry_run and not args.yes:
        confirm = input(f"\nAre you sure you want to create labels and issues in '{target_repo}'? (y/N): ")
        if confirm.lower() != "y":
            print("Aborted.")
            sys.exit(0)

    # 1. Sync labels
    print("\n--- Syncing Labels ---")
    for lbl in labels:
        name = lbl["name"]
        color = lbl.get("color", "cccccc").lstrip("#")
        desc = lbl.get("description", "")
        if args.dry_run:
            print(f"[DRY-RUN] Create label: {name} (color: #{color}, desc: '{desc}')")
        else:
            print(f"Creating label: {name}...")
            cmd = ["gh", "label", "create", name, "--repo", target_repo, "--color", color, "--force"]
            if desc:
                cmd.extend(["--description", desc])
            subprocess.run(cmd, stdout=subprocess.DEVNULL)

    # 2. Create issues
    print("\n--- Creating Issues ---")
    for i in issues:
        num = i["number"]
        title = i["title"]
        orig_author = i.get("author", {}).get("login", "unknown") if i.get("author") else "unknown"
        orig_url = i.get("url", "")
        created_at = i.get("createdAt", "")
        state = i.get("state", "OPEN")
        labels_list = [l["name"] for l in i.get("labels", [])]
        comments = i.get("comments", [])

        # Compose body with provenance header
        body_lines = [
            f"> *Mirrored from upstream issue [#{num}]({orig_url}) originally opened by @{orig_author} on {created_at}*",
            "",
            i.get("body", "").strip() or "*No description provided.*",
        ]

        if comments:
            body_lines.append("\n---\n### Upstream Discussion History\n")
            for idx, c in enumerate(comments, 1):
                c_author = c.get("author", {}).get("login", "unknown") if c.get("author") else "unknown"
                c_date = c.get("createdAt", "")
                c_body = c.get("body", "").strip()
                body_lines.append(f"**Comment #{idx} by @{c_author} on {c_date}:**\n")
                body_lines.append(c_body)
                body_lines.append("\n---\n")

        full_body = "\n".join(body_lines)

        if args.dry_run:
            print(f"[DRY-RUN] Create issue #{num}: '{title}' ({state}) with labels: {labels_list}")
        else:
            print(f"Creating issue #{num}: '{title}'...")
            cmd = ["gh", "issue", "create", "--repo", target_repo, "--title", title, "--body", full_body]
            for lbl in labels_list:
                cmd.extend(["--label", lbl])
            res = subprocess.run(cmd, capture_output=True, text=True)
            if res.returncode == 0:
                created_url = res.stdout.strip()
                print(f"  -> Created: {created_url}")
                # If state was closed, close it
                if state == "CLOSED":
                    # Extract issue number from output or close by URL
                    close_cmd = ["gh", "issue", "close", created_url, "--repo", target_repo, "--reason", "completed"]
                    subprocess.run(close_cmd, stdout=subprocess.DEVNULL)
                    print(f"  -> Closed (matching upstream state)")
            else:
                print(f"  -> Failed: {res.stderr.strip()}", file=sys.stderr)
            # Avoid rate-limiting
            time.sleep(1)

    print("\nDone!")

if __name__ == "__main__":
    main()
