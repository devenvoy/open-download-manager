import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { ipc } from "@/lib/ipc";
import { snapshotQueryOptions } from "@/lib/ipc/queries";
import { useHydrateTheme } from "@/stores/theme-store";
import {
  connectionSummary,
  visibleBrowsers,
} from "../model/browser-view";
import {
  useAdvancedIntegration,
  useAdvancedIntegrationActions,
  useBrowserSetup,
  useIntegrationInfo,
  useUnregisterBrowser,
} from "../api";
import { AdvancedIntegrationSection } from "../components/AdvancedIntegrationSection";
import { BrowserDetail } from "../components/BrowserDetail";
import { BrowserGrid } from "../components/BrowserGrid";

/** The `?integration=1` window. */
export function IntegrationScreen() {
  const info = useIntegrationInfo();
  const advanced = useAdvancedIntegration();
  const setup = useBrowserSetup();
  const unregister = useUnregisterBrowser();
  const advancedActions = useAdvancedIntegrationActions();
  const [selected, setSelected] = useState<string | null>(null);

  const snapshot = useQuery({
    ...snapshotQueryOptions,
    refetchInterval: false,
    staleTime: Infinity,
  });
  useHydrateTheme(snapshot.data?.settings.theme);

  const browsers = visibleBrowsers(info.data);
  const current = browsers.find((browser) => browser.name === selected) ?? null;

  function runSetup(action: string) {
    if (!current) return;
    setup.mutate(
      { action, browser: current.name },
      {
        onSuccess: () => {
          if (action === "copy") toast.success("Extension folder path copied.");
          if (action === "download") {
            toast.success("Extension saved in your Downloads folder.");
          }
        },
      },
    );
  }

  function runUnregister() {
    if (!current) return;
    unregister.mutate(
      { browser: current.name },
      {
        onSuccess: () => {
          toast.success(`${current.name} integration removed.`);
        },
        onError: (err) => {
          toast.error(
            `Failed to remove integration: ${err instanceof Error ? err.message : String(err)}`
          );
        },
      },
    );
  }

  function close() {
    void ipc.closeAuxiliaryWindow();
  }

  return (
    <dialog
      id="integration-dialog"
      className="integration-dialog w-full h-screen border-0 m-0 max-w-none max-h-none flex flex-col p-0 static"
      open
    >
      <div className="dialog-title">
        <strong id="integration-title">Browser integration</strong>
        <button
          type="button"
          className="close-button"
          onClick={close}
          aria-label="Close"
        >
          ×
        </button>
      </div>

      <div className="dialog-body browser-integration-body">
        {info.isPending ? (
          <div style={{ padding: 16 }}>Loading browsers…</div>
        ) : info.isError ? (
          <div style={{ padding: 16 }}>
            Could not read the browser integration:{" "}
            {info.error instanceof Error ? info.error.message : "Error"}
          </div>
        ) : current ? (
          <BrowserDetail
            browser={current}
            busy={setup.isPending || unregister.isPending}
            onBack={() => setSelected(null)}
            onSetup={runSetup}
            onUnregister={runUnregister}
          />
        ) : (
          <>
            <div className="integration-heading">
              <div>
                <h2>Choose your browser</h2>
                <p>
                  Open Download Manager detects the extension separately in
                  every installed browser profile.
                </p>
              </div>
              <button
                className="button"
                id="browser-refresh"
                type="button"
                disabled={info.isFetching}
                onClick={() => void info.refetch()}
              >
                Refresh
              </button>
            </div>
            <BrowserGrid browsers={browsers} onSelect={setSelected} />
          </>
        )}

        <AdvancedIntegrationSection
          capability={advanced.data}
          enabling={advancedActions.enable.isPending}
          disabling={advancedActions.disable.isPending}
          repairing={advancedActions.repair.isPending}
          onEnable={() => advancedActions.enable.mutate()}
          onDisable={() => advancedActions.disable.mutate()}
          onRepair={() => advancedActions.repair.mutate()}
        />
      </div>

      <div className="dialog-footer">
        <span
          id="browser-connection-summary"
          role="status"
        >
          {connectionSummary(info.data)}
        </span>
        <button
          type="button"
          className="button"
          data-close
          onClick={close}
        >
          Close
        </button>
      </div>
    </dialog>
  );
}
