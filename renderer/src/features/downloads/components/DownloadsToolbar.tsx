import { ToolbarIcon } from "@/components/ui/ToolbarIcon";

export interface DownloadsToolbarProps {
  selectedIds: string[];
  canResume?: boolean;
  canStop?: boolean;
  canStopAll?: boolean;
  canDelete?: boolean;
  canDeleteCompleted?: boolean;
  onAdd: () => void;
  onResume: () => void;
  onStop: () => void;
  onStopAll: () => void;
  onDelete: () => void;
  onDeleteCompleted: () => void;
  onOpenOptions: () => void;
  onOpenScheduler: () => void;
  onStartQueue: () => void;
  onStopQueue: () => void;
  onOpenIntegration: () => void;
  onOpenTorrents: () => void;
}

export function DownloadsToolbar({
  selectedIds,
  canResume = selectedIds.length > 0,
  canStop = selectedIds.length > 0,
  canStopAll = true,
  canDelete = selectedIds.length > 0,
  canDeleteCompleted = true,
  onAdd,
  onResume,
  onStop,
  onStopAll,
  onDelete,
  onDeleteCompleted,
  onOpenOptions,
  onOpenScheduler,
  onStartQueue,
  onStopQueue,
  onOpenIntegration,
  onOpenTorrents,
}: DownloadsToolbarProps) {
  return (
    <div className="toolbar" role="toolbar" aria-label="Download commands">
      <button
        id="new-download"
        className="tool"
        data-icon="add"
        title="Add URL (Ctrl+N)"
        onClick={onAdd}
        type="button"
      >
        <ToolbarIcon name="add" />
        <span>Add URL</span>
      </button>

      <button
        id="resume-selected"
        className="tool"
        data-icon="resume"
        disabled={!canResume}
        onClick={onResume}
        type="button"
      >
        <ToolbarIcon name="resume" />
        <span>Resume</span>
      </button>

      <button
        id="stop-selected"
        className="tool"
        data-icon="stop"
        disabled={!canStop}
        onClick={onStop}
        type="button"
      >
        <ToolbarIcon name="stop" />
        <span>Stop</span>
      </button>

      <button
        id="pause-all"
        className="tool"
        data-icon="stopall"
        disabled={!canStopAll}
        onClick={onStopAll}
        type="button"
      >
        <ToolbarIcon name="stopall" />
        <span>Stop all</span>
      </button>

      <span className="tool-divider" />

      <button
        id="remove-selected"
        className="tool"
        data-icon="delete"
        disabled={!canDelete}
        onClick={onDelete}
        type="button"
      >
        <ToolbarIcon name="delete" />
        <span>Delete</span>
      </button>

      <button
        id="remove-completed"
        className="tool wide"
        data-icon="clear"
        disabled={!canDeleteCompleted}
        onClick={onDeleteCompleted}
        type="button"
      >
        <ToolbarIcon name="clear" />
        <span>Delete Completed</span>
      </button>

      <span className="tool-divider" />

      <button
        id="settings-button"
        className="tool"
        data-icon="options"
        onClick={onOpenOptions}
        type="button"
      >
        <ToolbarIcon name="options" />
        <span>Options</span>
      </button>

      <button
        id="scheduler-button"
        className="tool"
        data-icon="clock"
        onClick={onOpenScheduler}
        type="button"
      >
        <ToolbarIcon name="clock" />
        <span>Scheduler</span>
      </button>

      <button
        id="resume-all"
        className="tool"
        data-icon="startqueue"
        onClick={onStartQueue}
        type="button"
      >
        <ToolbarIcon name="startqueue" />
        <span>Start Queue</span>
      </button>

      <button
        id="stop-queue"
        className="tool"
        data-icon="stopqueue"
        onClick={onStopQueue}
        type="button"
      >
        <ToolbarIcon name="stopqueue" />
        <span>Stop Queue</span>
      </button>

      <span className="tool-divider" />

      <button
        id="integration-button"
        className="tool integration-tool"
        data-icon="browser"
        onClick={onOpenIntegration}
        type="button"
      >
        <ToolbarIcon name="browser" />
        <span>Browser Integration</span>
      </button>

      <button
        id="torrents-button"
        className="tool"
        data-icon="torrent"
        title="Torrents (Beta)"
        onClick={onOpenTorrents}
        type="button"
      >
        <ToolbarIcon name="torrent" />
        <span>
          Torrents<i className="beta-tag" title="Beta">β</i>
        </span>
      </button>
    </div>
  );
}
