import { beforeAll, expect, test } from 'bun:test';
import { mkdirSync, rmSync, symlinkSync } from 'node:fs';
import { join, relative } from 'node:path';

const root = join(import.meta.dir, '..');
const tsc = join(root, 'node_modules', '.bin', 'tsc');
const fixtures = join(import.meta.dir, 'e2e');

// tsc does not resolve the `tsbars` content mapper package by self-reference, so each
// fixture gets a (gitignored) node_modules/tsbars link to the repo root.
function linkPackage(fixtureDir: string) {
	const modules = join(fixtureDir, 'node_modules');
	mkdirSync(modules, { recursive: true });
	const link = join(modules, 'tsbars');
	rmSync(link, { force: true });
	symlinkSync(relative(modules, root), link, 'dir');
}

function typecheck(fixtureDir: string) {
	linkPackage(fixtureDir);
	const result = Bun.spawnSync(
		[tsc, '--noEmit', '--runExternalCode', '-p', fixtureDir],
		{
			cwd: root,
		},
	);
	return {
		exitCode: result.exitCode,
		output: result.stdout.toString() + result.stderr.toString(),
	};
}

function fixture(name: string) {
	return typecheck(join(fixtures, name));
}

beforeAll(() => {
	const build = Bun.spawnSync(
		['cargo', 'build', '--release', '-p', 'tsbars-mapper'],
		{
			cwd: root,
			stdout: 'inherit',
			stderr: 'inherit',
		},
	);
	if (build.exitCode !== 0) {
		throw new Error('cargo build --release -p tsbars-mapper failed');
	}
}, 300_000);

test('examples/basic type-checks clean', () => {
	const result = typecheck(join(root, 'examples', 'basic'));
	expect(result.output).toBe('');
	expect(result.exitCode).toBe(0);
});

test('missing property is an error at the call site', () => {
	const result = fixture('missing-property');
	expect(result.exitCode).not.toBe(0);
	expect(result.output).toContain('missing-property/index.ts(4,2)');
	expect(result.output).toContain("Property 'name' is missing");
});

test('object where a Printable leaf is expected is an error', () => {
	const result = fixture('printable-leaf');
	expect(result.exitCode).not.toBe(0);
	expect(result.output).toContain('printable-leaf/index.ts');
	expect(result.output).toContain("'Printable'");
});

test('template parse error is reported against the .hbs file by tsbars', () => {
	const result = fixture('parse-error');
	expect(result.exitCode).not.toBe(0);
	expect(result.output).toContain('parse-error/broken.hbs(2,1)');
	expect(result.output).toContain('tsbars1');
	expect(result.output).toContain('not closed');
	// The fallback module keeps importers from cascading into extra errors.
	expect(result.output).not.toContain('index.ts');
});

test('options.helpers makes a bare helper mustache not required', () => {
	const configured = fixture('helpers');
	expect(configured.output).toBe('');
	expect(configured.exitCode).toBe(0);

	const unconfigured = fixture('helpers-unconfigured');
	expect(unconfigured.exitCode).not.toBe(0);
	expect(unconfigured.output).toContain("Property 'now' is missing");
});

test('@param override type is enforced at the call site', () => {
	const result = fixture('param-override');
	expect(result.exitCode).not.toBe(0);
	expect(result.output).toContain('param-override/index.ts(3,32)');
	expect(result.output).toContain(
		"Type 'string' is not assignable to type 'number'",
	);
});

test('type errors inside an @param annotation land in the template', () => {
	const result = fixture('param-bad-type');
	expect(result.exitCode).not.toBe(0);
	expect(result.output).toContain('param-bad-type/profile.hbs(1,23)');
	expect(result.output).toContain("Cannot find name 'Missing'");
});
