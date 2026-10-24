import type { Plugin } from "vite";
import { compile, type CompileOptions } from "./compile.js";

export default function tsbars(options?: CompileOptions): Plugin {
  return {
    name: "tsbars",
    enforce: "pre",
    transform(code, id) {
      const path = id.split("?")[0];
      if (!path.endsWith(".hbs")) return null;
      return { code: compile(code, options), map: null };
    },
  };
}
