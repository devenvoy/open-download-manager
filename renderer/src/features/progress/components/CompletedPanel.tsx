import { useState } from "react";
import { formatBytes } from "@/lib/format";
import { type DownloadSummary, type Settings, type SettingsPatch } from "@/types/ipc";

export interface CompletedPanelProps {
  download: DownloadSummary;
  settings: Settings;
  onOpen: () => void;
  onReveal: () => void;
  onClose: () => void;
  onUpdateSettings: (patch: SettingsPatch) => Promise<void>;
}

/**
 * The completion panel shown inside `#details-dialog` when a download
 * completes (`.is-complete > .completed-panel`).
 * Matches the legacy desktop dialog 1:1.
 */
export function CompletedPanel({
  download,
  settings,
  onOpen,
  onReveal,
  onClose,
  onUpdateSettings,
}: CompletedPanelProps) {
  const [hideDialog, setHideDialog] = useState(
    Boolean(settings.hideCompletionDialog),
  );

  const downloadedBytes = Number(download.total || download.downloaded || 0);
  const formattedDownloaded = formatBytes(downloadedBytes);
  const savePath = download.path || download.filename || "";

  async function handleSuppressChange(checked: boolean) {
    setHideDialog(checked);
    try {
      await onUpdateSettings({ hideCompletionDialog: checked });
    } catch {
      setHideDialog(!checked);
    }
  }

  return (
    <section className="completed-panel block p-3.5 bg-[#f0f0f0] dark:bg-[#1e1e1e] text-[#111] dark:text-[#e0e0e0] flex flex-col gap-3">
      <div className="completed-summary flex items-center gap-3 p-2.5 rounded-sm bg-white dark:bg-[#252526] border border-[#ddd] dark:border-[#383838]">
        <span className="completed-icon w-6 h-6 rounded-full bg-[#16a34a] text-white flex items-center justify-center font-bold text-sm shrink-0" aria-hidden="true">
          ✓
        </span>
        <div className="completed-summary-text text-[11px] leading-[1.4] whitespace-pre-line text-[#222] dark:text-[#ddd]">
          {`Download complete\nDownloaded ${formattedDownloaded} (${Math.round(downloadedBytes)} Bytes)`}
        </div>
      </div>

      <label className="completed-field flex flex-col gap-1 text-[11px] font-medium text-[#444] dark:text-[#aaa]">
        Address
        <input
          type="text"
          readOnly
          aria-label="Address"
          value={download.url || ""}
          className="h-[22px] px-1.5 border border-[#ccc] dark:border-[#444] rounded-xs bg-[#f8f8f8] dark:bg-[#2a2a2a] text-[#111] dark:text-[#e0e0e0] text-[11px]"
        />
      </label>

      <label className="completed-field flex flex-col gap-1 text-[11px] font-medium text-[#444] dark:text-[#aaa]">
        The file saved as
        <input
          type="text"
          readOnly
          aria-label="The file saved as"
          value={savePath}
          className="h-[22px] px-1.5 border border-[#ccc] dark:border-[#444] rounded-xs bg-[#f8f8f8] dark:bg-[#2a2a2a] text-[#111] dark:text-[#e0e0e0] text-[11px]"
        />
      </label>

      <div className="transfer-buttons flex justify-end gap-2 pt-1">
        <button
          type="button"
          className="button"
          id="completed-open"
          onClick={onOpen}
        >
          Open
        </button>
        <button
          type="button"
          className="button"
          id="completed-reveal"
          onClick={onReveal}
        >
          Open folder
        </button>
        <button
          type="button"
          className="button"
          id="completed-close"
          data-close
          onClick={onClose}
        >
          Close
        </button>
      </div>

      <label className="checkbox-label flex items-center gap-2 text-[11px] text-[#444] dark:text-[#aaa] cursor-pointer pt-1" htmlFor="suppress-completion">
        <input
          id="suppress-completion"
          type="checkbox"
          checked={hideDialog}
          onChange={(e) => void handleSuppressChange(e.target.checked)}
          className="m-0"
        />
        {" Don't show this dialog again"}
      </label>
    </section>
  );
}
