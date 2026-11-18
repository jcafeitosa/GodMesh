import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Porta fixa: o Tauri (devUrl em src-tauri/tauri.conf.json) espera 1420.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { outDir: "dist", target: "safari15" },
});
