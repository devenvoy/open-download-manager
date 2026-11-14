import { useState, type FormEvent } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { DOWNLOAD_CATEGORIES, type DownloadCategory } from "@/types/ipc";
import { downloadsApi } from "../api";
import { useAddDownload } from "../hooks/use-downloads";

const AUTOMATIC = "automatic";

export function AddDownloadDialog() {
  const [open, setOpen] = useState(false);
  const [url, setUrl] = useState("");
  const [directory, setDirectory] = useState("");
  const [category, setCategory] = useState<string>(AUTOMATIC);
  const addDownload = useAddDownload();

  function reset() {
    setUrl("");
    setDirectory("");
    setCategory(AUTOMATIC);
  }

  async function handlePaste() {
    try {
      setUrl(await downloadsApi.readClipboard());
    } catch {
      toast.error("Could not read the clipboard.");
    }
  }

  async function handleChooseDirectory() {
    try {
      const selected = await downloadsApi.chooseDirectory();
      if (selected) setDirectory(selected);
    } catch {
      toast.error("Could not open the folder picker.");
    }
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const trimmed = url.trim();
    if (!trimmed) return;
    try {
      await addDownload.mutateAsync({
        urls: [trimmed],
        directory: directory || undefined,
        category:
          category === AUTOMATIC
            ? undefined
            : (category as DownloadCategory),
      });
      toast.success("Download added.");
      reset();
      setOpen(false);
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Could not add the download.",
      );
    }
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>Add URL</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add download</DialogTitle>
          <DialogDescription>
            Enter a URL and choose where to save it.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4 p-4">
          <div className="space-y-2">
            <Label htmlFor="add-url">URL</Label>
            <Input
              id="add-url"
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              placeholder="https://example.com/file.zip"
              autoFocus
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="add-category">Category</Label>
            <Select value={category} onValueChange={setCategory}>
              <SelectTrigger id="add-category">
                <SelectValue placeholder="Automatic" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={AUTOMATIC}>Automatic</SelectItem>
                {DOWNLOAD_CATEGORIES.map((item) => (
                  <SelectItem key={item} value={item}>
                    {item}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <Label htmlFor="add-directory">Save to</Label>
            <div className="flex gap-2">
              <Input
                id="add-directory"
                value={directory}
                onChange={(event) => setDirectory(event.target.value)}
                placeholder="Default download folder"
              />
              <Button
                type="button"
                variant="outline"
                onClick={() => void handleChooseDirectory()}
              >
                Browse…
              </Button>
            </div>
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => void handlePaste()}
            >
              Paste
            </Button>
            <Button
              type="button"
              variant="ghost"
              onClick={() => setOpen(false)}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              disabled={addDownload.isPending || url.trim().length === 0}
            >
              Add download
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
