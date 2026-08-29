import { cssVariables } from "@fasti/tokens";
import type { Config } from "@docusaurus/types";
import { themes as prismThemes } from "prism-react-renderer";
import { resolve } from "node:path";

const config: Config = {
  title: "Fasti Documentation",
  tagline: "A local system of record for portable media activity",
  favicon: "brand/logos/fasti-icon.svg",
  url: "https://fasti.scrobble.dev",
  baseUrl: "/",
  organizationName: "Scrobble-dev",
  projectName: "Fasti",
  trailingSlash: true,
  onBrokenLinks: "throw",
  onDuplicateRoutes: "throw",
  headTags: [
    {
      tagName: "style",
      attributes: { id: "fasti-tokens" },
      innerHTML: cssVariables,
    },
  ],
  staticDirectories: [resolve(__dirname, "../../target/docs-site/static")],
  presets: [
    [
      "classic",
      {
        docs: {
          path: resolve(__dirname, "../../target/docs-site/content"),
          routeBasePath: "/",
          sidebarPath: resolve(__dirname, "sidebars.ts"),
          editUrl: "https://github.com/Scrobble-dev/Fasti/edit/dev/",
          showLastUpdateAuthor: false,
          showLastUpdateTime: false,
        },
        blog: false,
        sitemap: { changefreq: "weekly", priority: 0.5 },
        theme: { customCss: resolve(__dirname, "src/css/custom.css") },
      },
    ],
  ],
  themeConfig: {
    image: "brand/logos/fasti-lockup.svg",
    announcementBar: {
      id: "unsupported-release",
      content:
        "Fasti has no supported public release. Read contract, runtime, and support states separately.",
      backgroundColor: "#8b2e2a",
      textColor: "#fffdf8",
      isCloseable: false,
    },
    navbar: {
      title: "Fasti",
      logo: { alt: "Fasti", src: "brand/logos/fasti-icon.svg" },
      items: [
        {
          to: "/start/choose-a-path/",
          label: "Choose a path",
          position: "left",
        },
        { to: "/status/", label: "Status", position: "left" },
        { to: "/deploy/", label: "Deployment planner", position: "left" },
        { to: "/search/", label: "Search", position: "right" },
        {
          href: "https://github.com/Scrobble-dev/Fasti",
          label: "Repository",
          position: "right",
        },
      ],
    },
    footer: {
      style: "dark",
      links: [
        {
          title: "Documentation",
          items: [
            { label: "Start", to: "/start/what-fasti-is/" },
            { label: "Contracts", to: "/reference/contracts/" },
            { label: "Accessibility", to: "/accessibility/" },
            { label: "Security", to: "/security/" },
          ],
        },
        {
          title: "Contribute",
          items: [
            {
              label: "Fasti repository",
              href: "https://github.com/Scrobble-dev/Fasti",
            },
            {
              label: "Fasti issues",
              href: "https://github.com/Scrobble-dev/Fasti/issues",
            },
          ],
        },
      ],
      copyright: `Copyright ${new Date().getFullYear()} Fasti contributors. AGPL-3.0-or-later.`,
    },
    prism: { theme: prismThemes.github, darkTheme: prismThemes.dracula },
    colorMode: { respectPrefersColorScheme: true },
  },
};

export default config;
