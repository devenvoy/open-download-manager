import { Button } from "@/components/ui/button";
import { type BrowserStatus } from "@/types/ipc";
import { browserStatus, browserSubtitle } from "../model/browser-view";

export function BrowserIcon({
  browser,
  className,
}: {
  browser: BrowserStatus;
  className?: string;
}) {
  return (
    <span
      className={`flex size-9 shrink-0 items-center justify-center overflow-hidden rounded-md border border-border bg-muted ${className ?? ""}`}
      aria-hidden
    >
      {browser.icon ? (
        <img src={browser.icon} alt="" className="size-6" />
      ) : (
        <span className="text-sm font-semibold">{browser.name.slice(0, 1)}</span>
      )}
    </span>
  );
}

const TONE_CLASS: Record<string, string> = {
  connected: "text-emerald-600 dark:text-emerald-400",
  setup: "text-amber-600 dark:text-amber-400",
  missing: "text-muted-foreground",
};

export function BrowserGrid({
  browsers,
  onSelect,
}: {
  browsers: BrowserStatus[];
  onSelect: (name: string) => void;
}) {
  if (browsers.length === 0) {
    return (
      <p className="rounded-md border border-dashed border-border p-4 text-sm text-muted-foreground">
        No supported browser was found on this computer. Install Chrome, Edge,
        Brave, Firefox or another supported browser, then refresh.
      </p>
    );
  }

  return (
    <ul className="grid gap-2 sm:grid-cols-2" aria-label="Installed supported browsers">
      {browsers.map((browser) => {
        const status = browserStatus(browser);
        return (
          <li key={browser.name}>
            <Button
              type="button"
              variant="outline"
              className="h-auto w-full justify-start gap-3 px-3 py-2 text-left"
              onClick={() => onSelect(browser.name)}
            >
              <BrowserIcon browser={browser} />
              <span className="min-w-0 flex-1">
                <span className="block truncate font-medium">{browser.name}</span>
                <span className="block truncate text-xs font-normal text-muted-foreground">
                  {browserSubtitle(browser)}
                </span>
              </span>
              <span className={`shrink-0 text-xs ${TONE_CLASS[status.tone]}`}>
                {status.label}
              </span>
            </Button>
          </li>
        );
      })}
    </ul>
  );
}
