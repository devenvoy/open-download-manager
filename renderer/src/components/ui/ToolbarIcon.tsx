import {
  Plus,
  Play,
  Pause,
  Square,
  Trash2,
  CheckCheck,
  Settings2,
  Clock,
  ListPlus,
  ListX,
  Globe,
  Magnet,
  type LucideIcon,
} from "lucide-react";

interface IconDef {
  icon: LucideIcon;
  colorCls: string;
}

const ICON_MAP: Record<string, IconDef> = {
  add: { icon: Plus, colorCls: "text-emerald-600 dark:text-emerald-400" },
  resume: { icon: Play, colorCls: "text-blue-600 dark:text-blue-400" },
  stop: { icon: Pause, colorCls: "text-amber-600 dark:text-amber-400" },
  stopall: { icon: Square, colorCls: "text-rose-600 dark:text-rose-400" },
  delete: { icon: Trash2, colorCls: "text-rose-600 dark:text-rose-400" },
  clear: { icon: CheckCheck, colorCls: "text-indigo-600 dark:text-indigo-400" },
  options: { icon: Settings2, colorCls: "text-slate-700 dark:text-slate-300" },
  clock: { icon: Clock, colorCls: "text-sky-600 dark:text-sky-400" },
  startqueue: { icon: ListPlus, colorCls: "text-emerald-600 dark:text-emerald-400" },
  stopqueue: { icon: ListX, colorCls: "text-amber-600 dark:text-amber-400" },
  browser: { icon: Globe, colorCls: "text-teal-600 dark:text-teal-400" },
  torrent: { icon: Magnet, colorCls: "text-orange-500 dark:text-orange-400" },
};

export function ToolbarIcon({ name }: { name: string }) {
  const item = ICON_MAP[name];
  if (!item) return null;
  const Icon = item.icon;

  return (
    <Icon
      size={22}
      strokeWidth={2}
      className={`shrink-0 transition-transform duration-100 group-hover:scale-105 ${item.colorCls}`}
      aria-hidden="true"
    />
  );
}
