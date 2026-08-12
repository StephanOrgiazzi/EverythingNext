// @ts-check
import { defineConfig } from "astro/config";
import sitemap from "@astrojs/sitemap";

export default defineConfig({
  site: "https://stephanorgiazzi.github.io/EverythingNext",
  base: "/EverythingNext/",
  trailingSlash: "always",
  integrations: [sitemap()],
});
