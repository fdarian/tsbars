import type { BunPlugin } from "bun";
import { compile, type CompileOptions } from "./compile.js";

export default function tsbars(options?: CompileOptions): BunPlugin {
  return {
    name: "tsbars",
    setup(build) {
      build.onLoad({ filter: /\.hbs$/ }, async (args) => {
        const source = await Bun.file(args.path).text();
        return { contents: compile(source, options), loader: "js" };
      });
    },
  };
}
