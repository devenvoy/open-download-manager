import { useState, useEffect, useCallback } from "react";
import { CheckCircle2, XCircle, Copy, Check, Hash, Loader2 } from "lucide-react";
import { ClassicDialog } from "@/components/feedback";
import { ipc } from "@/lib/ipc";
import { type ChecksumVerificationReport } from "@/types/ipc";

export interface ChecksumDialogProps {
  open: boolean;
  filePath: string;
  fileName: string;
  onClose: () => void;
}

export function ChecksumDialog({
  open,
  filePath,
  fileName,
  onClose,
}: ChecksumDialogProps) {
  const [expected, setExpected] = useState("");
  const [loading, setLoading] = useState(false);
  const [report, setReport] = useState<ChecksumVerificationReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // Auto-detect algorithm based on input length or prefix
  const detectedAlgo = (() => {
    const trimmed = expected.trim().toLowerCase();
    if (!trimmed) return null;
    if (trimmed.startsWith("sha256:") || trimmed.startsWith("sha-256=")) return "SHA-256";
    if (trimmed.startsWith("sha512:") || trimmed.startsWith("sha-512=")) return "SHA-512";
    if (trimmed.startsWith("crc32:") || trimmed.startsWith("crc32=")) return "CRC32";

    const cleanHex = (trimmed.split(/\s+/)[0] ?? "").replace(/[^0-9a-f]/gi, "");
    if (cleanHex.length === 64) return "SHA-256";
    if (cleanHex.length === 128) return "SHA-512";
    if (cleanHex.length === 8) return "CRC32";
    return null;
  })();

  // On open, try reading clipboard if it contains a hash
  useEffect(() => {
    if (!open) {
      setExpected("");
      setReport(null);
      setError(null);
      setCopied(false);
      return;
    }

    async function tryReadClipboard() {
      try {
        const text = await ipc.readClipboard();
        if (text) {
          const trimmed = text.trim();
          const cleanHex = (trimmed.split(/\s+/)[0] ?? "").replace(/[^0-9a-f]/gi, "");
          if (cleanHex.length === 64 || cleanHex.length === 128 || cleanHex.length === 8) {
            setExpected(trimmed);
          }
        }
      } catch {
        // Clipboard access optional
      }
    }

    void tryReadClipboard();
  }, [open]);

  const handleVerify = useCallback(async () => {
    if (!expected.trim() || !filePath) return;
    setLoading(true);
    setError(null);
    setReport(null);

    try {
      const res = await ipc.verifyChecksum(filePath, expected.trim());
      setReport(res);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || "Failed to calculate checksum.");
    } finally {
      setLoading(false);
    }
  }, [expected, filePath]);

  const handleCopyActual = useCallback(async (actual: string) => {
    try {
      await navigator.clipboard.writeText(actual);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback
    }
  }, []);

  return (
    <ClassicDialog
      id="checksum-dialog"
      className="max-w-[560px] w-full"
      open={open}
      onClose={onClose}
      title={
        <div className="flex items-center gap-2">
          <Hash size={16} className="text-blue-500" />
          <span>Verify File Checksum</span>
        </div>
      }
    >
      <div className="p-4 space-y-4 text-xs font-sans">
        <div>
          <label className="block text-neutral-500 dark:text-neutral-400 mb-1 font-medium">
            File
          </label>
          <div
            className="p-2 bg-neutral-100 dark:bg-neutral-800 rounded border border-neutral-200 dark:border-neutral-700 font-mono text-[11px] truncate text-neutral-800 dark:text-neutral-200"
            title={filePath}
          >
            {fileName || filePath}
          </div>
        </div>

        <div>
          <div className="flex items-center justify-between mb-1">
            <label
              htmlFor="checksum-input"
              className="block text-neutral-700 dark:text-neutral-300 font-medium"
            >
              Expected Checksum
            </label>
            {detectedAlgo && (
              <span className="text-[10px] px-1.5 py-0.5 font-mono font-semibold rounded bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300">
                {detectedAlgo}
              </span>
            )}
          </div>
          <input
            id="checksum-input"
            type="text"
            className="w-full px-2.5 py-1.5 rounded border border-neutral-300 dark:border-neutral-700 bg-white dark:bg-neutral-900 font-mono text-xs focus:outline-none focus:ring-1 focus:ring-blue-500"
            placeholder="Paste SHA-256 (64 hex), SHA-512 (128 hex), or CRC32 (8 hex)..."
            value={expected}
            onChange={(e) => {
              setExpected(e.target.value);
              setReport(null);
              setError(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !loading && expected.trim()) {
                void handleVerify();
              }
            }}
            autoFocus
          />
          <p className="text-[11px] text-neutral-500 dark:text-neutral-400 mt-1">
            Supports raw hex or prefix format (e.g. <code className="text-neutral-700 dark:text-neutral-300">sha256:...</code>).
          </p>
        </div>

        {error && (
          <div className="p-2.5 rounded bg-red-50 dark:bg-red-950/40 border border-red-200 dark:border-red-800/60 text-red-700 dark:text-red-300 flex items-start gap-2">
            <XCircle size={15} className="shrink-0 mt-0.5 text-red-600 dark:text-red-400" />
            <span className="text-xs">{error}</span>
          </div>
        )}

        {report && (
          <div
            className={`p-3 rounded border space-y-2 ${
              report.matched
                ? "bg-emerald-50 dark:bg-emerald-950/30 border-emerald-200 dark:border-emerald-800/60 text-emerald-900 dark:text-emerald-100"
                : "bg-amber-50 dark:bg-amber-950/30 border-amber-200 dark:border-amber-800/60 text-amber-900 dark:text-amber-100"
            }`}
          >
            <div className="flex items-center gap-2 font-semibold">
              {report.matched ? (
                <>
                  <CheckCircle2 size={18} className="text-emerald-600 dark:text-emerald-400" />
                  <span>Checksum Match: File is authentic and intact ({report.algorithm})</span>
                </>
              ) : (
                <>
                  <XCircle size={18} className="text-amber-600 dark:text-amber-400" />
                  <span>Checksum Mismatch: File contents do not match expected digest</span>
                </>
              )}
            </div>

            <div className="space-y-1.5 pt-1 text-[11px] font-mono">
              <div>
                <span className="text-neutral-500 dark:text-neutral-400 block text-[10px] font-sans font-medium">
                  Expected
                </span>
                <span className="break-all">{report.expectedHex}</span>
              </div>
              <div>
                <div className="flex items-center justify-between">
                  <span className="text-neutral-500 dark:text-neutral-400 text-[10px] font-sans font-medium">
                    Actual Calculated
                  </span>
                  <button
                    type="button"
                    onClick={() => void handleCopyActual(report.actualHex)}
                    className="flex items-center gap-1 text-[10px] font-sans text-blue-600 dark:text-blue-400 hover:underline"
                  >
                    {copied ? (
                      <>
                        <Check size={11} /> Copied
                      </>
                    ) : (
                      <>
                        <Copy size={11} /> Copy actual
                      </>
                    )}
                  </button>
                </div>
                <span className="break-all">{report.actualHex}</span>
              </div>
            </div>
          </div>
        )}

        <div className="flex items-center justify-end gap-2 pt-2 border-t border-neutral-200 dark:border-neutral-800">
          <button
            type="button"
            className="px-3 py-1.5 rounded border border-neutral-300 dark:border-neutral-700 hover:bg-neutral-100 dark:hover:bg-neutral-800 text-neutral-700 dark:text-neutral-300 transition-colors"
            onClick={onClose}
          >
            Close
          </button>
          <button
            type="button"
            className="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-700 text-white font-medium flex items-center gap-1.5 transition-colors disabled:opacity-50"
            disabled={loading || !expected.trim()}
            onClick={() => void handleVerify()}
          >
            {loading ? (
              <>
                <Loader2 size={13} className="animate-spin" />
                <span>Hashing file...</span>
              </>
            ) : (
              <span>Verify</span>
            )}
          </button>
        </div>
      </div>
    </ClassicDialog>
  );
}
