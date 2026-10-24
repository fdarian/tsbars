import type { BunPlugin } from 'bun';
import { type CompileOptions, compile } from '#src/compile.ts';

export default function tsbars(options?: CompileOptions): BunPlugin {
	return {
		name: 'tsbars',
		setup(build) {
			build.onLoad({ filter: /\.hbs$/ }, async (args) => {
				const source = await Bun.file(args.path).text();
				return { contents: compile(source, options), loader: 'js' };
			});
		},
	};
}
