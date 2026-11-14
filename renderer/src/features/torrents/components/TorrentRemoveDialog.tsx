import { ClassicDialog } from "@/components/feedback";
import { type TorrentListItem } from "@/types/ipc";

export interface TorrentRemoveDialogProps {
  torrent: TorrentListItem;
  disabled: boolean;
  onAction: (action: "forget" | "delete") => void;
  onClose: () => void;
}

export function TorrentRemoveDialog({
  torrent,
  disabled,
  onAction,
  onClose,
}: TorrentRemoveDialogProps) {
  return (
    <ClassicDialog
      id="torrent-remove-dialog"
      className="torrent-dialog torrent-remove-dialog"
      open
      onClose={onClose}
      title={<span>Remove Torrent</span>}
    >
      <div className="dialog-body">
        <p>
          <strong id="torrent-remove-name">{torrent.name || torrent.infoHash}</strong>
        </p>
        <p className="field-help">
          Remove from list keeps the downloaded files on disk. Delete also removes
          the downloaded data.
        </p>
      </div>
      <div className="dialog-footer torrent-remove-footer">
        <button
          type="button"
          className="button"
          data-close="torrent-remove-dialog"
          onClick={onClose}
        >
          Cancel
        </button>
        <button
          type="button"
          id="torrent-remove-forget"
          className="button"
          disabled={disabled}
          onClick={() => {
            onAction("forget");
            onClose();
          }}
        >
          Remove from list
        </button>
        <button
          type="button"
          id="torrent-remove-delete"
          className="button danger"
          disabled={disabled}
          onClick={() => {
            onAction("delete");
            onClose();
          }}
        >
          Delete files
        </button>
      </div>
    </ClassicDialog>
  );
}
