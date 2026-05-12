import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

export default defineConfig({
  site: "https://alpinevm.github.io",
  base: "/wirebody",
  integrations: [
    starlight({
      title: "Wirebody",
      description: "Apple HealthKit, exposed honestly as JSON. Free, open-source, agent-native.",
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/alpinevm/wirebody",
        },
      ],
      sidebar: [
        {
          label: "Get Started",
          items: [
            { label: "Introduction", slug: "" },
            { label: "Quickstart", slug: "quickstart" },
          ],
        },
        {
          label: "Concepts",
          items: [{ autogenerate: { directory: "concepts" } }],
        },
        {
          label: "MCP Integration",
          items: [{ label: "Overview", slug: "mcp-overview" }],
        },
        {
          label: "API Reference",
          items: [{ autogenerate: { directory: "api-reference" } }],
        },
      ],
      customCss: ["./src/styles/custom.css"],
    }),
  ],
});
