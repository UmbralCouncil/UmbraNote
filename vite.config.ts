import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid()],
  build: { target: "es2022" },
  test: { environment: "node", include: ["src-web/**/*.test.ts"] },
});
