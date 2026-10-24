const DURATION = 10;
const CROSSFADE_START = 9.4;
const POINTER_REST = { x: 1120, y: 395 };

const MAIN_WIN = { left: 620, top: 78, width: 570, height: 364 };
const HBS_WIN = { left: 1220, top: 144, width: 460, height: 232 };

const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
const ramp = (t, a, b) => clamp((t - a) / (b - a), 0, 1);
const easeInOut = (x) => (x < 0.5 ? 4 * x * x * x : 1 - (-2 * x + 2) ** 3 / 2);

// Deterministic per-keystroke delays: ~80 ms with a little jitter.
function keySchedule(count, seed) {
	const times = [];
	let acc = 0;
	for (let i = 0; i < count; i++) {
		const x = Math.sin((seed + i) * 127.1) * 43758.5453;
		const jitter = x - Math.floor(x) - 0.5;
		acc += 0.08 + 0.034 * jitter;
		times.push(acc);
	}
	return times;
}

const HBS_INSERT = ', {{user.role}}';
const MAIN_PREFIX_INSERT = ', r';
const MAIN_SUFFIX_INSERT = ': "admin"';

const HBS_TYPE_START = 2.2;
const MAIN_TYPE_START = 6.15;
const MAIN_ACCEPT = 6.9;
const MAIN_SUFFIX_START = 7.0;

const hbsKeys = keySchedule(HBS_INSERT.length, 1);
const mainKeysA = keySchedule(MAIN_PREFIX_INSERT.length, 40);
const mainKeysB = keySchedule(MAIN_SUFFIX_INSERT.length, 70);

function typedCount(keys, elapsed) {
	let n = 0;
	while (n < keys.length && keys[n] <= elapsed) n++;
	return n;
}

function lastEvent(t, events) {
	let last = events[0];
	for (const e of events) if (e <= t && e > last) last = e;
	return last;
}

function blinkOn(t, last) {
	return (t - last) % 1.06 < 0.53;
}

function escapeHtml(s) {
	return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

const TS_KEYWORDS = new Set(['import', 'from', 'const']);

function tokenizeTs(line) {
	const re = /"(?:[^"\\]|\\.)*"?|[A-Za-z_$][\w$]*|\s+|./gy;
	const tokens = [];
	let prevWord = '';
	for (let m = re.exec(line); m !== null; m = re.exec(line)) {
		const text = m[0];
		let cls;
		if (text[0] === '"') cls = prevWord === 'from' ? 'pa' : 's';
		else if (/^[A-Za-z_$]/.test(text)) cls = TS_KEYWORDS.has(text) ? 'k' : 'i';
		else if (/^\s+$/.test(text)) cls = '';
		else cls = 'p';
		if (!/^\s+$/.test(text)) prevWord = text;
		tokens.push({ cls: cls, text: text });
	}
	return tokens;
}

