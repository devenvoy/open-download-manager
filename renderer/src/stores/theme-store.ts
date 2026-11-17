import { useEffect } from "react";
import { create } from "zustand";
import { type AppTheme } from "@/types/ipc";
import {
  applyTheme,
  type EffectiveTheme,
} from "@/lib/ui/theme";

interface ThemeState {
  theme: AppTheme;
  effective: EffectiveTheme;
  setTheme: (theme: AppTheme) => void;
  hydrate: (theme: AppTheme) => void;
  toggle: () => void;
}

const themeChannel =
  typeof window !== "undefined" && typeof BroadcastChannel !== "undefined"
    ? new BroadcastChannel("odm-theme-sync")
    : null;

function getStoredTheme(): AppTheme {
  if (typeof window !== "undefined") {
    const initial = (window as unknown as { __INITIAL_THEME__?: string }).__INITIAL_THEME__;
    if (initial === "light" || initial === "dark" || initial === "system") {
      return initial;
    }
    if (window.localStorage) {
      try {
        const val = localStorage.getItem("odm-theme");
        if (val === "light" || val === "dark" || val === "system") return val;
      } catch {
        // Ignore storage errors
      }
    }
  }
  return "system";
}

const initialTheme = getStoredTheme();
const initialEffective = typeof document !== "undefined" ? applyTheme(initialTheme) : "light";

/**
 * The one owner of the theme (plan §6.2). The Rust settings are the source of
 * truth; `hydrate` copies them in once the snapshot arrives.
 */
export const useThemeStore = create<ThemeState>()((set, get) => ({
  theme: initialTheme,
  effective: initialEffective,
  setTheme: (theme) => {
    try {
      if (typeof window !== "undefined") {
        (window as unknown as { __INITIAL_THEME__?: string }).__INITIAL_THEME__ = theme;
        if (window.localStorage) {
          localStorage.setItem("odm-theme", theme);
        }
      }
      themeChannel?.postMessage({ type: "THEME_CHANGE", theme });
    } catch {
      // Ignore broadcast or storage errors
    }
    set({ theme, effective: applyTheme(theme) });
  },
  hydrate: (theme) => {
    if (theme && theme !== get().theme) {
      get().setTheme(theme);
    }
  },
  toggle: () =>
    get().setTheme(get().effective === "dark" ? "light" : "dark"),
}));

/**
 * Keep the document in sync with the chosen theme and, when the choice is
 * "system", with the OS preference.
 */
export function useThemeSync(): void {
  const theme = useThemeStore((state) => state.theme);

  useEffect(() => {
    if (!themeChannel) return;
    const handleMessage = (e: MessageEvent) => {
      if (e.data && e.data.type === "THEME_CHANGE" && e.data.theme) {
        const newTheme = e.data.theme as AppTheme;
        if (newTheme !== useThemeStore.getState().theme) {
          useThemeStore.setState({
            theme: newTheme,
            effective: applyTheme(newTheme),
          });
        }
      }
    };
    themeChannel.addEventListener("message", handleMessage);
    return () => {
      themeChannel.removeEventListener("message", handleMessage);
    };
  }, []);

  useEffect(() => {
    if (typeof window.matchMedia !== "function") {
      useThemeStore.setState({ effective: applyTheme(theme, false) });
      return;
    }

    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const sync = () =>
      useThemeStore.setState({ effective: applyTheme(theme, media.matches) });

    sync();
    if (theme !== "system") return;

    media.addEventListener("change", sync);
    return () => media.removeEventListener("change", sync);
  }, [theme]);
}

/** Copy the theme out of the Rust settings the first time they arrive. */
export function useHydrateTheme(theme: AppTheme | undefined): void {
  const hydrate = useThemeStore((state) => state.hydrate);

  useEffect(() => {
    if (theme) hydrate(theme);
  }, [theme, hydrate]);
}
