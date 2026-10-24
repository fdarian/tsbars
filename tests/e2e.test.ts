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
	const build = Bun.spawnSync(['pnpm', 'run', 'build:wasm'], {
		cwd: root,
		stdout: 'inherit',
		stderr: 'inherit',
	});
	if (build.exitCode !== 0) {
		throw new Error('pnpm run build:wasm failed');
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

function frame(message: object) {
	const body = JSON.stringify(message);
	return `Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`;
}

interface Rpc {
	id: number;
	result: Record<string, unknown> & { text: string; diagnosticSource: string };
}

function parseFrames(output: string) {
	const messages: Rpc[] = [];
	let rest = Buffer.from(output);
	while (rest.length > 0) {
		const headerEnd = rest.indexOf('\r\n\r\n');
		const header = rest.subarray(0, headerEnd).toString();
		const length = Number(/Content-Length: (\d+)/.exec(header)?.[1]);
		const bodyStart = headerEnd + 4;
		messages.push(
			JSON.parse(rest.subarray(bodyStart, bodyStart + length).toString()),
		);
		rest = rest.subarray(bodyStart + length);
	}
	return messages;
}

test('bin/mapper.js serves framed JSON-RPC over piped stdio', async () => {
	const child = Bun.spawn(['node', join(root, 'bin', 'mapper.js')], {
		stdin: 'pipe',
		stdout: 'pipe',
		stderr: 'pipe',
	});
	const sleep = () => new Promise((resolve) => setTimeout(resolve, 50));

	const initialize = frame({
		jsonrpc: '2.0',
		id: 1,
		method: 'initialize',
		params: { positionEncodings: ['utf-8'] },
	});
	// One message split mid-header, mid-body across separate writes.
	const cut = initialize.indexOf('Length') + 3;
	const cutBody = initialize.length - 5;
	child.stdin.write(initialize.slice(0, cut));
	child.stdin.flush();
	await sleep();
	child.stdin.write(initialize.slice(cut, cutBody));
	child.stdin.flush();
	await sleep();
	child.stdin.write(initialize.slice(cutBody));
	child.stdin.flush();
	await sleep();

	// Two messages in a single write.
	child.stdin.write(
		frame({
			jsonrpc: '2.0',
			id: 2,
			method: 'openProject',
			params: {
				projectHandle: 'h',
				options: { helpers: ['now'] },
				compilerOptions: {},
			},
		}) +
			frame({
				jsonrpc: '2.0',
				id: 3,
				method: 'transform',
				params: {
					fileName: '/p/a.hbs',
					content: 'Hi {{name}} {{now}}',
					projectHandle: 'h',
				},
			}),
	);
	child.stdin.flush();
	await sleep();
	child.stdin.write(
		frame({
			jsonrpc: '2.0',
			id: 4,
			method: 'closeProject',
			params: { projectHandle: 'h' },
		}),
	);
	child.stdin.end();

	const [stdout, stderr, exitCode] = await Promise.all([
		new Response(child.stdout).text(),
		new Response(child.stderr).text(),
		child.exited,
	]);
	expect(stderr).toBe('');
	expect(exitCode).toBe(0);

	const responses = parseFrames(stdout);
	expect(responses.map((response) => response.id)).toEqual([1, 2, 3, 4]);
	expect(responses[0].result.diagnosticSource).toBe('tsbars');
	expect(responses[1].result).toEqual({});
	expect(responses[2].result.text).toContain('name: Printable');
	expect(responses[2].result.text).not.toContain('now');
	expect(responses[3].result).toEqual({});
});
