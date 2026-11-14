import { useThemeStore } from "@/stores/theme-store";

export const TOOLBAR_PATHS: Record<string, string[] | { light: string[]; dark: string[] }> = {
  add: ["M5 9h8l2-3h6l2 3h6v20H5z", "M17 13v12M11 19h12"],
  resume: [
    "M16 2a14 14 0 1 0 0 28a14 14 0 0 0 0-28",
    "M16 8v16M10 18l6 6 6-6",
  ],
  stop: [
    "M9 17V8a2 2 0 0 1 4 0v8V4a2 2 0 0 1 4 0v12V6a2 2 0 0 1 4 0v11V10a2 2 0 0 1 4 0v12q0 8-9 8h-1q-4 0-7-5L3 18q-2-4 2-4z",
  ],
  stopall: [
    "M9 17V8a2 2 0 0 1 4 0v8V4a2 2 0 0 1 4 0v12V6a2 2 0 0 1 4 0v11V10a2 2 0 0 1 4 0v12q0 8-9 8h-1q-4 0-7-5L3 18q-2-4 2-4z",
  ],
  delete: ["M5 7h22M12 7V3h8v4M7 10l2 20h14l2-20M13 12v14M19 12v14"],
  clear: ["M5 6h22l-2 24H7zM12 6V3h8v3", "M10 17l5 5 8-11"],
  options: {
    light: [
      "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.38a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.51a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z",
      "M12 9a3 3 0 1 0 0 6a3 3 0 0 0 0-6",
    ],
    dark: [
      "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.38a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.51a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z",
      "M12 9a3 3 0 1 0 0 6a3 3 0 0 0 0-6",
    ],
  },
  clock: [
    "M16 4a13 13 0 1 0 0 26a13 13 0 0 0 0-26",
    "M6 5 3 2M26 5l3-3M16 9v8l6-6",
  ],
  startqueue: ["M2 2h23v24H2zM7 7h23v23H7z", "M22 12v18M16 24l6 6 6-6"],
  stopqueue: ["M2 2h23v24H2zM7 7h23v23H7z", "M19 18h12v13H19z"],
  browser: [
    "M16 3a13 13 0 1 0 0 26a13 13 0 0 0 0-26",
    "M3 16h26M16 3c-8 8-8 18 0 26M16 3c8 8 8 18 0 26M6 8h20M6 24h20",
  ],
  torrent: {
    light: [
      "M8 27V16a9 9 0 0 1 18 0v11",
      "M14 27V16a3 3 0 0 1 6 0v11",
      "M8 22h6M20 22h6",
    ],
    dark: [
      "M8 27V16a9 9 0 0 1 18 0v11",
      "M14 27V16a3 3 0 0 1 6 0v11",
      "M8 22h6M20 22h6",
    ],
  },
};

export function ToolbarIcon({ name }: { name: string }) {
  const effective = useThemeStore((state) => state.effective);
  const entry = TOOLBAR_PATHS[name];
  if (!entry) return null;

  const paths = Array.isArray(entry)
    ? entry
    : effective === "dark"
      ? entry.dark
      : entry.light;

  return (
    <svg
      viewBox={name === "options" ? "0 0 24 24" : "0 0 34 34"}
      aria-hidden="true"
    >
      {paths.map((d, i) => (
        <path key={i} d={d} />
      ))}
    </svg>
  );
}
