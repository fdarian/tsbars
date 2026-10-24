# tsbars

Import `.hbs` files from TypeScript and get a call signature inferred from the template.

`prompt.hbs`:

```hbs
Hello {{user.name}}!
{{#if user.admin}}You are an admin.{{/if}}
{{#each items}}- {{title}}
{{/each}}
```

`index.ts`:

```ts
import prompt from './prompt.hbs';

prompt({ user: { admin: true }, items: [{ title: 'Write docs' }] });
```

```text
index.ts(3,10): error TS2741: Property 'name' is missing in type '{ admin: true; }' but required in type '{ name: Printable; admin?: unknown; }'.
```

The import is typed as `(context: Context) => string`, where `Context` is:

```ts
type Printable = string | number | bigint | boolean;
type Context = {
	user: { name: Printable; admin?: unknown };
	items: ReadonlyArray<{ title: Printable }>;
};
```

A [TypeScript content mapper](https://github.com/microsoft/typescript-go/pull/4712) does the inference: the compiler hands it the template, it returns a virtual `.ts` module, and errors are mapped back to the template. At runtime the template is compiled by Handlebars through a Vite or Bun plugin.

## Requirements

- TypeScript `7.1.0-beta` (the native compiler), run as `tsc --noEmit --runExternalCode`.
- Imports must include the `.hbs` extension.
- [Handlebars](https://handlebarsjs.com) `^4.7.0` as a peer dependency.
- A Rust toolchain. There are no prebuilt binaries yet, so you build the mapper yourself.

## Install

tsbars is not published to npm yet. From a clone of this repo:

```sh
bun install
bun run build          # runtime plugins -> dist/
bun run build:native   # cargo build --release -p tsbars-mapper
```

Then make the package resolvable as `tsbars` from your project, for example by symlinking the clone into `node_modules/tsbars` (this is what the e2e tests do).

`bin/mapper.js` looks for the mapper binary in `$TSBARS_MAPPER_BIN`, then `native/`, then `target/release/`, then `target/debug/`. If `TSBARS_MAPPER_BIN` is set to a path that doesn't exist, it exits with an error instead of falling back.

## Setup

Add a content mapper to your `tsconfig.json`:

```jsonc
{
	"compilerOptions": {
		"module": "NodeNext",
		"moduleResolution": "NodeNext",
		"strict": true,
		"noEmit": true
	},
	"contentMappers": [
		{
			"package": "tsbars",
			"extensions": [".hbs"],
			"options": { "helpers": ["now"] }
		}
	]
}
```

`options.helpers` lists helpers you call without arguments (`{{now}}`). Without it, a bare `{{now}}` is read as a context variable and becomes a required property.

Type-check with:

```sh
tsc --noEmit --runExternalCode
```

`examples/basic` is a working project.

<details>
<summary>Editor support (not tested by us yet)</summary>

Content mappers are external code, so the editor has to opt in. In VS Code this should mean:
- the TypeScript Native Preview extension,
- a trusted workspace, with the content mapper permission granted,
- `js/ts.contentMappers.enabled` turned on.

We haven't verified this in an editor. Only the `tsc` path above is covered by tests.

</details>

## Runtime

Both plugins take [Handlebars compile options](https://handlebarsjs.com/api-reference/compilation.html) and pass them to `Handlebars.precompile`.

```ts
// vite.config.ts
import tsbars from 'tsbars/vite';

export default {
	plugins: [tsbars({ noEscape: true })],
};
```

```ts
// preload.ts, referenced from bunfig.toml: preload = ["./preload.ts"]
import { plugin } from 'bun';
import tsbars from 'tsbars/bun';

plugin(tsbars({ noEscape: true }));
```

Use `noEscape: true` for LLM prompts. Handlebars HTML-escapes `{{value}}` by default, so `<` and `&` in your data would come out as `&lt;` and `&amp;`.

Compiled templates use `handlebars/runtime`, so register helpers there:

```ts
import Handlebars from 'handlebars/runtime';

Handlebars.registerHelper('shout', (text: string) => text.toUpperCase());
```

The helper must be registered before the template runs. tsbars doesn't see it, so list it in `options.helpers` if you call it with no arguments.

## How types are inferred

| Template                                                 | Inferred                                                                    |
| -------------------------------------------------------- | --------------------------------------------------------------------------- |
| `{{a.b}}`                                                | `a: { b: Printable }`                                                       |
| `{{#if a}}`, `{{#unless a}}`, `{{else if a}}`            | `a?: unknown`, or `a?: <type from other uses>`                              |
| `{{#each xs}}{{title}}{{/each}}`                         | `xs: ReadonlyArray<{ title: Printable }>`                                   |
| `{{#each xs}}{{this}}{{/each}}`                          | `xs: ReadonlyArray<Printable>`                                              |
| `{{#each xs}}{{@key}} {{this}}{{/each}}`                 | `xs: Readonly<Record<string, Printable>>`                                   |
| `{{#each xs as \|x i\|}}{{x.id}}{{/each}}`               | `xs: ReadonlyArray<{ id: Printable }>` (first block param names the item)   |
| `{{#with a}}{{b}}{{/with}}`                              | `a: { b: Printable }`; optional if the block has `{{else}}`                 |
| `{{../b}}` inside a block                                | `b` is looked up in the parent scope, one level per `../`                   |
| `{{@root.b}}`                                            | `b` on the root context                                                     |
| `{{shout name}}`, `{{upper (trim raw)}}`                 | `name: unknown`, `raw: unknown`; the helper itself is not part of `Context` |
| `{{lookup obj key}}`                                     | `obj: unknown`, `key: Printable`                                            |
| `{{#fancy z}}{{inner}}{{/fancy}}` (unknown block helper) | `z: unknown`; the body adds nothing                                         |
| `{{now}}` with `"helpers": ["now"]`                      | nothing                                                                     |
| no variables at all                                      | `context?: Record<string, never>`                                           |

Notes:
- `Printable` is `string | number | bigint | boolean`. A path that is both printed and used as an object (`{{a}}` and `{{a.b}}`) becomes the object.
- Property names that aren't valid identifiers (`{{[odd key]}}`) become quoted keys.
- Properties are listed in the order the template first mentions them.

### Overriding inference

Top-level comments can replace an inferred type or import one:

```hbs
{{!-- @import type { User } from "./user" --}}
{{!-- @param user User --}}
{{!-- @param stats.count number --}}
Hello {{user.name}}, you have {{stats.count}} messages.
```

- `@param <path> <type>` replaces the inferred type at that dotted path with any TypeScript type.
- `@import <clause>` emits `import <clause>;` into the generated module. The clause is whatever follows `import`, for example `type { User } from "./user"`.
- A type error inside an annotation is reported on the template line (`profile.hbs(1,23): error TS2304: Cannot find name 'Missing'.`).
- A malformed directive is reported as `tsbars2` against the template.

## Limitations

- Partials contribute no types: `{{> name}}` and `{{#> name}}...{{/name}}` are skipped, including everything inside a partial block. Variables used only there won't appear in `Context`.
- Decorators (`{{* ... }}`) and inline partials (`{{#*inline "x"}}`) are not supported. They are parse errors.
- Helpers have no signatures. Their arguments are typed `unknown`, and nothing checks the helper exists.
- A template that fails to parse is reported as `tsbars1` (for example `broken.hbs(2,1): error tsbars1: Block \`if\` is not closed`). Its default export then accepts `any`, so importers don't produce more errors.
- Content mappers are a beta TypeScript feature and may change.
- No prebuilt binaries, no npm release, and editor support is untested.

## Acknowledgements

tsbars is modelled on [reylo](https://github.com/colinhacks/reylo) by Colin McDonnell (MIT). Its content-mapper server, packaging and typed-import design were the reference. tsbars reimplements that approach in Rust for Handlebars.

Thanks to the TypeScript team for content mappers ([microsoft/typescript-go#4712](https://github.com/microsoft/typescript-go/pull/4712)).

## License

[MIT](LICENSE)
