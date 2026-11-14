import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
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
  const status = browserStatus(browser);
  const action = primaryAction(browser);
  const storeAction = needsStoreFallback(browser);

  async function copyPath() {
    await onSetup("copy");
    setCopied(true);
    window.setTimeout(() => setCopied(false), 2_000);
  }

  return (
    <section className="space-y-4">
      <Button type="button" variant="ghost" size="sm" onClick={onBack}>
        ‹ All browsers
      </Button>

      <header className="flex items-center gap-3">
        <BrowserIcon browser={browser} className="size-11" />
        <div className="min-w-0">
          <h2 className="truncate text-base font-semibold">{browser.name}</h2>
          <p className="text-sm text-muted-foreground">{status.label}</p>
        </div>
      </header>

      <p className="text-sm text-muted-foreground">{browserSummary(browser)}</p>

      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          disabled={busy || !browser.installed}
          onClick={() => onSetup(action)}
        >
          {primaryActionLabel(browser, action)}
        </Button>
        {storeAction ? (
          <Button
            type="button"
            variant="outline"
            disabled={busy || !browser.installed}
            onClick={() => onSetup("store")}
          >
            Install from Web Store
          </Button>
        ) : null}
        {!browser.storeUrl ? (
          <Button
            type="button"
            variant="outline"
            disabled={busy || !browser.installed}
            onClick={() => onSetup("download")}
          >
            Download extension
          </Button>
        ) : null}
      </div>

      <ol className="list-decimal space-y-1 pl-5 text-sm text-muted-foreground">
        {setupSteps(browser).map((step) => (
          <li key={step}>{step}</li>
        ))}
      </ol>

      {!browser.storeUrl ? (
        <>
          <Separator />
          <div className="space-y-2">
            <Label htmlFor="extension-path">Extension folder</Label>
            <div className="flex gap-2">
              <Input
                id="extension-path"
                readOnly
                value={browser.extensionPath}
                aria-label="Extension folder"
              />
              <Button type="button" variant="outline" onClick={() => void copyPath()}>
                {copied ? "Copied" : "Copy path"}
              </Button>
              <Button
                type="button"
                variant="outline"
                disabled={busy}
                onClick={() => onSetup("folder")}
              >
                Show folder
              </Button>
            </div>
          </div>

          <details className="rounded-md border border-border p-3 text-sm">
            <summary className="cursor-pointer font-medium">
              Why isn&apos;t setup completely automatic?
            </summary>
            <p className="mt-2 text-muted-foreground">
              {automaticSetupCopy(browser)}
            </p>
          </details>
        </>
      ) : null}
    </section>
  );
}
