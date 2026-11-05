#!/usr/bin/env python3
"""
setup_project_management.py
Sets up GitHub labels, milestones, imports the 19 active issues, and creates
a Master Roadmap tracking issue on devenvoy/open-download-manager.
"""

import json
import os
import re
import subprocess
import sys
import time

TARGET_REPO = "devenvoy/open-download-manager"

LABELS = [
    # Types
    {"name": "type: goal", "color": "5319e7", "description": "Tracking goal delivered through linked implementation steps"},
    {"name": "type: feature", "color": "0e8a16", "description": "New feature or capability"},
    {"name": "type: enhancement", "color": "a2eeef", "description": "Improvement to existing functionality"},
    {"name": "type: bug", "color": "d73a4a", "description": "Something isn't working as expected"},
    {"name": "type: documentation", "color": "0075ca", "description": "Improvements or additions to documentation"},
    {"name": "type: infra", "color": "6f42c1", "description": "CI, build tooling, or test infrastructure"},
    # Areas
    {"name": "area: desktop", "color": "1d76db", "description": "Desktop client application UI and Tauri shell"},
    {"name": "area: extension", "color": "f9d0c4", "description": "Browser extension and native messaging bridge"},
    {"name": "area: core-rust", "color": "e99695", "description": "Rust core download engine and transfer services"},
    {"name": "area: packaging", "color": "c2e0c6", "description": "Packaging, installers, binaries, and OS integration"},
    {"name": "area: media", "color": "d4c5f9", "description": "Video detection, format streaming, and audio/video muxing"},
    {"name": "area: network", "color": "bfd4f2", "description": "Network protocols, proxy, TLS, and transport"},
    {"name": "area: ci", "color": "5319e7", "description": "Continuous integration, workflows, and test harnesses"},
    # Status
    {"name": "status: ready", "color": "0e8a16", "description": "Ready for implementation; no blocking prerequisites"},
    {"name": "status: blocked", "color": "b60205", "description": "Waiting for linked prerequisite to merge and pass validation"},
    {"name": "status: in-progress", "color": "fbca04", "description": "Actively being worked on"},
    # Priority
    {"name": "priority: high", "color": "b60205", "description": "High priority item"},
    {"name": "priority: medium", "color": "fbca04", "description": "Medium priority item"},
    {"name": "priority: low", "color": "0e8a16", "description": "Low priority item"},
    # Community
    {"name": "help wanted", "color": "008672", "description": "Extra attention or community contribution is welcome"},
    {"name": "good first issue", "color": "7057ff", "description": "Good for newcomers"},
]

MILESTONES = [
    {
        "title": "v0.2.0 - Core Engine & UI Polish",
        "description": "Core reliability improvements, media stream capture hardening, and desktop UI theming & options.",
    },
    {
        "title": "v0.3.0 - Browser & Network Parity",
        "description": "Cross-browser support including Firefox, proxy configurations, and contributor CI checks.",
    },
    {
        "title": "v0.4.0 - Advanced Queues & Batch Link Grabbing",
        "description": "Named download queues, recurring schedules, and bounded website link grabbing.",
    },
    {
        "title": "v1.0.0 - Cross-Platform Release",
        "description": "Native platform packaging, installers, and signing for Linux, macOS, and Windows ARM64.",
    },
]

