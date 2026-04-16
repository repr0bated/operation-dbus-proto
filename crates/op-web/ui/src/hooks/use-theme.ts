import { useState, useEffect } from "react";

type ThemeMode = "dark" | "light" | "system";
type ResolvedTheme = "dark" | "light";

function getSystemTheme(): ResolvedTheme {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function resolveTheme(mode: ThemeMode): ResolvedTheme {
  return mode === "system" ? getSystemTheme() : mode;
}

export function useTheme() {
  const [mode, setMode] = useState<ThemeMode>(() => {
    const stored = localStorage.getItem("dbus-theme") as ThemeMode | null;
    return stored || "dark";
  });

  const resolved = resolveTheme(mode);

  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("dark", "light");
    root.classList.add(resolved);
    localStorage.setItem("dbus-theme", mode);
  }, [mode, resolved]);

  const setTheme = (next: ThemeMode) => setMode(next);
  const toggleTheme = () => {
    setMode((m) => (m === "dark" ? "light" : m === "light" ? "system" : "dark"));
  };

  return { theme: mode, resolved, setTheme, toggleTheme };
}
