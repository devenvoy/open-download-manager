import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useSettings, useUpdateSettings } from "../hooks/use-settings";
import { SettingsPanel } from "./SettingsPanel";

export function SettingsDialog() {
  const [open, setOpen] = useState(false);
  const { data, isPending } = useSettings();
  const updateSettings = useUpdateSettings();

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline">Options</Button>
      </DialogTrigger>
      <DialogContent className="max-w-2xl max-h-[90vh] flex flex-col p-0">
        <DialogHeader className="p-6 pb-2">
          <DialogTitle id="settings-title">Options</DialogTitle>
          <DialogDescription>Configure application preferences.</DialogDescription>
        </DialogHeader>
        <ScrollArea className="flex-1 px-6">
          <div className="pb-6">
            {isPending || !data ? (
              <p className="text-sm text-muted-foreground py-8 text-center">
                Loading settings…
              </p>
            ) : (
              <SettingsPanel
                settings={data.settings}
                saving={updateSettings.isPending}
                onSave={async (patch) => {
                  await updateSettings.mutateAsync(patch);
                }}
                onClose={() => setOpen(false)}
              />
            )}
          </div>
        </ScrollArea>
      </DialogContent>
    </Dialog>
  );
}