ISSUE_CONFIGS = [
    # v0.2.0
    {
        "file": "issues/issue-025-enhance-youtube-video-session-capture-before-playback.md",
        "title": "Enhance YouTube video session capture before playback",
        "milestone": "v0.2.0 - Core Engine & UI Polish",
        "labels": ["type: bug", "area: media", "area: extension", "status: ready", "priority: high"],
    },
    {
        "file": "issues/issue-007-recover-interrupted-transfers-and-refresh-expired-media-link.md",
        "title": "Recover interrupted transfers and refresh expired media links",
        "milestone": "v0.2.0 - Core Engine & UI Polish",
        "labels": ["type: enhancement", "area: core-rust", "area: media", "status: ready", "help wanted", "priority: high"],
    },
    {
        "file": "issues/issue-021-add-light-dark-and-system-theme-options-to-the-desktop-app.md",
        "title": "Add light, dark, and system theme options to the desktop app",
        "milestone": "v0.2.0 - Core Engine & UI Polish",
        "labels": ["type: enhancement", "area: desktop", "status: ready", "priority: medium"],
    },
    {
        "file": "issues/issue-022-improve-the-browser-video-format-picker-ui.md",
        "title": "Improve the browser video format picker UI",
        "milestone": "v0.2.0 - Core Engine & UI Polish",
        "labels": ["type: enhancement", "area: extension", "status: ready", "priority: medium"],
    },
    {
        "file": "issues/issue-008-polish-extension-dropdown-dismissal-and-video-button-positio.md",
        "title": "Polish extension dropdown dismissal and video-button positioning",
        "milestone": "v0.2.0 - Core Engine & UI Polish",
        "labels": ["type: enhancement", "area: extension", "status: ready", "help wanted", "priority: low"],
    },
    {
        "file": "issues/issue-023-docs-add-interface-screenshots-to-the-readme-and-documentati.md",
        "title": "[Docs] Add interface screenshots to the README and Documentation",
        "milestone": "v0.2.0 - Core Engine & UI Polish",
        "labels": ["type: documentation", "status: ready", "good first issue", "priority: low"],
    },

    # v0.3.0
    {
        "file": "issues/issue-005-goal-firefox-support.md",
        "title": "[Goal] Firefox support",
        "milestone": "v0.3.0 - Browser & Network Parity",
        "labels": ["type: goal", "area: extension", "help wanted", "priority: high"],
    },
    {
        "file": "issues/issue-014-firefox-1-2-media-capture-and-native-messaging-compatibility.md",
        "title": "[Firefox 1/2] Media capture and native messaging compatibility",
        "milestone": "v0.3.0 - Browser & Network Parity",
        "labels": ["type: enhancement", "area: extension", "status: ready", "help wanted", "priority: high"],
    },
    {
        "file": "issues/issue-015-firefox-2-2-persistent-extension-distribution-and-guided-set.md",
        "title": "[Firefox 2/2] Persistent extension distribution and guided setup",
        "milestone": "v0.3.0 - Browser & Network Parity",
        "labels": ["type: enhancement", "area: extension", "status: blocked", "help wanted", "priority: medium"],
    },
    {
        "file": "issues/issue-024-http-and-https-proxy-support.md",
        "title": "HTTP and HTTPS proxy support",
        "milestone": "v0.3.0 - Browser & Network Parity",
        "labels": ["type: enhancement", "area: network", "area: core-rust", "status: ready", "priority: high"],
    },
    {
        "file": "issues/issue-006-require-safe-contributor-ci-and-platform-regression-checks.md",
        "title": "Require safe contributor CI and platform regression checks",
        "milestone": "v0.3.0 - Browser & Network Parity",
        "labels": ["type: infra", "area: ci", "status: ready", "help wanted", "priority: medium"],
    },

    # v0.4.0
    {
        "file": "issues/issue-009-add-named-download-queues-and-recurring-schedules.md",
        "title": "Add named download queues and recurring schedules",
        "milestone": "v0.4.0 - Advanced Queues & Batch Link Grabbing",
        "labels": ["type: enhancement", "area: core-rust", "area: desktop", "status: ready", "help wanted", "priority: medium"],
    },
    {
        "file": "issues/issue-010-implement-bounded-website-link-grabbing.md",
        "title": "Implement bounded website/link grabbing",
        "milestone": "v0.4.0 - Advanced Queues & Batch Link Grabbing",
        "labels": ["type: enhancement", "area: core-rust", "area: desktop", "status: ready", "help wanted", "priority: medium"],
    },

    # v1.0.0
    {
        "file": "issues/issue-002-goal-linux-support.md",
        "title": "[Goal] Linux support",
        "milestone": "v1.0.0 - Cross-Platform Release",
        "labels": ["type: goal", "area: packaging", "area: desktop", "help wanted", "priority: high"],
    },
    {
        "file": "issues/issue-011-linux-3-3-package-media-dependencies-and-validate-install-up.md",
        "title": "[Linux 3/3] Package media dependencies and validate install/upgrade/uninstall",
        "milestone": "v1.0.0 - Cross-Platform Release",
        "labels": ["type: enhancement", "area: packaging", "status: blocked", "help wanted", "priority: high"],
    },
    {
        "file": "issues/issue-003-goal-macos-support.md",
        "title": "[Goal] macOS support",
        "milestone": "v1.0.0 - Cross-Platform Release",
        "labels": ["type: goal", "area: packaging", "area: desktop", "help wanted", "priority: high"],
    },
    {
        "file": "issues/issue-012-macos-3-3-package-app-and-media-tools-signing-plan-and-clean.md",
        "title": "[macOS 3/3] Package app and media tools, signing plan and clean-install validation",
        "milestone": "v1.0.0 - Cross-Platform Release",
        "labels": ["type: enhancement", "area: packaging", "status: blocked", "help wanted", "priority: high"],
    },
    {
        "file": "issues/issue-004-goal-windows-arm64-support.md",
        "title": "[Goal] Windows ARM64 support",
        "milestone": "v1.0.0 - Cross-Platform Release",
        "labels": ["type: goal", "area: packaging", "help wanted", "priority: medium"],
    },
    {
        "file": "issues/issue-013-windows-arm64-3-3-installer-distinct-artifacts-and-hardware.md",
        "title": "[Windows ARM64 3/3] Installer, distinct artifacts and hardware validation",
        "milestone": "v1.0.0 - Cross-Platform Release",
        "labels": ["type: enhancement", "area: packaging", "status: blocked", "help wanted", "priority: medium"],
    },
]

