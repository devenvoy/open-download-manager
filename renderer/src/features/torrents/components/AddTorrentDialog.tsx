import { useState, type FormEvent } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
import { torrentsApi } from "../api";

type Source = "magnet" | "file";

export function AddTorrentDialog({
  onAddMagnet,
  onAddFile,
  pending,
}: {
  onAddMagnet: (magnet: string, options: { outputFolder?: string; startPaused: boolean; review: boolean }) => void;
  onAddFile: (path: string, options: { outputFolder?: string; startPaused: boolean; review: boolean }) => void;
  pending: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [source, setSource] = useState<Source>("magnet");
  const [magnet, setMagnet] = useState("");
  const [path, setPath] = useState("");
  const [outputFolder, setOutputFolder] = useState("");
  const [startPaused, setStartPaused] = useState(false);
  const [review, setReview] = useState(true);
  const [error, setError] = useState<string | null>(null);

  function reset() {
    setMagnet("");
    setPath("");
    setError(null);
  }

  async function browse() {
    const picked = await torrentsApi.chooseTorrentFile();
    if (picked) setPath(picked);
  }

  async function browseFolder() {
    const picked = await torrentsApi.chooseDirectory();
    if (picked) setOutputFolder(picked);
  }

  function submit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    const options = {
      ...(outputFolder.trim() ? { outputFolder: outputFolder.trim() } : {}),
      startPaused,
      review,
    };

    if (source === "magnet") {
      const value = magnet.trim();
      if (!value.startsWith("magnet:")) {
        setError("Paste a magnet link that starts with magnet:?");
        return;
      }
      onAddMagnet(value, options);
    } else {
      if (!path.trim()) {
        setError("Choose a .torrent file first.");
        return;
      }
      onAddFile(path.trim(), options);
    }

    reset();
    setOpen(false);
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>Add torrent</Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={submit} className="space-y-4">
          <DialogHeader>
            <DialogTitle>Add torrent</DialogTitle>
            <DialogDescription>
              Paste a magnet link or choose a .torrent file. With review on,
              the engine fetches metadata and asks which files to download
              before any payload moves.
            </DialogDescription>
          </DialogHeader>

          <div className="flex gap-2" role="group" aria-label="Torrent source">
            <Button
              type="button"
              variant={source === "magnet" ? "default" : "outline"}
              onClick={() => setSource("magnet")}
            >
              Magnet link
            </Button>
            <Button
              type="button"
              variant={source === "file" ? "default" : "outline"}
              onClick={() => setSource("file")}
            >
              Torrent file
            </Button>
          </div>

          {source === "magnet" ? (
            <div className="space-y-2">
              <Label htmlFor="torrent-magnet">Magnet link</Label>
              <Input
                id="torrent-magnet"
                value={magnet}
                onChange={(event) => setMagnet(event.target.value)}
                placeholder="magnet:?xt=urn:btih:…"
                autoFocus
              />
            </div>
          ) : (
            <div className="space-y-2">
              <Label htmlFor="torrent-file">Torrent file</Label>
              <div className="flex gap-2">
                <Input
                  id="torrent-file"
                  value={path}
                  onChange={(event) => setPath(event.target.value)}
                  placeholder="/path/to/file.torrent"
                />
                <Button type="button" variant="outline" onClick={browse}>
                  Browse
                </Button>
              </div>
            </div>
          )}

          <div className="space-y-2">
            <Label htmlFor="torrent-output">Download folder</Label>
            <div className="flex gap-2">
              <Input
                id="torrent-output"
                value={outputFolder}
                onChange={(event) => setOutputFolder(event.target.value)}
                placeholder="Use the torrent settings folder"
              />
              <Button type="button" variant="outline" onClick={browseFolder}>
                Browse
              </Button>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Checkbox
              id="torrent-review"
              checked={review}
              onCheckedChange={(checked) => setReview(checked === true)}
            />
            <Label htmlFor="torrent-review">Choose files before downloading</Label>
          </div>

          <div className="flex items-center gap-2">
            <Checkbox
              id="torrent-paused"
              checked={startPaused}
              onCheckedChange={(checked) => setStartPaused(checked === true)}
            />
            <Label htmlFor="torrent-paused">Add paused</Label>
          </div>

          {error ? (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          ) : null}

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={pending}>
              Add
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
