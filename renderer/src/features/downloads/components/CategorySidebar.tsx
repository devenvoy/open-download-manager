import { useState } from "react";
import { cn } from "@/lib/utils";
import { type DownloadCategory } from "@/types/ipc";
import { useDownloadsStore } from "../state/downloads-store";

const CATEGORIES: Array<{
  name: DownloadCategory;
  label: string;
  iconClass: string;
}> = [
  { name: "Archives", label: "Compressed", iconClass: "compressed" },
  { name: "Documents", label: "Documents", iconClass: "documents" },
  { name: "Audio", label: "Music", iconClass: "music" },
  { name: "Programs", label: "Programs", iconClass: "programs" },
  { name: "Video", label: "Video", iconClass: "videos" },
  { name: "Other", label: "Other", iconClass: "" },
];

export function CategorySidebar() {
  const currentCategory = useDownloadsStore((state) => state.category);
  const setCategory = useDownloadsStore((state) => state.setCategory);
  const [collapsed, setCollapsed] = useState(false);

  return (
    <aside
      className="categories w-[150px] shrink-0 border-r-3 border-[#ddd] dark:border-[#383838] bg-white dark:bg-[#1e1e1e] overflow-auto select-none"
      aria-label="Categories"
    >
      <div className="pane-heading h-[23px] flex items-center justify-between px-1.5 bg-[#f0f0f0] dark:bg-[#252526] border-b border-[#bbb] dark:border-[#383838] text-[11px] font-semibold text-[#111] dark:text-[#e0e0e0]">
        Categories <span aria-hidden="true" className="text-[#555] dark:text-[#888] font-mono text-[13px]">×</span>
      </div>
      <nav className="category-tree py-1 px-0.5 min-w-[142px] flex flex-col gap-0.5" aria-label="Filter downloads">
        <button
          type="button"
          className={cn(
            "tree-item root flex items-center gap-1 w-full h-[21px] whitespace-nowrap text-[11px] px-1 py-0.5 border-0 bg-transparent text-left cursor-pointer transition-colors",
            currentCategory === "all"
              ? "selected bg-[#cce8ff] dark:bg-[#1f3d5c] outline outline-1 outline-[#99d1ff] dark:outline-[#2c5885] -outline-offset-1 text-[#111] dark:text-[#e0e0e0]"
              : "hover:bg-[#e9f2fc] dark:hover:bg-[#2d3742] text-[#333] dark:text-[#c0c0c0]",
          )}
          data-filter="all"
          aria-current={currentCategory === "all" ? "page" : undefined}
          onClick={() => setCategory("all")}
        >
          <span
            className="tree-expander text-[12px] text-[#777] dark:text-[#999] w-3 shrink-0 text-center"
            role="button"
            tabIndex={0}
            aria-label="Collapse All Downloads categories"
            aria-expanded={!collapsed}
            aria-controls="all-download-categories"
            onClick={(e) => {
              e.stopPropagation();
              setCollapsed(!collapsed);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.stopPropagation();
                setCollapsed(!collapsed);
              }
            }}
          >
            {collapsed ? "⊞" : "⊟"}
          </span>
          <span className="folder-icon" />
          All Downloads
        </button>

        {!collapsed && (
          <div className="tree-children" id="all-download-categories">
            {CATEGORIES.map((cat) => (
              <button
                key={cat.name}
                type="button"
                className={cn(
                  "tree-item child flex items-center gap-1 w-full h-[21px] whitespace-nowrap text-[11px] px-1 py-0.5 pl-7 border-0 bg-transparent text-left cursor-pointer transition-colors",
                  currentCategory === cat.name
                    ? "selected bg-[#cce8ff] dark:bg-[#1f3d5c] outline outline-1 outline-[#99d1ff] dark:outline-[#2c5885] -outline-offset-1 text-[#111] dark:text-[#e0e0e0]"
                    : "hover:bg-[#e9f2fc] dark:hover:bg-[#2d3742] text-[#333] dark:text-[#c0c0c0]",
                )}
                data-category={cat.name}
                onClick={() => setCategory(cat.name)}
              >
                <span className={`folder-icon ${cat.iconClass}`} />
                {cat.label}
              </button>
            ))}
          </div>
        )}

        <button
          type="button"
          className={cn(
            "tree-item root flex items-center gap-1 w-full h-[21px] whitespace-nowrap text-[11px] px-1 py-0.5 border-0 bg-transparent text-left cursor-pointer transition-colors",
            currentCategory === "unfinished"
              ? "selected bg-[#cce8ff] dark:bg-[#1f3d5c] outline outline-1 outline-[#99d1ff] dark:outline-[#2c5885] -outline-offset-1 text-[#111] dark:text-[#e0e0e0]"
              : "hover:bg-[#e9f2fc] dark:hover:bg-[#2d3742] text-[#333] dark:text-[#c0c0c0]",
          )}
          data-filter="unfinished"
          onClick={() => setCategory("unfinished")}
        >
          <span className="tree-expander spacer text-[12px] w-3 shrink-0" aria-hidden="true" />
          <span className="folder-icon unfinished" />
          Unfinished
        </button>

        <button
          type="button"
          className={cn(
            "tree-item root flex items-center gap-1 w-full h-[21px] whitespace-nowrap text-[11px] px-1 py-0.5 border-0 bg-transparent text-left cursor-pointer transition-colors",
            currentCategory === "completed"
              ? "selected bg-[#cce8ff] dark:bg-[#1f3d5c] outline outline-1 outline-[#99d1ff] dark:outline-[#2c5885] -outline-offset-1 text-[#111] dark:text-[#e0e0e0]"
              : "hover:bg-[#e9f2fc] dark:hover:bg-[#2d3742] text-[#333] dark:text-[#c0c0c0]",
          )}
          data-filter="completed"
          onClick={() => setCategory("completed")}
        >
          <span className="tree-expander spacer text-[12px] w-3 shrink-0" aria-hidden="true" />
          <span className="folder-icon finished" />
          Finished
        </button>

        <button
          type="button"
          className="tree-item root flex items-center gap-1 w-full h-[21px] whitespace-nowrap text-[11px] px-1 py-0.5 border-0 bg-transparent text-left cursor-pointer hover:bg-[#e9f2fc] dark:hover:bg-[#2d3742] text-[#333] dark:text-[#c0c0c0] transition-colors"
          id="grabber-button"
        >
          <span className="tree-expander spacer text-[12px] w-3 shrink-0" aria-hidden="true" />
          <span className="folder-icon grabber" />
          Grabber projects
        </button>

        <button
          type="button"
          className={cn(
            "tree-item root flex items-center gap-1 w-full h-[21px] whitespace-nowrap text-[11px] px-1 py-0.5 border-0 bg-transparent text-left cursor-pointer transition-colors",
            currentCategory === "queued"
              ? "selected bg-[#cce8ff] dark:bg-[#1f3d5c] outline outline-1 outline-[#99d1ff] dark:outline-[#2c5885] -outline-offset-1 text-[#111] dark:text-[#e0e0e0]"
              : "hover:bg-[#e9f2fc] dark:hover:bg-[#2d3742] text-[#333] dark:text-[#c0c0c0]",
          )}
          data-filter="queued"
          onClick={() => setCategory("queued")}
        >
          <span className="tree-expander spacer text-[12px] w-3 shrink-0" aria-hidden="true" />
          <span className="folder-icon queues" />
          Queues
        </button>
      </nav>
    </aside>
  );
}