def run_cmd(cmd, check=True):
    res = subprocess.run(cmd, capture_output=True, text=True)
    if check and res.returncode != 0:
        raise RuntimeError(f"Command failed ({res.returncode}): {' '.join(cmd)}\nStderr: {res.stderr}")
    return res

def sync_labels():
    print("=== 1. Syncing Labels ===")
    for lbl in LABELS:
        name = lbl["name"]
        color = lbl["color"].lstrip("#")
        desc = lbl.get("description", "")
        cmd = ["gh", "label", "create", name, "--repo", TARGET_REPO, "--color", color, "--description", desc, "--force"]
        run_cmd(cmd, check=False)
        print(f"  ✓ Label ready: {name} (#{color})")

def sync_milestones():
    print("\n=== 2. Syncing Milestones ===")
    existing_raw = run_cmd(["gh", "api", f"repos/{TARGET_REPO}/milestones"]).stdout
    existing = json.loads(existing_raw)
    existing_titles = {m["title"]: m for m in existing}

    for m in MILESTONES:
        title = m["title"]
        desc = m["description"]
        if title in existing_titles:
            print(f"  ✓ Milestone exists: {title}")
        else:
            run_cmd(["gh", "api", f"repos/{TARGET_REPO}/milestones", "-f", f"title={title}", "-f", f"description={desc}"])
            print(f"  + Created milestone: {title}")

def extract_body_from_file(file_path):
    with open(file_path, "r", encoding="utf-8") as f:
        content = f.read()

    # Split at '## Description\n\n'
    if "## Description\n\n" in content:
        body = content.split("## Description\n\n", 1)[1].strip()
    else:
        # Fallback: strip yaml frontmatter
        body = re.sub(r"^---\n.*?\n---\n", "", content, flags=re.DOTALL).strip()
    return body

