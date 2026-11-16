import { useEffect, useState } from "react";
import { toast } from "sonner";
import { ipc } from "@/lib/ipc";
import { call } from "@/lib/ipc/client";
import { useHydrateTheme } from "@/stores/theme-store";
import { type YouTubeFormatChoice } from "@/types/ipc";
import {
  useAddDefaults,
  useAddPlaylist,
  useAddQueues,
  useSubmitDownload,
} from "../api";
import { AddDownloadForm } from "../components/AddDownloadForm";
import { PlaylistDialog } from "../components/PlaylistDialog";
import { VideoFormatDialog } from "../components/VideoFormatDialog";
import {
  EMPTY_ADD_FORM,
  applyFormatChoice,
  buildAddRequest,
  isYouTubePlaylistUrl,
  isYouTubeVideoUrl,
  splitUrls,
  validateAddForm,
  type AddFormValues,
} from "../model/add-form";

export interface AddDownloadScreenProps {
  paste: boolean;
  scheduledAt?: string;
}

/** The `?add=1` window. */
export function AddDownloadScreen({ paste, scheduledAt }: AddDownloadScreenProps) {
  const defaults = useAddDefaults();
  const queues = useAddQueues();
  const submit = useSubmitDownload();
  const playlist = useAddPlaylist();

  const [values, setValues] = useState<AddFormValues>({
    ...EMPTY_ADD_FORM,
    scheduledAt: scheduledAt ?? "",
  });
  const [error, setError] = useState<string | null>(null);
  const [playlistOpen, setPlaylistOpen] = useState(false);
  const [formatOpen, setFormatOpen] = useState(false);
  const [videoLabel, setVideoLabel] = useState<string | null>(null);

  useHydrateTheme(defaults.data?.settings.theme);

  useEffect(() => {
    void call("take_add_window_options", undefined)
      .then((opts) => {
        if (!opts) return;
        if (opts.scheduled) {
          setValues((cur) => ({
            ...cur,
            scheduledAt: opts.scheduled ?? cur.scheduledAt,
          }));
        }
      })
      .catch(() => {});
  }, []);

  // Start in the configured folder; a picked category can override it.
  useEffect(() => {
    const directory = defaults.data?.settings.defaultDirectory;
    if (!directory) return;
    setValues((current) =>
      current.directory ? current : { ...current, directory },
    );
  }, [defaults.data?.settings.defaultDirectory]);

  useEffect(() => {
    if (!paste) return;
    let cancelled = false;
    void ipc
      .readClipboard()
      .then((text) => {
        if (!cancelled && text) {
          setValues((current) => ({ ...current, urlsText: text }));
        }
      })
      .catch(() => {
        if (!cancelled) setError("Could not read the clipboard.");
      });
    return () => {
      cancelled = true;
    };
  }, [paste]);

  const urls = splitUrls(values.urlsText);
  const firstUrl = urls[0] ?? "";
  const showPlaylistBanner = isYouTubePlaylistUrl(firstUrl);
  const showVideoBanner = isYouTubeVideoUrl(firstUrl);

  function patch(next: Partial<AddFormValues>) {
    setValues((current) => {
      const merged = { ...current, ...next };
      // Picking a category jumps to its configured folder, like the legacy
      // window, unless the user already typed a folder for this download.
      if (next.category !== undefined && next.category) {
        const folder = defaults.data?.settings.categoryDirectories?.[next.category];
        if (folder) merged.directory = folder;
      }
      return merged;
    });
  }

  async function chooseDirectory() {
    try {
      const picked = await ipc.chooseDirectory();
      if (picked) patch({ directory: picked });
    } catch {
      setError("Could not open the folder picker.");
    }
  }

  async function pasteClipboard() {
    try {
      const text = await ipc.readClipboard();
      patch({ urlsText: text });
    } catch {
      setError("Could not read the clipboard.");
    }
  }

  function handleSubmit(queueOnly: boolean) {
    const problem = validateAddForm({
      urls,
      filename: values.filename,
      scheduledAt: values.scheduledAt,
    });
    if (problem) {
      setError(problem);
      return;
    }
    setError(null);
    submit.mutate(buildAddRequest(values, queueOnly), {
      onSuccess: async (ids) => {
        const [first] = ids;
        if (!queueOnly && !values.scheduledAt && first) {
          await ipc.openProgressWindow(first);
        }
        await ipc.closeAuxiliaryWindow();
      },
    });
  }

  function handleFormat(choice: YouTubeFormatChoice) {
    const applied = applyFormatChoice(choice, firstUrl);
    patch({
      filename: applied.filename,
      category: applied.category,
      youtubeFormat: choice.youtubeFormat,
    });
    setVideoLabel(applied.label);
    setFormatOpen(false);
  }

  function handlePlaylist(
    items: Array<{ id: string; url: string }>,
    metadata: { id: string; title: string },
  ) {
    playlist.mutate(
      {
        playlistId: metadata.id,
        playlistTitle: metadata.title,
        items,
        ...(values.category ? { commonCategory: values.category } : {}),
        ...(values.directory ? { commonDirectory: values.directory } : {}),
      },
      {
        onSuccess: async () => {
          setPlaylistOpen(false);
          toast.success(`Added ${items.length} playlist videos to downloads.`);
          await ipc.closeAuxiliaryWindow();
        },
        onError: (problem) =>
          setError(
            problem instanceof Error
              ? problem.message
              : "Could not add the playlist.",
          ),
      },
    );
  }

  return (
    <dialog
      id="add-dialog"
      className="add-dialog w-full h-screen border-0 m-0 max-w-none max-h-none flex flex-col p-0 static"
      open
    >
      <AddDownloadForm
        values={values}
        error={error}
        saving={submit.isPending}
        showPlaylistBanner={showPlaylistBanner}
        showVideoBanner={showVideoBanner}
        videoLabel={videoLabel}
        queueOptions={queues.data ?? [{ id: "default", name: "Default Queue" }]}
        onPatch={patch}
        onUrlsChange={(text) => patch({ urlsText: text })}
        onChooseDirectory={() => void chooseDirectory()}
        onPaste={() => void pasteClipboard()}
        onOpenPlaylist={() => setPlaylistOpen(true)}
        onOpenFormats={() => setFormatOpen(true)}
        onSubmit={handleSubmit}
        onCancel={() => void ipc.closeAuxiliaryWindow()}
      />

      <PlaylistDialog
        open={playlistOpen}
        url={firstUrl}
        category={values.category}
        directory={values.directory}
        adding={playlist.isPending}
        onOpenChange={setPlaylistOpen}
        onAdd={handlePlaylist}
      />

      <VideoFormatDialog
        open={formatOpen}
        url={firstUrl}
        onOpenChange={setFormatOpen}
        onPick={handleFormat}
      />
    </dialog>
  );
}
