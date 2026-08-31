import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";
import path from "path";

const GRPC_TARGET = process.env.VITE_GRPC_PROXY_TARGET || "http://127.0.0.1:8090";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    host: "::",
    port: 4173,
    proxy: {
      "/operation.v1": {
        target: GRPC_TARGET,
        changeOrigin: true,
      },
      "/operation.registry.v1": {
        target: GRPC_TARGET,
        changeOrigin: true,
      },
      "/operation.plugin.v1": {
        target: GRPC_TARGET,
        changeOrigin: true,
      },
    },
  },
});
