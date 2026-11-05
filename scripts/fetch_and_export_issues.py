#!/usr/bin/env python3
"""
fetch_and_export_issues.py
Fetch all issues and labels from upstream repository (Ahmadkhattak1/awesome-downloader)
and export them locally into issues/ directory with complete Markdown files and JSON backups.
"""

import json
import os
import re
import subprocess
import sys

UPSTREAM_REPO = "Ahmadkhattak1/awesome-downloader"
ISSUES_DIR = "issues"

def slugify(title: str) -> str:
    s = title.lower()
    s = re.sub(r'[^a-z0-9]+', '-', s).strip('-')
    return s[:60].rstrip('-')

def main():
    os.makedirs(ISSUES_DIR, exist_ok=True)

    print(f"Fetching labels from {UPSTREAM_REPO}...")
    try:
        labels_raw = subprocess.check_output([
            "gh", "label", "list", "--repo", UPSTREAM_REPO,
            "--json", "name,color,description"
        ]).decode("utf-8")
        labels = json.loads(labels_raw)
    except Exception as e:
        print(f"Error fetching labels: {e}", file=sys.stderr)
        labels = []

    with open(os.path.join(ISSUES_DIR, "labels.json"), "w", encoding="utf-8") as f:
        json.dump(labels, f, indent=2, ensure_ascii=False)

    print(f"Fetching issues from {UPSTREAM_REPO}...")
    fields = [
        "assignees", "author", "body", "closed", "closedAt",
        "comments", "createdAt", "id", "isPinned", "labels",
        "milestone", "number", "reactionGroups", "state",
        "stateReason", "title", "updatedAt", "url"
    ]
    try:
        issues_raw = subprocess.check_output([
            "gh", "issue", "list", "--repo", UPSTREAM_REPO, "--state", "all",
            "--limit", "100", "--json", ",".join(fields)
        ]).decode("utf-8")
        issues = json.loads(issues_raw)
    except Exception as e:
        print(f"Error fetching issues: {e}", file=sys.stderr)
        sys.exit(1)

    issues.sort(key=lambda x: x["number"])

    with open(os.path.join(ISSUES_DIR, "issues.json"), "w", encoding="utf-8") as f:
        json.dump(issues, f, indent=2, ensure_ascii=False)

    print(f"Exported {len(issues)} issues to JSON.")

    generated_files = []

    for issue in issues:
        num = issue["number"]
        title = issue["title"]
        slug = slugify(title)
        filename = f"issue-{num:03d}-{slug}.md"
        filepath = os.path.join(ISSUES_DIR, filename)

        author_login = issue.get("author", {}).get("login", "unknown") if issue.get("author") else "unknown"
        created_at = issue.get("createdAt", "")
        updated_at = issue.get("updatedAt", "")
        closed_at = issue.get("closedAt", "")
        state = issue.get("state", "OPEN")
        state_reason = issue.get("stateReason", "") or ""
        labels_list = [l["name"] for l in issue.get("labels", [])]
        assignees_list = [a["login"] for a in issue.get("assignees", [])]
        url = issue.get("url", f"https://github.com/{UPSTREAM_REPO}/issues/{num}")
        body = (issue.get("body") or "").strip()
        comments = issue.get("comments", [])

        # Build YAML frontmatter
        fm_lines = [
            "---",
            f"number: {num}",
            f"title: {json.dumps(title)}",
            f"state: {state}",
            f"author: {author_login}",
            f"created_at: {created_at}",
            f"updated_at: {updated_at}",
            f"closed_at: {json.dumps(closed_at) if closed_at else 'null'}",
            f"url: {url}",
            f"labels: {json.dumps(labels_list)}",
            f"assignees: {json.dumps(assignees_list)}",
            f"comments_count: {len(comments)}",
            "---",
            ""
        ]

        md_content = []
        md_content.extend(fm_lines)
        md_content.append(f"# Issue #{num}: {title}\n")

        status_badge = f"`{state}`" if state == "OPEN" else f"`CLOSED ({state_reason})`" if state_reason else "`CLOSED`"
        md_content.append(f"- **Status**: {status_badge}")
        md_content.append(f"- **Author**: [@{author_login}](https://github.com/{author_login})")
        md_content.append(f"- **Created**: `{created_at}`")
        if updated_at:
            md_content.append(f"- **Updated**: `{updated_at}`")
        if closed_at:
            md_content.append(f"- **Closed**: `{closed_at}`")
        if labels_list:
            labels_str = ", ".join(f"`{l}`" for l in labels_list)
            md_content.append(f"- **Labels**: {labels_str}")
        if assignees_list:
            assignees_str = ", ".join(f"@{a}" for a in assignees_list)
            md_content.append(f"- **Assignees**: {assignees_str}")
        md_content.append(f"- **Original Issue**: [#{num} on GitHub]({url})")
        md_content.append("\n---\n")

        md_content.append("## Description\n")
        if body:
            md_content.append(body)
        else:
            md_content.append("*No description provided.*")
        md_content.append("\n")

        md_content.append("\n---\n")
        md_content.append(f"## Comments ({len(comments)})\n")

        if comments:
            for idx, c in enumerate(comments, 1):
                c_author = c.get("author", {}).get("login", "unknown") if c.get("author") else "unknown"
                c_date = c.get("createdAt", "")
                c_assoc = c.get("authorAssociation", "")
                c_url = c.get("url", "")
                c_body = (c.get("body") or "").strip()

                header = f"### Comment #{idx} by [@{c_author}](https://github.com/{c_author}) • `{c_date}`"
                if c_assoc and c_assoc != "NONE":
                    header += f" ({c_assoc})"
                md_content.append(header)
                if c_url:
                    md_content.append(f"[Original Comment Link]({c_url})\n")
                else:
                    md_content.append("")
                md_content.append(c_body if c_body else "*No content.*")
                md_content.append("\n---\n")
        else:
            md_content.append("*No comments on this issue yet.*\n")

        with open(filepath, "w", encoding="utf-8") as f:
            f.write("\n".join(md_content))

        generated_files.append((num, title, state, labels_list, author_login, len(comments), filename, url))

    # Generate README.md index
    print("Generating README.md index...")
    readme_path = os.path.join(ISSUES_DIR, "README.md")
    
    open_count = sum(1 for i in issues if i["state"] == "OPEN")
    closed_count = sum(1 for i in issues if i["state"] == "CLOSED")

    readme = [
        "# Issues Archive: `awesome-downloader`",
        "",
        f"> Local copy of all issues and discussions imported from [{UPSTREAM_REPO}](https://github.com/{UPSTREAM_REPO}/issues).",
        "",
        "## Summary",
        "",
        f"- **Total Issues**: {len(issues)}",
        f"- **Open**: {open_count}",
        f"- **Closed**: {closed_count}",
        f"- **Labels Defined**: {len(labels)}",
        "- **JSON Archive**: [`issues.json`](./issues.json)",
        "- **Labels Archive**: [`labels.json`](./labels.json)",
        "",
        "---",
        "",
        "## Roadmap & Multi-Step Goals",
        "",
        "The upstream project organized larger platform additions and major features into multi-step goals:",
        "",
        "### 1. Linux Support Goal ([#6](./issue-006-goal-linux-support.md))",
        "- **Prerequisite**: [#14 Extract tested OS and architecture adapters](./issue-014-extract-tested-os-and-architecture-adapters-without-changing.md) (`status: ready`)",
        "- **Step 2**: [#22 Linux: desktop runtime and native messaging integration](./issue-022-linux-2-3-desktop-runtime-and-native-messaging-integration.md) (`status: blocked`)",
        "- **Step 3**: [#23 Linux: package media dependencies and validate install/upgrade/uninstall](./issue-023-linux-3-3-package-media-dependencies-and-validate-install-up.md) (`status: blocked`)",
        "",
        "### 2. macOS Support Goal ([#7](./issue-007-goal-macos-support.md))",
        "- **Prerequisite**: [#14 Extract tested OS and architecture adapters](./issue-014-extract-tested-os-and-architecture-adapters-without-changing.md) (`status: ready`)",
        "- **Step 2**: [#24 macOS: desktop runtime and native messaging integration](./issue-024-macos-2-3-desktop-runtime-and-native-messaging-integration.md) (`status: blocked`)",
        "- **Step 3**: [#25 macOS: package app and media tools, signing plan and clean-install validation](./issue-025-macos-3-3-package-app-and-media-tools-signing-plan-and-clean.md) (`status: blocked`)",
        "",
        "### 3. Windows ARM64 Support Goal ([#8](./issue-008-goal-windows-arm64-support.md))",
        "- **Prerequisite**: [#14 Extract tested OS and architecture adapters](./issue-014-extract-tested-os-and-architecture-adapters-without-changing.md) (`status: ready`)",
        "- **Step 2**: [#26 Windows ARM64: audit and implement runtime/native dependency compatibility](./issue-026-windows-arm64-2-3-audit-and-implement-runtime-native-depende.md) (`status: blocked`)",
        "- **Step 3**: [#27 Windows ARM64: installer, distinct artifacts and hardware validation](./issue-027-windows-arm64-3-3-installer-distinct-artifacts-and-hardware.md) (`status: blocked`)",
        "",
        "### 4. Firefox Support Goal ([#10](./issue-010-goal-firefox-support.md))",
        "- **Step 1**: [#28 Firefox: media capture and native messaging compatibility](./issue-028-firefox-1-2-media-capture-and-native-messaging-compatibility.md) (`status: ready`)",
        "- **Step 2**: [#29 Firefox: persistent extension distribution and guided setup](./issue-029-firefox-2-2-persistent-extension-distribution-and-guided-set.md) (`status: blocked`)",
        "",
        "### 5. YouTube Playlists Goal ([#21](./issue-021-goal-youtube-playlists-support-and-playlist-downloading.md))",
        "- **Step 1**: [#30 YouTube playlists: bounded discovery and entry/quality model](./issue-030-youtube-playlists-1-2-bounded-discovery-and-entry-quality-mo.md) (`status: ready`)",
        "- **Step 2**: [#31 YouTube playlists: preview selection, queue integration and batch controls](./issue-031-youtube-playlists-2-2-preview-selection-queue-integration-an.md) (`status: blocked`)",
        "",
        "---",
        "",
        "## Ready for Implementation (`status: ready`)",
        "",
        "- [#14 Extract tested OS and architecture adapters without changing Windows behavior](./issue-014-extract-tested-os-and-architecture-adapters-without-changing.md)",
        "- [#28 [Firefox 1/2] media capture and native messaging compatibility](./issue-028-firefox-1-2-media-capture-and-native-messaging-compatibility.md)",
        "- [#30 [YouTube playlists 1/2] bounded discovery and entry/quality model](./issue-030-youtube-playlists-1-2-bounded-discovery-and-entry-quality-mo.md)",
        "",
        "---",
        "",
        "## Complete Issue List",
        "",
        "| # | Status | Title | Labels | Comments | Author | GitHub |",
        "|---|---|---|---|---|---|---|"
    ]

    for num, title, state, labels_list, author_login, comments_count, filename, url in generated_files:
        status_str = f"**{state}**" if state == "OPEN" else f"*{state}*"
        clean_title = title.replace("|", "\\|")
        labels_str = " ".join(f"`{l}`" for l in labels_list) if labels_list else "-"
        comments_str = str(comments_count) if comments_count > 0 else "-"
        readme.append(
            f"| [{num}](./{filename}) | {status_str} | [{clean_title}](./{filename}) | {labels_str} | {comments_str} | [@{author_login}](https://github.com/{author_login}) | [#{num}]({url}) |"
        )

    readme.extend([
        "",
        "---",
        "",
        "## Pushing to GitHub in the Future",
        "",
        "When ready to push/publish these issues to our own GitHub repository, run:",
        "```bash",
        "# Dry run preview",
        "python3 scripts/import_issues_to_github.py --target-repo devenvoy/open-download-manager --dry-run",
        "",
        "# Apply and create labels and issues",
        "python3 scripts/import_issues_to_github.py --target-repo devenvoy/open-download-manager",
        "```",
        ""
    ])

    with open(readme_path, "w", encoding="utf-8") as f:
        f.write("\n".join(readme))

    print(f"Done! {len(issues)} markdown issue files created in {ISSUES_DIR}/ along with README.md index.")

if __name__ == "__main__":
    main()
