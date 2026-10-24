#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageDir = join(dirname(fileURLToPath(import.meta.url)), '..');
const binaryName =
	process.platform === 'win32' ? 'tsbars-mapper.exe' : 'tsbars-mapper';

const override = process.env.TSBARS_MAPPER_BIN;
if (override !== undefined && override !== '' && !existsSync(override)) {
	process.stderr.write(
		`tsbars: TSBARS_MAPPER_BIN is set but does not exist: ${override}\n`,
	);
	process.exit(1);
}

const candidates = [
	override,
	join(packageDir, 'native', binaryName),
	join(packageDir, 'target', 'release', binaryName),
	join(packageDir, 'target', 'debug', binaryName),
].filter(Boolean);

const binary = candidates.find((candidate) => existsSync(candidate));

if (binary === undefined) {
	process.stderr.write(
		`tsbars: native mapper binary not found. Looked in:\n${candidates.map((candidate) => `  ${candidate}`).join('\n')}\n` +
			'Run `cargo build --release -p tsbars-mapper` (or `bun run build:native`) in the tsbars repo, ' +
			'or set TSBARS_MAPPER_BIN to the binary path.\n',
	);
	process.exit(1);
}

const child = spawn(binary, process.argv.slice(2), { stdio: 'inherit' });

child.on('error', (error) => {
	process.stderr.write(`tsbars: failed to start ${binary}: ${error.message}\n`);
	process.exit(1);
});

child.on('exit', (code, signal) => {
	if (signal !== null) {
		process.kill(process.pid, signal);
		return;
	}
	process.exit(code ?? 1);
});
