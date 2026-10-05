import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid()],
  // Electron loads the production renderer from file://, so assets must be
  // relative to index.html rather than rooted at the filesystem URL.
  base: "./",
  build: { target: "es2022" },
  test: { environment: "node", include: ["src-web/**/*.test.ts"] },
});
