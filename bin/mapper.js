#!/usr/bin/env node
import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const wasmPath = join(
	dirname(fileURLToPath(import.meta.url)),
	'..',
	'wasm',
	'tsbars-mapper.wasm',
);

// tsc discards our stderr, but keep it clean for direct runs. Must precede the node:wasi import.
process.removeAllListeners('warning');
const { WASI } = await import('node:wasi');

let bytes;
try {
	bytes = await readFile(wasmPath);
} catch (error) {
	if (error.code !== 'ENOENT') throw error;
	process.stderr.write(
		`tsbars: mapper not found at ${wasmPath}\nRun \`pnpm run build:wasm\` in the tsbars repo.\n`,
	);
	process.exit(1);
}

// Fds 0/1/2 are passed straight through; process.stdin is deliberately never touched so
// the WASI blocking read keeps working on the inherited pipe.
const wasi = new WASI({
	version: 'preview1',
	args: ['tsbars-mapper'],
	env: {},
	stdin: 0,
	stdout: 1,
	stderr: 2,
	returnOnExit: true,
});
const instance = await WebAssembly.instantiate(
	await WebAssembly.compile(bytes),
	wasi.getImportObject(),
);
process.exit(wasi.start(instance));
