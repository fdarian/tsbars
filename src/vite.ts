import type { Plugin } from "vite";
import { compile, type CompileOptions } from "#src/compile.ts";

export default function tsbars(options?: CompileOptions): Plugin {
  return {
    name: "tsbars",
    enforce: "pre",
    transform(code, id) {
      const path = id.replace(/\?.*$/, "");
      if (!path.endsWith(".hbs")) return null;
      return { code: compile(code, options), map: null };
    },
  };
}
