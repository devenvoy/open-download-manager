import { EmptyState } from "@/components/feedback/EmptyState";
import { settingsApi } from "../api";
import { SettingsPanel } from "../components/SettingsPanel";
import { useSettings, useUpdateSettings } from "../hooks/use-settings";

/** The `?options=1` window. */
export function SettingsScreen() {
  const { data, isPending, isError, error } = useSettings();
  const updateSettings = useUpdateSettings();

  async function handleClose() {
    try {
      await settingsApi.close();
    } catch {
      // Window close in non-desktop environments can be safely ignored
    }
  }

  return (
    <div className="min-h-screen bg-background p-6 text-foreground">
      <div className="mx-auto w-full max-w-2xl space-y-6">
        <div className="flex items-center justify-between border-b pb-4">
          <h1 className="text-xl font-semibold" id="settings-title">
            Options
          </h1>
        </div>
        {isPending ? (
          <p className="text-sm text-muted-foreground py-8 text-center">
            Loading settings…
          </p>
        ) : isError ? (
          <EmptyState
            title="Could not load settings"
            description={error instanceof Error ? error.message : undefined}
          />
        ) : (
          <SettingsPanel
            settings={data.settings}
            saving={updateSettings.isPending}
            onSave={async (patch) => {
              await updateSettings.mutateAsync(patch);
              await handleClose();
            }}
            onClose={() => void handleClose()}
          />
        )}
      </div>
    </div>
  );
}