function tokenizeHbs(line) {
	const tokens = [];
	let i = 0;
	let inside = false;
	while (i < line.length) {
		if (!inside) {
			if (line.startsWith('{{', i)) {
				tokens.push({ cls: 'd', text: '{{' });
				i += 2;
				inside = true;
			} else {
				let j = line.indexOf('{{', i);
				if (j < 0) j = line.length;
				tokens.push({ cls: 't', text: line.slice(i, j) });
				i = j;
			}
		} else if (line.startsWith('}}', i)) {
			tokens.push({ cls: 'd', text: '}}' });
			i += 2;
			inside = false;
		} else {
			const m = /^(?:[#/]\w+|[\w.@]+|\s+|.)/.exec(line.slice(i));
			const text = m[0];
			let cls = 'i';
			if (/^[#/]/.test(text) || text === '}') cls = 'd';
			else if (/^\s+$/.test(text)) cls = '';
			tokens.push({ cls: cls, text: text });
			i += text.length;
		}
	}
	return tokens;
}

function span(cls, text) {
	const body = escapeHtml(text);
	return cls ? `<span class="${cls}">${body}</span>` : body;
}

const CARET = '<span class="caret"></span>';

// `caret` is a character index into the line or -1. `keyAt` is the offset of the identifier
// rendered as the error anchor (`user`).
function renderLine(tokens, caret, keyAt) {
	let html = '';
	let offset = 0;
	let caretDone = caret < 0;
	for (const tok of tokens) {
		const end = offset + tok.text.length;
		const text = tok.text;
		if (!caretDone && caret >= offset && caret < end) {
			const cut = caret - offset;
			html += cut > 0 ? span(tok.cls, text.slice(0, cut)) : '';
			html += CARET;
			html += span(tok.cls, text.slice(cut));
			caretDone = true;
		} else if (keyAt === offset) {
			html += `<span class="uk ${tok.cls}">${escapeHtml(text)}<i class="squig"></i></span>`;
		} else {
			html += span(tok.cls, text);
		}
		offset = end;
	}
	if (!caretDone) html += CARET;
	return `<div class="line">${html}</div>`;
}

const PROPERTY_ICON = `<svg viewBox="0 0 16 16"><path fill="#75beff" d="M13.9 3.1l-2.3 2.3-1.1-1.1 2.3-2.3A3.7 3.7 0 0 0 8.1 6.7c0 .4.1.8.2 1.1l-5.7 5.7a1.2 1.2 0 0 0 1.7 1.7l5.7-5.7c.4.2.8.2 1.2.2a3.7 3.7 0 0 0 3.5-4.8z"/></svg>`;
const POINTER = `<svg width="28" height="36" viewBox="0 0 28 36"><path d="M2 2 L2 27 L8.5 21.5 L13 31.5 L18 29.3 L13.6 19.5 L22 19.5 Z" fill="#000" stroke="#fff" stroke-width="2" stroke-linejoin="round"/></svg>`;

const pos = (w) =>
	`left:${w.left}px;top:${w.top}px;width:${w.width}px;height:${w.height}px`;

function createScene(root) {
	root.innerHTML = `
    <div class="scene">
      <div class="win main" style="${pos(MAIN_WIN)}">
        <div class="bar"><div class="lights"><i></i><i></i><i></i></div><div class="title">main.ts</div></div>
        <pre class="code"></pre>
        <div class="tip tip-type">
          <div class="row"><span class="p">(property)</span> <span class="i">user</span><span class="p">: {</span></div>
          <div class="row"><span class="i">    name</span><span class="p">: </span><span style="color:var(--teal)">Printable</span><span class="p">;</span></div>
          <div class="row new"><span class="i">    role</span><span class="p">: </span><span style="color:var(--teal)">Printable</span><span class="p">;</span></div>
          <div class="row"><span class="p">}</span></div>
        </div>
        <div class="tip tip-err">
          <div class="msg">Property <code>'role'</code> is missing in type <code>'{ name: string; }'</code> but required in type <code>'{ name: Printable; role: Printable; }'</code>. <span class="src">ts(2741)</span></div>
          <div class="foot"><b>View Problem</b><span>No quick fixes available</span></div>
        </div>
        <div class="popup"><div class="item">${PROPERTY_ICON}<span><span class="m">r</span>ole</span><span class="detail">Printable</span></div></div>
      </div>
      <div class="win hbs" style="${pos(HBS_WIN)}">
        <div class="bar"><div class="lights"><i></i><i></i><i></i></div><div class="title">prompt.hbs</div></div>
        <pre class="code"></pre>
      </div>
      <div class="pointer">${POINTER}</div>
    </div>`;

	const q = (sel) => root.querySelector(sel);
	const mainWin = q('.win.main');
	const hbsWin = q('.win.hbs');
	const mainCode = mainWin.querySelector('.code');
	const hbsCode = hbsWin.querySelector('.code');
	const tipType = q('.tip-type');
	const newRow = q('.tip-type .row.new');
	const tipErr = q('.tip-err');
	const popup = q('.popup');
	const pointer = q('.pointer');

	function pointerAt(t, userRect) {
		const u1 = {
			x: userRect.left + userRect.width * 0.3,
			y: userRect.top + userRect.height * 0.55,
		};
		const u2 = {
			x: userRect.left + userRect.width * 0.72,
			y: userRect.bottom - 5,
		};
		const track = [
			{ t: 0, p: POINTER_REST },
			{ t: 0.8, p: POINTER_REST },
			{ t: 1.6, p: u1 },
			{ t: 4.2, p: u1 },
			{ t: 4.9, p: u2 },
			{ t: 5.6, p: u2 },
			{ t: 6.1, p: POINTER_REST },
		];
		if (t >= track[track.length - 1].t) return POINTER_REST;
		let i = 0;
		while (track[i + 1].t <= t) i++;
		const a = track[i];
		const b = track[i + 1];
		const e = easeInOut(ramp(t, a.t, b.t));
		const dx = b.p.x - a.p.x;
		const dy = b.p.y - a.p.y;
		const dist = Math.hypot(dx, dy) || 1;
		// Slight arc so the glide doesn't read as a straight lerp.
		const bow = Math.min(dist * 0.12, 26) * Math.sin(Math.PI * e);
		return {
			x: a.p.x + dx * e + (-dy / dist) * bow,
			y: a.p.y + dy * e + (dx / dist) * bow,
		};
	}

	return {
		render(t, opacity) {
			root.style.opacity = String(opacity);

			// main.ts
			const nPre =
				t >= MAIN_TYPE_START ? typedCount(mainKeysA, t - MAIN_TYPE_START) : 0;
			const accepted = t >= MAIN_ACCEPT;
			let inserted = MAIN_PREFIX_INSERT.slice(0, nPre);
			const mainEvents = [
				MAIN_TYPE_START - 0.15,
				...mainKeysA.map((k) => MAIN_TYPE_START + k),
			];
			if (accepted) {
				const nSuf =
					t >= MAIN_SUFFIX_START
						? typedCount(mainKeysB, t - MAIN_SUFFIX_START)
						: 0;
				inserted = ', role' + MAIN_SUFFIX_INSERT.slice(0, nSuf);
				mainEvents.push(
					MAIN_ACCEPT,
					...mainKeysB.map((k) => MAIN_SUFFIX_START + k),
				);
			}
			const mainCaretVisible = t >= MAIN_TYPE_START - 0.15;
			const before = '  user: { name: "Ada"';
			const line4 = before + inserted + ' },';
			const caretIdx =
				mainCaretVisible && blinkOn(t, lastEvent(t, mainEvents))
					? before.length + inserted.length
					: -1;
			const mainLines = [
				'import prompt from "./prompt.hbs";',
				'',
				'const text = prompt({',
				line4,
				'  items: [{ title: "Ship tsbars" }],',
				'});',
			];
			mainCode.innerHTML = mainLines
				.map((l, idx) =>
					renderLine(
						tokenizeTs(l),
						idx === 3 ? caretIdx : -1,
						idx === 3 ? 2 : -1,
					),
				)
				.join('');

			// prompt.hbs
			const hbsCaretVisible = t >= 2.0 && t < MAIN_TYPE_START - 0.15;
			const nHbs =
				t >= HBS_TYPE_START ? typedCount(hbsKeys, t - HBS_TYPE_START) : 0;
			const hbsPre = 'Hello {{user.name}}';
			const hbsIns = HBS_INSERT.slice(0, nHbs);
			const hbsEvents = [2.0, ...hbsKeys.map((k) => HBS_TYPE_START + k)];
			const hbsCaret =
				hbsCaretVisible && blinkOn(t, lastEvent(t, hbsEvents))
					? hbsPre.length + hbsIns.length
					: -1;
			const hbsLines = [
				hbsPre + hbsIns + '!',
				'{{#each items}}',
				'- {{title}}',
				'{{/each}}',
			];
			hbsCode.innerHTML = hbsLines
				.map((l, idx) =>
					renderLine(tokenizeHbs(l), idx === 0 ? hbsCaret : -1, -1),
				)
				.join('');

			// window focus
			const hbsActive =
				ramp(t, 2.0, 2.15) *
				(1 - ramp(t, MAIN_TYPE_START - 0.15, MAIN_TYPE_START));
			hbsWin.style.setProperty('--active', String(hbsActive));
			mainWin.style.setProperty('--active', String(1 - hbsActive));
			hbsWin.querySelector('.title').style.color =
				hbsActive > 0.5 ? '#c9c9c9' : '#6a6a6a';
			mainWin.querySelector('.title').style.color =
				hbsActive > 0.5 ? '#6a6a6a' : '#c9c9c9';

			// squiggle + anchors
			const userEl = mainCode.querySelector('.uk');
			const squig = mainCode.querySelector('.squig');
			squig.style.opacity = String(
				ramp(t, 3.6, 3.8) * (1 - ramp(t, 7.8, 8.05)),
			);
			const userRect = userEl.getBoundingClientRect();
			const tipLeft = userRect.left - MAIN_WIN.left - 6;
			const tipTop = userRect.bottom - MAIN_WIN.top + 10;

			// hover tooltips
			const typeOpacity = ramp(t, 1.6, 1.72) * (1 - ramp(t, 4.9, 5.05));
			tipType.style.cssText = `left:${tipLeft}px;top:${tipTop}px;opacity:${typeOpacity};transform:translateY(${(1 - typeOpacity) * 4}px)`;
			const roleShown = t >= 3.6;
			newRow.style.display = roleShown ? '' : 'none';
			newRow.style.setProperty(
				'--glow',
				String(roleShown ? 0.32 * (1 - ramp(t, 3.6, 4.6)) : 0),
			);

			const errOpacity = ramp(t, 4.9, 5.1) * (1 - ramp(t, 5.85, 6.0));
			tipErr.style.cssText = `left:${tipLeft}px;top:${tipTop}px;opacity:${errOpacity};transform:translateY(${(1 - errOpacity) * 4}px)`;

			// completion popup
			const popupOpacity =
				ramp(t, 6.45, 6.57) * (1 - ramp(t, MAIN_ACCEPT, MAIN_ACCEPT + 0.06));
			popup.style.opacity = String(popupOpacity);
			if (popupOpacity > 0) {
				const caretRect = mainCode
					.querySelector('.caret')
					.getBoundingClientRect();
				popup.style.left = `${caretRect.left - MAIN_WIN.left - 12 - 28}px`;
				popup.style.top = `${caretRect.bottom - MAIN_WIN.top + 8}px`;
			}

			const p = pointerAt(t, userRect);
			pointer.style.transform = `translate(${p.x - 2}px, ${p.y - 2}px)`;
		},
	};
}

const sceneA = createScene(document.getElementById('sceneA'));
const sceneB = createScene(document.getElementById('sceneB'));

window.seek = function seek(time) {
	const t = ((time % DURATION) + DURATION) % DURATION;
	if (t < CROSSFADE_START) {
		sceneA.render(t, 1);
		sceneB.render(0, 0);
	} else {
		// The outgoing final state fades over the initial state, which makes the loop seamless.
		sceneA.render(0, 1);
		sceneB.render(CROSSFADE_START, 1 - ramp(t, CROSSFADE_START, DURATION));
	}
};

window.seek(0);
