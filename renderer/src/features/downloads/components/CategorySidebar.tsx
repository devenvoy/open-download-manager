import { useState } from "react";
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
    <aside className="categories" aria-label="Categories">
      <div className="pane-heading">
        Categories <span aria-hidden="true">×</span>
      </div>
      <nav className="category-tree" aria-label="Filter downloads">
        <button
          type="button"
          className={`tree-item root ${currentCategory === "all" ? "selected" : ""}`}
          data-filter="all"
          aria-current={currentCategory === "all" ? "page" : undefined}
          onClick={() => setCategory("all")}
        >
          <span
            className="tree-expander"
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
                className={`tree-item child ${currentCategory === cat.name ? "selected" : ""}`}
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
          className={`tree-item root ${currentCategory === "unfinished" ? "selected" : ""}`}
          data-filter="unfinished"
          onClick={() => setCategory("unfinished")}
        >
          <span className="tree-expander spacer" aria-hidden="true" />
          <span className="folder-icon unfinished" />
          Unfinished
        </button>

        <button
          type="button"
          className={`tree-item root ${currentCategory === "completed" ? "selected" : ""}`}
          data-filter="completed"
          onClick={() => setCategory("completed")}
        >
          <span className="tree-expander spacer" aria-hidden="true" />
          <span className="folder-icon finished" />
          Finished
        </button>

        <button
          type="button"
          className="tree-item root"
          id="grabber-button"
        >
          <span className="tree-expander spacer" aria-hidden="true" />
          <span className="folder-icon grabber" />
          Grabber projects
        </button>

        <button
          type="button"
          className={`tree-item root ${currentCategory === "queued" ? "selected" : ""}`}
          data-filter="queued"
          onClick={() => setCategory("queued")}
        >
          <span className="tree-expander spacer" aria-hidden="true" />
          <span className="folder-icon queues" />
          Queues
        </button>
      </nav>
    </aside>
  );
}
