# Download behavior notes

These notes summarize observed download-manager behavior relevant to the
feature comparison. They describe functional evidence and implementation
questions, not source-code reuse or a claim of runtime equivalence.

## HTTP ranges and segmented transfers

HTTP request and response markers include `Range`, `If-Range`,
`Content-Range`, and range-reply handling. Diagnostic messages also describe
reassigning a file chunk from one connection to another and restarting with
one connection after inconsistent parallel data.

These observations support a feature-level conclusion: segmented transfers,
chunk reassignment, and a recovery path are represented. They do not establish
the exact chunk-sizing policy, maximum connection count, trigger conditions,
or whether every recovery path is reachable.

For this application, only use parallel ranges when the server confirms range
support and the response identifies the same entity. Persist validator kind as
well as value, reject mismatched ranges, clean up invalid partial data, and
fall back to a single stream without appending incompatible bytes.

## Download-dialog timing and unknown size

Messages describe an option to begin transfer work while the “Download File
Info” dialog is visible, including the possibility that some bytes have been
received before choosing “Download Later”. This indicates a workflow option;
it does not establish a default or behavior for every download.

Other messages warn that a file may be incomplete when its size is unknown,
and report bytes downloaded without presenting an invented total. Keep unknown
size distinct from zero and avoid authoritative percentages without a known
total.

## Protocol-specific paths

FTP behavior has separate connection, authentication, directory, metadata,
passive/active data-channel, restart, retrieval, and reply-processing stages.
It should be designed as a dedicated executor rather than folded into an HTTP
client. Resume must depend on server confirmation and matching remote-file
metadata.

Media handling recognizes video MIME types and distinct YouTube/Google-video,
HLS, and MPD/DASH routing paths. Recognition does not prove complete coverage
of modern sites or manifest variants. Unsupported encrypted, live, byte-range,
or nested-manifest cases should be reported before a transfer is queued.

## Evidence limits

The observations above establish feature presence at a functional level.
Static messages and identifiers alone do not prove defaults, complete runtime
behavior, performance, or compatibility across sites and operating systems.
Validate each proposed implementation with focused fixtures and end-to-end
tests before marking a parity item complete.
