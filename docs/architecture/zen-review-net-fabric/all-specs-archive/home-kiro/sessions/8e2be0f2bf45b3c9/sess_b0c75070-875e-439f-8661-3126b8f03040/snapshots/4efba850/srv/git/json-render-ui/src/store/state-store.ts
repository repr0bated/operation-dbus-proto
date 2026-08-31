import { create } from "zustand";

export interface StateStore {
  connected: boolean;
  plugins: Record<string, Record<string, unknown>>;
  /** Set connection status */
  setConnected: (connected: boolean) => void;
  /** Apply a state update for a plugin (full or member-level) */
  applyUpdate: (pluginId: string, memberName: string | undefined, value: unknown) => void;
  /** Get a nested value by dot-path */
  getValue: (path: string) => unknown;
}

export const useStateStore = create<StateStore>((set, get) => ({
  connected: false,
  plugins: {},

  setConnected: (connected) => set({ connected }),

  applyUpdate: (pluginId, memberName, value) =>
    set((state) => {
      const plugin = state.plugins[pluginId] ?? {};
      if (!memberName) {
        // Full plugin state replace
        const newState = value && typeof value === "object" && !Array.isArray(value)
          ? (value as Record<string, unknown>)
          : { _value: value };
        return { plugins: { ...state.plugins, [pluginId]: newState } };
      }
      // Member-level update
      return {
        plugins: {
          ...state.plugins,
          [pluginId]: { ...plugin, [memberName]: value },
        },
      };
    }),

  getValue: (path) => {
    const parts = path.split(".");
    let current: unknown = get().plugins;
    for (const part of parts) {
      if (current === null || current === undefined || typeof current !== "object") return undefined;
      current = (current as Record<string, unknown>)[part];
    }
    return current;
  },
}));
