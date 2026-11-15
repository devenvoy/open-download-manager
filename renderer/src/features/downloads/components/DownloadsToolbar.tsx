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

const TOOL_BTN_CLS =
  "tool h-[63px] min-w-[53px] px-1 py-1 border border-transparent bg-transparent flex flex-col items-center justify-center gap-1.5 whitespace-nowrap shrink-0 text-[10px] text-inherit cursor-pointer hover:not-disabled:border-[#99b4d1] dark:hover:not-disabled:border-[#4a6a8a] hover:not-disabled:bg-[#e5eff9] dark:hover:not-disabled:bg-[#2d3b4a] active:not-disabled:border-[#6f95bd] dark:active:not-disabled:border-[#5d87b0] active:not-disabled:bg-[#cfdfef] dark:active:not-disabled:bg-[#364a5e] disabled:text-[#929292] dark:disabled:text-[#6a6a6a] disabled:opacity-40 transition-colors";

const TOOL_DIV_CLS =
  "tool-divider w-[1px] self-stretch bg-[#c8c8c8] dark:bg-[#383838] border-r border-white dark:border-[#2b2b2b] my-1.5 mx-1 shrink-0";

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
    <div
      className="toolbar h-[70px] shrink-0 bg-[#f0f0f0] dark:bg-[#1e1e1e] border-t border-white dark:border-[#2b2b2b] border-b border-[#aaa] dark:border-[#383838] flex items-center px-1 py-0.5 overflow-hidden select-none"
      role="toolbar"
      aria-label="Download commands"
    >
      <button
        id="new-download"
        className={TOOL_BTN_CLS}
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
        className={TOOL_BTN_CLS}
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
        className={TOOL_BTN_CLS}
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
        className={TOOL_BTN_CLS}
        data-icon="stopall"
        disabled={!canStopAll}
        onClick={onStopAll}
        type="button"
      >
        <ToolbarIcon name="stopall" />
        <span>Stop all</span>
      </button>

      <span className={TOOL_DIV_CLS} />

      <button
        id="remove-selected"
        className={TOOL_BTN_CLS}
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
        className={`${TOOL_BTN_CLS} wide min-w-[95px]`}
        data-icon="clear"
        disabled={!canDeleteCompleted}
        onClick={onDeleteCompleted}
        type="button"
      >
        <ToolbarIcon name="clear" />
        <span>Delete Completed</span>
      </button>

      <span className={TOOL_DIV_CLS} />

      <button
        id="settings-button"
        className={TOOL_BTN_CLS}
        data-icon="options"
        onClick={onOpenOptions}
        type="button"
      >
        <ToolbarIcon name="options" />
        <span>Options</span>
      </button>

      <button
        id="scheduler-button"
        className={TOOL_BTN_CLS}
        data-icon="clock"
        onClick={onOpenScheduler}
        type="button"
      >
        <ToolbarIcon name="clock" />
        <span>Scheduler</span>
      </button>

      <button
        id="resume-all"
        className={TOOL_BTN_CLS}
        data-icon="startqueue"
        onClick={onStartQueue}
        type="button"
      >
        <ToolbarIcon name="startqueue" />
        <span>Start Queue</span>
      </button>

      <button
        id="stop-queue"
        className={TOOL_BTN_CLS}
        data-icon="stopqueue"
        onClick={onStopQueue}
        type="button"
      >
        <ToolbarIcon name="stopqueue" />
        <span>Stop Queue</span>
      </button>

      <span className={TOOL_DIV_CLS} />

      <button
        id="integration-button"
        className={`${TOOL_BTN_CLS} integration-tool min-w-[104px]`}
        data-icon="browser"
        onClick={onOpenIntegration}
        type="button"
      >
        <ToolbarIcon name="browser" />
        <span>Browser Integration</span>
      </button>

      <button
        id="torrents-button"
        className={TOOL_BTN_CLS}
        data-icon="torrent"
        title="Torrents (Beta)"
        onClick={onOpenTorrents}
        type="button"
      >
        <ToolbarIcon name="torrent" />
        <span>
          Torrents
          <i
            className="beta-tag inline-block ml-0.5 px-0.5 rounded bg-[#d67b1f] text-white text-[8px] font-bold leading-[11px] align-[2px]"
            title="Beta"
          >
            β
          </i>
        </span>
      </button>
    </div>
  );
}
