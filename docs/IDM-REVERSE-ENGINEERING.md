# Internet Download Manager: Static Reverse-Engineering Notes

## Scope and method

This report examines the IDM installer `idman643build18.exe` and its embedded
`IDMan.exe` without executing either binary. REA 6.8.0 was used for static
inspection; its Ghidra provider did not accept the installed Ghidra 12.2, so the
main executable was also inspected directly in Ghidra 12.2. The analysis
included PE metadata, imports, strings and references, and decompiler output
for selected routines.

The analyzed main executable is a 32-bit x86 Windows GUI program. Its SHA-256
is `a705c0154fafc7cd509decf165cb6f6c050b6521b40adb6f730dd827c86051c7`.
The installer SHA-256 is
`7346128fab42957b4254eabdb012bfdbd751e9069361e98a6916f26bb03d62ee`.

This is static evidence, not an observation of live traffic or runtime
behavior. Ghidra's `FUN_...` names are generated labels, and decompiler output
is an approximation of the original program.

## Findings

### 1. IDM uses HTTP byte ranges and parallel file chunks

**Evidence:** The binary contains HTTP connection routines, including strings
for `HTTPConnection::SendHTTPSGet()`,
`HTTPConnection::RecvHTTPSData()`, and
`HTTPConnection::GetRetrRangeReply()`. HTTP header strings include `Range:`,
`If-Range:`, and `Content-Length:`. A recovered routine at `0x005db150`
recognizes `Range:` and parses a `bytes=0-...` value.

**Conclusion (high confidence):** IDM has explicit HTTP range-request handling;
the strings and code are consistent with dividing a file into byte ranges and
downloading those ranges independently. This is not merely a UI-level
connection-count setting.

**Limit:** This pass did not establish IDM's exact chunk-sizing policy, maximum
connection count, or how it chooses the initial count for every server.

### 2. Chunks can be reassigned when a connection changes state

**Evidence:** Strings include “The file chunk %d is reassigned to connection
%d.” and “The file chunk of connection %d is reassigned to connection %d.”
Ghidra recovered code at `0x005b1c10` that records reassignment, updates
download counters, stops the prior connection object, and resets associated
state.

**Conclusion (high confidence):** IDM's chunk manager can transfer
responsibility for a chunk between connection workers. Its workers are not
simply fixed, independent bars.

**Limit:** The recovered function alone does not establish every trigger for
reassignment or imply that IDM dynamically splits every slow chunk.

### 3. It detects inconsistent parallel data and can fall back

**Evidence:** User-facing strings explain that IDM may restart from the
beginning after newly received data does not match data already downloaded,
and may restart using one connection when data from several connections is
corrupt or inconsistent.

**Conclusion (high confidence):** Parallelism is guarded by integrity and
consistency handling. IDM is prepared to sacrifice parallel speed for a
single-stream restart when it cannot trust the assembled data.

**Limit:** These strings establish supported recovery behavior, not its
frequency or every condition that triggers it.

### 4. Downloading can begin before the “Download File Info” dialog closes

**Evidence:** A message says IDM “always starts downloading while displaying
‘Download File Info’ dialog,” and warns that choosing “Download Later” may
leave part of the file already downloaded.

**Conclusion (high confidence):** At least one IDM workflow starts transfer
work before the user dismisses that dialog. This supports an eager,
background-first UX rather than waiting for every detail of the dialog flow
to finish.

**Limit:** The message does not establish that all file types, sites, or
download paths behave this way.

### 5. File size may legitimately remain unknown

**Evidence:** IDM includes messages warning that a file may be incomplete
because its size is unknown, and a completion message that reports bytes
downloaded while preserving that warning.

**Conclusion (high confidence):** IDM does not treat an unknown size as a
known zero or universally infer a total. It can complete a transfer while
disclosing the uncertainty.

### 6. The installer and the downloader are separate concerns

**Evidence:** The setup package has a separate extraction and relaunch flow.
Its manifest includes `IDMan.exe`, network-monitoring DLLs, message-host and
browser-integration components, among other items.

**Conclusion:** Installer extraction, browser integration, and the download
engine should not be conflated. Component roles inferred from filenames are
not proof of their runtime responsibilities.

## Practical interpretation

The strongest transferable ideas are:

1. Start useful work early and keep discovery/metadata work off the UI's
   critical path.
2. Use real range requests only when the server demonstrates support and the
   returned byte ranges refer to the same entity.
3. Coordinate chunks as workers, with reassignment/retry/recovery rather than
   assuming every connection progresses equally.
4. Fall back safely when range responses or assembled data are inconsistent.
5. Represent unknown size honestly; do not make a percentage look authoritative
   when no trustworthy total exists.

These are architectural lessons, not a basis for copying IDM code or claiming
the current app must be replaced wholesale.

## Evidence limits

- Neither Windows executable was run; there was no dynamic analysis, packet
  capture, or observation of a live IDM download.
- Strings can be unused, and recovered control flow can be incomplete.
- The analysis does not prove a specific TLS/socket implementation. Imports
  include WinINet URL and cookie helpers, but that alone does not establish
  which transport implementation serves each request.
- No claim is made about IDM's current release behavior beyond evidence in the
  analyzed binary.
