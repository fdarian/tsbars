import Handlebars from "handlebars";

export type CompileOptions = Parameters<typeof Handlebars.precompile>[1];

export function compile(source: string, options?: CompileOptions): string {
  const spec = Handlebars.precompile(source, options);
  return `import Handlebars from "handlebars/runtime";\nexport default Handlebars.template(${spec});\n`;
}