def create_issues():
    print("\n=== 3. Creating Issues on GitHub ===")
    created_issues = []

    for cfg in ISSUE_CONFIGS:
        file_path = cfg["file"]
        title = cfg["title"]
        milestone = cfg["milestone"]
        labels = cfg["labels"]

        body = extract_body_from_file(file_path)

        cmd = [
            "gh", "issue", "create",
            "--repo", TARGET_REPO,
            "--title", title,
            "--body", body,
            "--milestone", milestone,
        ]
        for l in labels:
            cmd.extend(["--label", l])

        res = run_cmd(cmd)
        issue_url = res.stdout.strip()
        issue_num = int(issue_url.split("/")[-1])
        print(f"  ✓ Issue #{issue_num}: '{title}' -> {issue_url}")

        created_issues.append({
            "number": issue_num,
            "title": title,
            "url": issue_url,
            "milestone": milestone,
            "labels": labels,
            "file": file_path,
        })
        time.sleep(0.5)

    return created_issues

def create_master_tracking_issue(created_issues):
    print("\n=== 4. Creating Master Roadmap & Project Tracker Issue ===")
    
    # Group issues by milestone
    by_milestone = {}
    for m in MILESTONES:
        by_milestone[m["title"]] = []
    
    for item in created_issues:
        by_milestone[item["milestone"]].append(item)

    body_lines = [
        "# 🎯 Project Roadmap & Master Backlog Tracker",
        "",
        "This master tracking issue organizes the active development backlog into planned release milestones.",
        "Track task completion, prerequisites, and milestone status here.",
        "",
        "---",
        "",
    ]

    for m in MILESTONES:
        title = m["title"]
        desc = m["description"]
        items = by_milestone.get(title, [])
        body_lines.append(f"## 📌 Milestone: `{title}`")
        body_lines.append(f"> *{desc}*")
        body_lines.append("")
        for item in items:
            num = item["number"]
            it_title = item["title"]
            labels_str = " ".join([f"`{l}`" for l in item["labels"]])
            body_lines.append(f"- [ ] #{num} — **{it_title}** {labels_str}")
        body_lines.append("")
        body_lines.append("---")
        body_lines.append("")

    body_lines.append("## 🏆 Completely Implemented / Architecture-Eaten Baseline")
    body_lines.append("The 13 legacy issues from Electron that were completely solved by the Tauri v2 + Rust rewrite are documented in detail in `reference/MIGRATION-RESOLVED-ISSUES.md` (e.g. YouTube playlist queueing, native macOS/Linux adapters, sustained high-throughput downloads without V8 pauses, segmented fallback).")

    full_body = "\n".join(body_lines)

    cmd = [
        "gh", "issue", "create",
        "--repo", TARGET_REPO,
        "--title", "🎯 Master Roadmap & Project Backlog Tracker",
        "--body", full_body,
        "--label", "type: goal",
        "--label", "priority: high",
    ]
    res = run_cmd(cmd)
    tracker_url = res.stdout.strip()
    tracker_num = int(tracker_url.split("/")[-1])
    print(f"  ✓ Master Roadmap Issue #{tracker_num} created: {tracker_url}")

    # Pin the tracking issue if possible
    pin_cmd = ["gh", "issue", "pin", str(tracker_num), "--repo", TARGET_REPO]
    run_cmd(pin_cmd, check=False)
    print(f"  ✓ Pinned Master Roadmap Issue #{tracker_num}")

    return tracker_num, tracker_url

def main():
    print(f"Target repository: {TARGET_REPO}")
    sync_labels()
    sync_milestones()
    created = create_issues()
    tracker_num, tracker_url = create_master_tracking_issue(created)

    # Save mapping for reference
    with open("issues/created_issues_map.json", "w", encoding="utf-8") as f:
        json.dump({
            "tracker": {"number": tracker_num, "url": tracker_url},
            "issues": created
        }, f, indent=2)

    print("\n🎉 Project management and issue setup complete!")

if __name__ == "__main__":
    main()
