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

  if (isPending) {
    return (
      <dialog id="settings-dialog" className="settings-dialog w-full h-screen border-0 m-0 max-w-none max-h-none flex flex-col p-0 static" open>
        <div className="dialog-body" style={{ padding: 20 }}>
          Loading options…
        </div>
      </dialog>
    );
  }

  if (isError) {
    return (
      <dialog id="settings-dialog" className="settings-dialog w-full h-screen border-0 m-0 max-w-none max-h-none flex flex-col p-0 static" open>
        <div className="dialog-body" style={{ padding: 20 }}>
          Could not load settings: {error instanceof Error ? error.message : "Error"}
        </div>
      </dialog>
    );
  }

  return (
    <dialog id="settings-dialog" className="settings-dialog w-full h-screen border-0 m-0 max-w-none max-h-none flex flex-col p-0 static" open>
      <SettingsPanel
        settings={data.settings}
        saving={updateSettings.isPending}
        onSave={async (patch) => {
          await updateSettings.mutateAsync(patch);
        }}
        onClose={() => void handleClose()}
      />
    </dialog>
  );
}
