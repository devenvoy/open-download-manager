import { useCallback, useState } from "react";
import { useTimeout } from "@/lib/hooks/use-timeout";
import { type BrowserStatus } from "@/types/ipc";
import {
  automaticSetupCopy,
  browserStatus,
  browserSummary,
  needsStoreFallback,
  primaryAction,
  primaryActionLabel,
  setupSteps,
} from "../model/browser-view";
import { BrowserIcon } from "./BrowserGrid";

export interface BrowserDetailProps {
  browser: BrowserStatus;
  busy: boolean;
  onBack: () => void;
  onSetup: (action: string) => void;
}

export function BrowserDetail({
  browser,
  busy,
  onBack,
  onSetup,
}: BrowserDetailProps) {
  const [copied, setCopied] = useState(false);
  const resetCopied = useCallback(() => setCopied(false), []);
  useTimeout(resetCopied, copied ? 2_000 : null);

  const status = browserStatus(browser);
  const action = primaryAction(browser);
  const storeAction = needsStoreFallback(browser);

  async function copyPath() {
    await onSetup("copy");
    setCopied(true);
  }

  return (
    <section className="browser-detail" id="browser-detail">
      <button
        className="browser-back"
        id="browser-back"
        type="button"
        onClick={onBack}
      >
        ‹ All browsers
      </button>

      <div className="browser-detail-heading">
        <BrowserIcon browser={browser} />
        <div>
          <h2 id="browser-detail-name">{browser.name}</h2>
          <p
            id="browser-detail-status"
            className={`browser-detail-status ${status.tone}`}
          >
            {status.label}
          </p>
        </div>
      </div>

      <p id="browser-detail-summary">{browserSummary(browser)}</p>

      <div className="browser-actions">
        <button
          className="button primary"
          id="browser-primary-action"
          type="button"
          disabled={busy || !browser.installed}
          onClick={() => onSetup(action)}
        >
          {primaryActionLabel(browser, action)}
        </button>
        {storeAction && (
          <button
            className="button"
            id="browser-store-action"
            type="button"
            disabled={busy || !browser.installed}
            onClick={() => onSetup("store")}
          >
            Install from Web Store
          </button>
        )}
        {!browser.storeUrl && (
          <button
            className="button"
            id="browser-download-action"
            type="button"
            disabled={busy || !browser.installed}
            onClick={() => onSetup("download")}
          >
            Download extension
          </button>
        )}
      </div>

      <ol className="browser-steps" id="browser-steps">
        {setupSteps(browser).map((step) => (
          <li key={step}>{step}</li>
        ))}
      </ol>

      {!browser.storeUrl && (
        <>
          <label className="extension-path-label" htmlFor="extension-path">
            Extension folder
          </label>
          <div className="extension-path-row">
            <input
              id="extension-path"
              readOnly
              aria-label="Extension folder"
              value={browser.extensionPath ?? ""}
            />
            <button
              className="button"
              id="copy-extension-path"
              type="button"
              onClick={() => void copyPath()}
            >
              {copied ? "Copied" : "Copy path"}
            </button>
            <button
              className="button"
              id="show-extension-folder"
              type="button"
              onClick={() => onSetup("reveal")}
            >
              Show folder
            </button>
          </div>
        </>
      )}

      <details className="automatic-setup-note">
        <summary>Why isn't setup completely automatic?</summary>
        <p id="automatic-setup-copy">{automaticSetupCopy(browser)}</p>
      </details>
    </section>
  );
}
