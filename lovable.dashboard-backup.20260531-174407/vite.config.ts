import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";
import path from "path";
import { componentTagger } from "lovable-tagger";
import { zeroclawDevShim } from "./dev-shim.mjs";

const apiProxyTarget = process.env.VITE_API_PROXY || "http://100.90.37.254";

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => ({
  server: {
    host: "::",
    port: 9090,
    hmr: {
      overlay: false,
    },
    proxy: {
      "/api": {
        target: apiProxyTarget,
        changeOrigin: true,
      },
      "/ws": {
        target: apiProxyTarget,
        changeOrigin: true,
        ws: true,
      },
    },
  },
  plugins: [react(), zeroclawDevShim(), mode === "development" && componentTagger()].filter(
    Boolean,
  ),
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      react: path.resolve(__dirname, "node_modules/react"),
      "react-dom": path.resolve(__dirname, "node_modules/react-dom"),
    },
  },
}));
