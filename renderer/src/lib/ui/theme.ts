import { type AppTheme } from "@/types/ipc";

export type EffectiveTheme = "light" | "dark";

export function getEffectiveTheme(
  theme: AppTheme,
  systemPrefersDark: boolean,
): EffectiveTheme {
  if (theme === "light" || theme === "dark") return theme;
  return systemPrefersDark ? "dark" : "light";
}

export function systemPrefersDark(): boolean {
  if (typeof window === "undefined" || !window.matchMedia) return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

/**
 * Apply a theme to the document. Tokens live in index.css; this only flips
 * the attribute the CSS keys off (plan R15).
 */
export function applyTheme(
  theme: AppTheme,
  prefersDark: boolean = systemPrefersDark(),
): EffectiveTheme {
  const effective = getEffectiveTheme(theme, prefersDark);
  if (typeof document !== "undefined") {
    const root = document.documentElement;
    root.setAttribute("data-theme", effective);
    root.style.colorScheme = effective;
    if (document.body) {
      document.body.setAttribute("data-theme", effective);
    }
  }
  return effective;
}
