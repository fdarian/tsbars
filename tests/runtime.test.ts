import { expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "vite";
import tsbarsBun from "../src/bun.ts";
import { compile } from "../src/compile.ts";
import tsbarsVite from "../src/vite.ts";

type Template = (context: unknown) => string;

const context = { name: "Ada", who: "<b>", items: ["x", "y"] };
const fixtures = join(import.meta.dir, "fixtures");

async function importCompiled(source: string, options?: Parameters<typeof compile>[1]) {
  // Written inside the repo so `handlebars/runtime` resolves from node_modules.
  const dir = mkdtempSync(join(import.meta.dir, ".tmp-"));
  try {
    const file = join(dir, "template.mjs");
    writeFileSync(file, compile(source, options));
    const mod = await import(pathToFileURL(file).href);
    return mod.default as Template;
  } finally {
    rmSync(dir, { recursive: true });
  }
}

test("compile emits a module using handlebars/runtime", () => {
  const code = compile("hi {{name}}");
  expect(code).toStartWith('import Handlebars from "handlebars/runtime";\nexport default Handlebars.template(');
});

test("compiled module renders with HTML escaping by default", async () => {
  const template = await importCompiled("Hello {{name}} {{who}}");
  expect(template({ name: "Ada", who: "<b>" })).toBe("Hello Ada &lt;b&gt;");
});

test("noEscape passes through to precompile", async () => {
  const template = await importCompiled("Hello {{name}} {{who}}", { noEscape: true });
  expect(template({ name: "Ada", who: "<b>" })).toBe("Hello Ada <b>");
});

test("compile throws on invalid templates", () => {
  expect(() => compile("{{#if a}}unclosed")).toThrow();
});

test("bun plugin loads .hbs imports", async () => {
  Bun.plugin(tsbarsBun({ noEscape: true }));
  const mod = await import("./fixtures/greeting.hbs");
  expect((mod.default as Template)(context)).toBe("Hello Ada & <<b>>\n- x\n- y\n");
});

test("vite plugin transforms .hbs, ignoring query strings and other files", async () => {
  const plugin = tsbarsVite({ noEscape: true });
  expect(plugin.enforce).toBe("pre");
  const transform = plugin.transform as (code: string, id: string) => { code: string };
  expect(transform("hi", "/a/b.hbs?raw").code).toContain("Handlebars.template(");
  expect(transform("hi", "/a/b.ts")).toBeNull();
});

test("vite build bundles a .hbs import", async () => {
  const output = await build({
    root: fixtures,
    logLevel: "silent",
    configFile: false,
    plugins: [tsbarsVite({ noEscape: true })],
    build: {
      write: false,
      minify: false,
      lib: { entry: join(fixtures, "entry.ts"), formats: ["es"], fileName: "out" },
    },
  });
  const outputs = Array.isArray(output) ? output : [output];
  const chunk = (outputs[0] as { output: { type: string; code: string }[] }).output.find((o) => o.type === "chunk");
  expect(chunk?.code).toContain("Hello ");
  expect(chunk?.code).toContain("template");
});
