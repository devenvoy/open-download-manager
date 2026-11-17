import {
  Sliders,
  Palette,
  FolderDown,
  Wifi,
  Shield,
  Filter,
  KeyRound,
  Video,
  Sparkles,
  type LucideIcon,
} from "lucide-react";

export type SettingsTabId =
  | "general"
  | "appearance"
  | "storage"
  | "connection"
  | "proxy"
  | "capture"
  | "credentials"
  | "media"
  | "automation";

interface TabItem {
  id: SettingsTabId;
  label: string;
  icon: LucideIcon;
}

const TABS: TabItem[] = [
  { id: "general", label: "General", icon: Sliders },
  { id: "appearance", label: "Appearance", icon: Palette },
  { id: "storage", label: "Save To", icon: FolderDown },
  { id: "connection", label: "Connection", icon: Wifi },
  { id: "proxy", label: "Proxy", icon: Shield },
  { id: "capture", label: "File Types", icon: Filter },
  { id: "credentials", label: "Site Logins", icon: KeyRound },
  { id: "media", label: "Video Tools", icon: Video },
  { id: "automation", label: "Automation", icon: Sparkles },
];

export interface SettingsSidebarProps {
  activeTab: SettingsTabId;
  onSelectTab: (tab: SettingsTabId) => void;
}

export function SettingsSidebar({ activeTab, onSelectTab }: SettingsSidebarProps) {
  return (
    <aside
      className="settings-sidebar w-[180px] shrink-0 border-r border-[#e0e0e0] dark:border-[#2b2b2b] bg-[#f8f9fa] dark:bg-[#18181b] flex flex-col p-2 select-none overflow-y-auto"
      role="tablist"
      aria-label="Settings categories"
    >
      <div className="space-y-1">
        {TABS.map((tab) => {
          const Icon = tab.icon;
          const isActive = activeTab === tab.id;

          return (
            <button
              key={tab.id}
              type="button"
              role="tab"
              aria-selected={isActive}
              data-tab={tab.id}
              onClick={() => onSelectTab(tab.id)}
              className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-md text-xs font-medium text-left transition-all duration-150 cursor-pointer ${
                isActive
                  ? "bg-blue-600 text-white shadow-sm dark:bg-blue-600 dark:text-white"
                  : "text-[#4b5563] dark:text-[#a1a1aa] hover:bg-[#e9ecef] dark:hover:bg-[#27272a] hover:text-[#111827] dark:hover:text-[#f4f4f5]"
              }`}
            >
              <Icon
                size={16}
                strokeWidth={isActive ? 2.2 : 1.8}
                className={isActive ? "text-white" : "text-[#6b7280] dark:text-[#71717a]"}
                aria-hidden="true"
              />
              <span className="truncate">{tab.label}</span>
            </button>
          );
        })}
      </div>
    </aside>
  );
}
