import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import { componentTagger } from "lovable-tagger";

// Backend gRPC-Web and API target (adjust for your environment)
const API_TARGET = process.env.VITE_API_TARGET || "http://localhost:50051";

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => ({
  server: {
    host: "::",
    port: 8080,
    hmr: {
      overlay: false,
    },
    proxy: {
      // Proxy REST API requests
      "/api": {
        target: API_TARGET,
        changeOrigin: true,
        secure: false,
      },
      // Proxy gRPC-Web requests (binary and JSON)
      "/operation.v1": {
        target: API_TARGET,
        changeOrigin: true,
        secure: false,
        ws: false,
      },
      "/operation.registry.v1": {
        target: API_TARGET,
        changeOrigin: true,
        secure: false,
        ws: false,
      },
    },
  },
  plugins: [react(), mode === "development" && componentTagger()].filter(Boolean),
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
}));
