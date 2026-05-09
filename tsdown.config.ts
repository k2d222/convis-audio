import { defineConfig, Rolldown } from "tsdown";
import baseConfig from "../../tsdown.config.ts";

const ignoreNodeFiles: Rolldown.Plugin = {
  name: "ignore-node-files",
  resolveId(source: string) {
    if (source.endsWith(".node")) {
      return { id: source, external: true };
    }
  },
};

export default defineConfig({
  ...baseConfig,
  platform: "node",
  deps: {
    skipNodeModulesBundle: true,
    neverBundle: ["@convis/audio-*"],
  },
  plugins: [...baseConfig.plugins, ignoreNodeFiles],
  loader: {
    ...baseConfig.loader,
    ".node": "empty", // this prevents tsdown from bundling .node files from platform packages
  },
});
