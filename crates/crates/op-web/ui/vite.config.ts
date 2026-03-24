import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import { componentTagger } from "lovable-tagger";

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => ({
  server: {
    host: "::",
    port: 8080,
    hmr: {
      overlay: false,
    },
  },
  plugins: [react(), mode === "development" && componentTagger()].filter(Boolean),
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      react: path.resolve(__dirname, "node_modules/react"),
      "react-dom": path.resolve(__dirname, "node_modules/react-dom"),
      // Use WASM rollup on Chimera Linux (musl + glibc-compat reports fake glibc version,
      // causing native rollup-linux-x64-gnu bindings to crash).
      rollup: path.resolve(__dirname, "node_modules/@rollup/wasm-node"),
    },
  },
}));
