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

/**
 * The one owner of the theme (plan §6.2). The Rust settings are the source of
 * truth; `hydrate` copies them in once the snapshot arrives.
 */
export const useThemeStore = create<ThemeState>()((set, get) => ({
  theme: "system",
  effective: "light",
  setTheme: (theme) => set({ theme, effective: applyTheme(theme) }),
  hydrate: (theme) => {
    if (theme !== get().theme) get().setTheme(theme);
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
