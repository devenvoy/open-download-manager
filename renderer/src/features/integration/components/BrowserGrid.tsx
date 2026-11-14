import { type BrowserStatus } from "@/types/ipc";
import { browserStatus, browserSubtitle } from "../model/browser-view";

export function BrowserIcon({
  browser,
}: {
  browser: BrowserStatus;
}) {
  return (
    <span className="browser-icon-frame" aria-hidden="true">
      {browser.icon ? (
        <img src={browser.icon} alt="" />
      ) : (
        <span className="browser-monogram">{browser.name.slice(0, 1)}</span>
      )}
    </span>
  );
}

export function BrowserGrid({
  browsers,
  onSelect,
}: {
  browsers: BrowserStatus[];
  onSelect: (name: string) => void;
}) {
  if (browsers.length === 0) {
    return (
      <p className="field-help" style={{ padding: 16 }}>
        No supported browser was found on this computer. Install Chrome, Edge,
        Brave, Firefox or another supported browser, then refresh.
      </p>
    );
  }

  return (
    <div
      className="browser-grid"
      id="browser-grid"
      role="list"
      aria-label="Installed supported browsers"
    >
      {browsers.map((browser) => {
        const status = browserStatus(browser);
        return (
          <button
            key={browser.name}
            type="button"
            className="browser-card"
            onClick={() => onSelect(browser.name)}
          >
            <BrowserIcon browser={browser} />
            <span className="browser-card-copy">
              <strong>{browser.name}</strong>
              <span className="browser-card-subtitle">
                {browserSubtitle(browser)}
              </span>
            </span>
            <span className={`browser-status ${status.tone}`}>
              {status.label}
            </span>
          </button>
        );
      })}
    </div>
  );
}
