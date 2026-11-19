const { L, icon, pick, shell, onChange } = FE;

const VARIANTS = [
	["cap", "Keycaps", "键帽"],
	["line", "Outline", "线框"],
	["ghost", "Ghost", "幽灵"],
];
const PRESETS = [
	["wasd", "WASD + mouse", "WASD 加鼠标", 30],
	["full", "Full keyboard", "整块键盘", 52],
	["pad", "Gamepad", "手柄", 24],
	["mine", "My preset", "我的预设", 30],
];
const STOPS = [0, 0.5, 1];
const MARGIN = 0.03;

const s = {
	variant: "cap",
	preset: "wasd",
	manual: false,
	used: "keys",
	pad: "xbox",
	demo: "auto",
	scene: "dark",
	on: true,
	x: 0,
	y: 1,
	xy: false,
	size: 60,
	opacity: 100,
	info: false,
	record: true,
	instant: false,
	instantDevice: "auto",
};

const live = new Set();
const flashes = new Map();

const U = 40;
const GAP = 4;
const key = (x, y, w, label, code) => {
	const width = w * U - GAP;
	const height = U - GAP;
	const small = label.length > 1 ? " small" : "";
	return `<g class="k${small}" data-code="${code}"><rect x="${x}" y="${y}" width="${width}" height="${height}" rx="6"/><text x="${x + width / 2}" y="${y + height / 2}">${label}</text></g>`;
};
const rows = (layout, x0, y0) =>
	layout
		.map((row, r) => {
			let x = x0;
			return row
				.map(([label, code, w = 1]) => {
					const out = key(x, y0 + r * U, w, label, code);
					x += w * U;
					return out;
				})
				.join("");
		})
		.join("");
const rowWidth = (layout) =>
	Math.max(...layout.map((row) => row.reduce((n, [, , w = 1]) => n + w, 0))) *
	U;

const MOUSE_PARTS = [
	["left", "Left button", "左键"],
	["right", "Right button", "右键"],
	["wheel", "Wheel", "滚轮"],
	["side", "Side buttons", "侧键"],
];
const PAD_PARTS = [
	["sticks", "Sticks", "摇杆"],
	["dpad", "D-pad", "方向键"],
	["face", "Face buttons", "功能键"],
	["shoulders", "Shoulders", "肩键"],
	["triggers", "Triggers", "扳机"],
	["menu", "Menu buttons", "菜单键"],
];
const every = (parts) => Object.fromEntries(parts.map(([id]) => [id, true]));
const mouse = (x, y, parts = every(MOUSE_PARTS)) => `
	<rect class="body" x="${x}" y="${y}" width="96" height="150" rx="44"/>
	${parts.left ? `<g class="k" data-code="M0"><path d="M${x + 4} ${y + 66} V${y + 46} A42 42 0 0 1 ${x + 45} ${y + 4} V${y + 66} Z"/></g>` : ""}
	${parts.right ? `<g class="k" data-code="M2"><path d="M${x + 92} ${y + 66} V${y + 46} A42 42 0 0 0 ${x + 51} ${y + 4} V${y + 66} Z"/></g>` : ""}
	${
		parts.wheel
			? `<g class="k" data-code="WheelUp M1"><path d="M${x + 41} ${y + 33} V${y + 25} a7 7 0 0 1 14 0 V${y + 33} Z"/></g>
	<g class="k" data-code="WheelDown M1"><path d="M${x + 41} ${y + 35} V${y + 43} a7 7 0 0 0 14 0 V${y + 35} Z"/></g>`
			: ""
	}
	${
		parts.side
			? `<g class="k" data-code="M3"><rect x="${x - 4}" y="${y + 74}" width="7" height="18" rx="3"/></g>
	<g class="k" data-code="M4"><rect x="${x - 4}" y="${y + 96}" width="7" height="18" rx="3"/></g>`
			: ""
	}`;
const WASD = [
	[
		["Tab", "Tab", 1.5],
		["Q", "KeyQ"],
		["W", "KeyW"],
		["E", "KeyE"],
		["R", "KeyR"],
	],
	[
		["Shift", "ShiftLeft", 1.5],
		["A", "KeyA"],
		["S", "KeyS"],
		["D", "KeyD"],
		["F", "KeyF"],
	],
	[
		["Ctrl", "ControlLeft", 1.5],
		["Space", "Space", 4],
	],
];
const letters = (text) => [...text].map((c) => [c, `Key${c}`]);
const FULL = [
	[
		["`", "Backquote"],
		...[..."1234567890"].map((c) => [c, `Digit${c}`]),
		["-", "Minus"],
		["=", "Equal"],
		["Back", "Backspace", 2],
	],
	[
		["Tab", "Tab", 1.5],
		...letters("QWERTYUIOP"),
		["[", "BracketLeft"],
		["]", "BracketRight"],
		["\\", "Backslash", 1.5],
	],
	[
		["Caps", "CapsLock", 1.75],
		...letters("ASDFGHJKL"),
		[";", "Semicolon"],
		["'", "Quote"],
		["Enter", "Enter", 2.25],
	],
	[
		["Shift", "ShiftLeft", 2.25],
		...letters("ZXCVBNM"),
		[",", "Comma"],
		[".", "Period"],
		["/", "Slash"],
		["Shift", "ShiftRight", 2.75],
	],
	[
		["Ctrl", "ControlLeft", 1.25],
		["Win", "MetaLeft", 1.25],
		["Alt", "AltLeft", 1.25],
		["", "Space", 6.25],
		["Alt", "AltRight", 1.25],
		["Win", "MetaRight", 1.25],
		["Menu", "ContextMenu", 1.25],
		["Ctrl", "ControlRight", 1.25],
	],
];

const keysWithMouse = (layout) => {
	const width = rowWidth(layout);
	const height = layout.length * U;
	const mouseY = Math.max(0, (height - 150) / 2);
	const total = Math.max(height, 150);
	return {
		view: [width + 24 + 100, total],
		body: `${rows(layout, 0, (total - height) / 2)}${mouse(width + 24, mouseY)}`,
	};
};

const FACE = {
	xbox: ["A", "B", "X", "Y"],
	ps: ["✕", "○", "□", "△"],
	switch: ["B", "A", "Y", "X"],
};
const round = (cx, cy, r, label, code) =>
	`<g class="k${label.length > 1 ? " small" : ""}" data-code="${code}"><circle cx="${cx}" cy="${cy}" r="${r}"/><text x="${cx}" y="${cy}">${label}</text></g>`;
const stick = (cx, cy, side, code) =>
	`<circle class="well" cx="${cx}" cy="${cy}" r="25"/><g class="k" data-code="${code}" data-stick="${side}"><circle cx="${cx}" cy="${cy}" r="15"/></g>`;
const dpad = (cx, cy) =>
	[
		["GpUp", cx - 7, cy - 23, 14, 16],
		["GpDown", cx - 7, cy + 7, 14, 16],
		["GpLeft", cx - 23, cy - 7, 16, 14],
		["GpRight", cx + 7, cy - 7, 16, 14],
	]
		.map(
			([code, x, y, w, h]) =>
				`<g class="k" data-code="${code}"><rect x="${x}" y="${y}" width="${w}" height="${h}" rx="3"/></g>`,
		)
		.join("");
const trigger = (x, axis, label) =>
	`<g class="k small"><rect x="${x}" y="4" width="46" height="20" rx="6"/><text x="${x + 23}" y="14">${label}</text></g><clipPath id="clip-${axis}-${++clipCount}"><rect x="${x}" y="4" width="46" height="20" rx="6"/></clipPath><rect class="trig-fill" data-axis="${axis}" clip-path="url(#clip-${axis}-${clipCount})" x="${x}" y="4" width="0" height="20"/>`;
let clipCount = 0;
const pad = (kind, parts = every(PAD_PARTS)) => {
	const [bottom, right, left, top] = FACE[kind];
	const symmetric = kind === "ps";
	const leftStick = symmetric ? [165, 152] : [120, 108];
	const cross = symmetric ? [120, 108] : [165, 152];
	const shoulders =
		kind === "ps"
			? ["L1", "R1", "L2", "R2"]
			: kind === "switch"
				? ["L", "R", "ZL", "ZR"]
				: ["LB", "RB", "LT", "RT"];
	return {
		view: [420, 262],
		body: `
			${parts.triggers ? `${trigger(90, "lt", shoulders[2])}${trigger(284, "rt", shoulders[3])}` : ""}
			${
				parts.shoulders
					? `<g class="k small" data-code="GpLB"><rect x="76" y="30" width="74" height="18" rx="9"/><text x="113" y="39">${shoulders[0]}</text></g>
			<g class="k small" data-code="GpRB"><rect x="270" y="30" width="74" height="18" rx="9"/><text x="307" y="39">${shoulders[1]}</text></g>`
					: ""
			}
			<path class="body" d="M90 62 Q210 44 330 62 Q372 70 388 152 Q400 217 372 238 Q345 252 318 207 Q300 180 270 180 L150 180 Q120 180 102 207 Q75 252 48 238 Q20 217 32 152 Q48 70 90 62 Z"/>
			${parts.sticks ? stick(leftStick[0], leftStick[1], "l", "GpL3") : ""}
			${parts.dpad ? dpad(cross[0], cross[1]) : ""}
			${parts.sticks ? stick(255, 152, "r", "GpR3") : ""}
			${parts.face ? `${round(300, 130, 11, bottom, "GpA")}${round(322, 108, 11, right, "GpB")}${round(278, 108, 11, left, "GpX")}${round(300, 86, 11, top, "GpY")}` : ""}
			${parts.menu ? `${round(185, 108, 7, "", "GpView")}${round(235, 108, 7, "", "GpMenu")}${round(210, 84, 9, "", "GpGuide")}` : ""}`,
	};
};

const COLS = 18;
const ROWS = 5;
const WIDTHS = [1, 1.5, 2, 4];
const NAMES = {
	Space: "Space",
	ShiftLeft: "Shift",
	ShiftRight: "Shift",
	ControlLeft: "Ctrl",
	ControlRight: "Ctrl",
	AltLeft: "Alt",
	AltRight: "Alt",
	MetaLeft: "Win",
	MetaRight: "Win",
	Escape: "Esc",
	Backspace: "Back",
	CapsLock: "Caps",
	ArrowUp: "↑",
	ArrowDown: "↓",
	ArrowLeft: "←",
	ArrowRight: "→",
};
const labelFor = (code) =>
	NAMES[code] ??
	(code.startsWith("Key")
		? code.slice(3)
		: code.startsWith("Digit")
			? code.slice(5)
			: code);
const MOUSE_W = 2.5;
const MOUSE_H = 3.75;
const STARTS = [
	["wasd", "WASD + mouse", "WASD + 鼠标"],
	["full", "Full keyboard", "全键盘"],
	["pad", "Gamepad", "手柄"],
	["empty", "Empty", "空白"],
];
const seed = (layout) =>
	layout.flatMap((row, y) => {
		let x = 0;
		return row.map(([label, code, w = 1]) => {
			const placed = { label, code, x, y, w };
			x += w;
			return placed;
		});
	});
const startFrom = (from) => {
	const layout = from === "full" ? FULL : from === "wasd" ? WASD : [];
	const keys = seed(layout);
	const right = Math.max(0, ...keys.map((k) => k.x + k.w));
	return {
		from,
		kind: from === "pad" ? "pad" : "keys",
		keys,
		sel: -1,
		capture: from === "empty",
		mouse: {
			on: from !== "empty",
			x: Math.min(COLS - MOUSE_W, right + 0.5),
			y: 0.5,
			parts: every(MOUSE_PARTS),
		},
		pad: every(PAD_PARTS),
	};
};
let m = startFrom("wasd");
const mine = () => {
	if (m.kind === "pad") return pad(s.pad, m.pad);
	const blocks = m.keys.map((k) => [k.x, k.y, k.w, 1]);
	if (m.mouse.on) blocks.push([m.mouse.x, m.mouse.y, MOUSE_W, MOUSE_H]);
	if (blocks.length === 0) return { view: [U, U], body: "" };
	const left = Math.min(...blocks.map(([x]) => x));
	const top = Math.min(...blocks.map(([, y]) => y));
	const keys = m.keys
		.map((k) => key((k.x - left) * U, (k.y - top) * U, k.w, k.label, k.code))
		.join("");
	return {
		view: [
			Math.max(...blocks.map(([x, , w]) => x + w - left)) * U,
			Math.max(...blocks.map(([, y, , h]) => y + h - top)) * U,
		],
		body: `${keys}${m.mouse.on ? mouse((m.mouse.x - left) * U + 2, (m.mouse.y - top) * U, m.mouse.parts) : ""}`,
	};
};
const freeSlot = () => {
	for (let y = 0; y < ROWS; y++)
		for (let x = 0; x < COLS; x++)
			if (!m.keys.some((k) => k.y === y && x < k.x + k.w && x + 1 > k.x))
				return { x, y };
	return { x: 0, y: 0 };
};

const overlay = (preset) => {
	const { view, body } =
		preset === "pad"
			? pad(s.pad)
			: preset === "mine"
				? mine()
				: keysWithMouse(preset === "full" ? FULL : WASD);
	return `<svg class="io ${s.variant}" viewBox="-6 -2 ${view[0] + 12} ${view[1] + 4}" aria-hidden="true">${body}</svg>`;
};

const KEY_DEMO = [
	["KeyW"],
	["KeyW", "ShiftLeft"],
	["KeyW", "KeyD", "ShiftLeft"],
	["KeyD"],
	["KeyD", "Space"],
	["KeyS", "M0"],
	["KeyA", "M0"],
	["KeyA"],
	["M2"],
	["WheelUp"],
	["WheelDown"],
	["KeyQ"],
	["KeyE", "M0"],
	["ControlLeft", "KeyS"],
	["M3"],
	[],
];
const PAD_DEMO = [
	["GpA"],
	["GpA", "GpRB"],
	["GpX"],
	["GpUp"],
	["GpRight"],
	["GpB", "GpLB"],
	["GpY"],
	["GpL3"],
	["GpDown", "GpA"],
	["GpMenu"],
	[],
];
const PAD_BUTTONS = [
	"GpA",
	"GpB",
	"GpX",
	"GpY",
	"GpLB",
	"GpRB",
	null,
	null,
	"GpView",
	"GpMenu",
	"GpL3",
	"GpR3",
	"GpUp",
	"GpDown",
	"GpLeft",
	"GpRight",
	"GpGuide",
];

const input = (now) => {
	if (s.demo === "auto") {
		const seq = s.preset === "pad" ? PAD_DEMO : KEY_DEMO;
		return {
			pressed: new Set(seq[Math.floor(now / 280) % seq.length]),
			lx: Math.cos(now / 500),
			ly: Math.sin(now / 500),
			rx: Math.sin(now / 900),
			ry: 0,
			lt: Math.max(0, Math.sin(now / 800)),
			rt: Math.max(0, Math.cos(now / 650)),
		};
	}
	const pressed = new Set(live);
	for (const [code, until] of flashes) {
		if (until > now) pressed.add(code);
		else flashes.delete(code);
	}
	const gp = [...(navigator.getGamepads?.() ?? [])].find(Boolean);
	if (!gp) return { pressed, lx: 0, ly: 0, rx: 0, ry: 0, lt: 0, rt: 0 };
	gp.buttons.forEach((button, index) => {
		if (button.pressed && PAD_BUTTONS[index]) pressed.add(PAD_BUTTONS[index]);
	});
	return {
		pressed,
		lx: gp.axes[0] ?? 0,
		ly: gp.axes[1] ?? 0,
		rx: gp.axes[2] ?? 0,
		ry: gp.axes[3] ?? 0,
		lt: gp.buttons[6]?.value ?? 0,
		rt: gp.buttons[7]?.value ?? 0,
	};
};

const paint = (now) => {
	const root = document.querySelector("#stage .io");
	if (root) {
		const state = input(now);
		for (const el of root.querySelectorAll("[data-code]")) {
			const on = el.dataset.code
				.split(" ")
				.some((code) => state.pressed.has(code));
			el.classList.toggle("on", on);
		}
		for (const el of root.querySelectorAll("[data-stick]")) {
			const side = el.dataset.stick;
			el.setAttribute(
				"transform",
				`translate(${state[`${side}x`] * 9} ${state[`${side}y`] * 9})`,
			);
		}
		for (const el of root.querySelectorAll("[data-axis]"))
			el.setAttribute("width", 46 * state[el.dataset.axis]);
	}
	for (const el of document.querySelectorAll(
		"#board [data-code], #maker-preview [data-code]",
	))
		el.classList.toggle(
			"on",
			el.dataset.code.split(" ").some((code) => live.has(code)),
		);
	requestAnimationFrame(paint);
};

const seg = (key, options) =>
	`<div class="seg">${options.map(([value, en, zh]) => `<button data-p="${key}:${value}" class="${String(s[key]) === value ? "on" : ""}">${L(en, zh)}</button>`).join("")}</div>`;
const tool = (en, zh, body) => `<div class="tool">${L(en, zh)}${body}</div>`;
const toggle = (key) =>
	`<button class="switch ${s[key] ? "on" : ""}" data-t="${key}" aria-label="${key}"></button>`;
const slider = (key, min, max) => {
	const at = ((s[key] - min) / (max - min)) * 100;
	return `<div class="slider" data-slide="${key}" data-min="${min}" data-max="${max}"><i></i><b style="width:${at}%"></b><u style="left:${at}%"></u></div>`;
};

const place = () => {
	const [, , , base] = PRESETS.find(([id]) => id === s.preset);
	return `width:${(base * s.size) / 60}%;opacity:${s.opacity / 100}`;
};
const travel = () => {
	const stage = document.getElementById("stage");
	const wrap = stage.querySelector(".io-wrap");
	if (!wrap) return null;
	const pad = stage.clientWidth * MARGIN;
	return {
		wrap,
		pad,
		w: Math.max(1, stage.clientWidth - 2 * pad - wrap.offsetWidth),
		h: Math.max(1, stage.clientHeight - 2 * pad - wrap.offsetHeight),
	};
};
const layout = () => {
	const t = travel();
	if (!t) return;
	t.wrap.style.left = `${t.pad + s.x * t.w}px`;
	t.wrap.style.top = `${t.pad + s.y * t.h}px`;
};
const snap = (value) => {
	const at = Math.min(1, Math.max(0, value));
	return STOPS.find((stop) => Math.abs(stop - at) < 0.04) ?? at;
};

const renderStage = () => {
	const stage = document.getElementById("stage");
	stage.className = `stage ${s.scene}`;
	stage.innerHTML = `
		<div class="ground"></div>
		<div class="block" style="left:12%;top:30%;width:14%;height:32%"></div>
		<div class="block" style="left:44%;top:22%;width:9%;height:40%"></div>
		<div class="block" style="left:70%;top:38%;width:20%;height:24%"></div>
		<div class="hud"></div>
		<div class="hint">${s.demo === "live" ? L("Press keys, click or scroll here, or use a gamepad", "在这里按键、点击、滚动，或用手柄") : ""}</div>
		${s.on ? `<div class="io-wrap" style="${place()}">${overlay(s.preset)}<i class="grip"></i></div>` : ""}`;
	layout();
};

const renderTools = () => {
	document.getElementById("tools").innerHTML = [
		tool(
			"Devices used in this recording",
			"这段录制里用过的设备",
			seg("used", [
				["keys", "Keyboard and mouse", "键鼠"],
				["pad", "Gamepad", "手柄"],
			]),
		),
		tool(
			"Input",
			"输入",
			seg("demo", [
				["auto", "Demo loop", "自动演示"],
				["live", "Yours", "你来操作"],
			]),
		),
		tool(
			"Footage",
			"画面",
			seg("scene", [
				["dark", "Dark", "暗"],
				["bright", "Bright", "亮"],
			]),
		),
		s.preset === "pad"
			? tool(
					"Controller (detected in the app)",
					"手柄类型（应用里自动识别）",
					seg("pad", [
						["xbox", "Xbox", "Xbox"],
						["ps", "PlayStation", "PlayStation"],
						["switch", "Switch", "Switch"],
					]),
				)
			: "",
	].join("");
};

const renderInspector = () => {
	const tiles = PRESETS.map(
		([id, en, zh]) =>
			`<button class="tile ${s.preset === id ? "on" : ""}" data-p="preset:${id}">${id === "mine" ? `<span class="acts">${icon("pencil")}${icon("trash")}</span>` : ""}${overlay(id)}<span>${L(en, zh)}</span></button>`,
	).join("");
	const anchors = STOPS.flatMap((y) =>
		STOPS.map(
			(x) =>
				`<button data-xy="${x}:${y}" class="${s.x === x && s.y === y ? "on" : ""}" aria-label="${x} ${y}"></button>`,
		),
	).join("");
	const number = (key) =>
		`<label class="num">${key.toUpperCase()}<input class="input" type="number" min="0" max="100" data-n="${key}" value="${Math.round(s[key] * 100)}"></label>`;
	const exact = s.xy
		? `<div class="exact">${number("x")}${number("y")}</div>`
		: "";
	document.getElementById("inspector").innerHTML = `
		<div class="head"><b>${L("Input display", "操作显示")}</b><span class="grow"></span>${toggle("on")}</div>
		${
			s.on
				? `<div class="tiles">${tiles}<button class="tile add">${L("New…", "新建…")}</button><button class="tile add">${L("Import…", "导入…")}</button></div>
		${seg("variant", VARIANTS).replace('class="seg"', 'class="seg block"')}
		<div class="field"><div class="line">${L("Position", "位置")}<button class="ibtn ${s.xy ? "on" : ""}" data-t="xy" aria-label="exact">${icon("chevron-down")}</button></div><div class="anchor">${anchors}</div>${exact}</div>
		<div class="field"><div class="line">${L("Size", "大小")}<span class="tnum">${s.size}%</span></div>${slider("size", 30, 120)}</div>
		<div class="field"><div class="line">${L("Opacity", "不透明度")}<span class="tnum">${s.opacity}%</span></div>${slider("opacity", 20, 100)}</div>`
				: ""
		}`;
};

const renderRows = () => {
	const pop = s.info
		? `<div class="pop"><ul><li>${L("Saved only in this project's folder on this PC", "只保存在这台电脑的项目文件夹里")}</li><li>${L("Read only while recording", "只在录制期间读取")}</li><li>${L("Clear it any time in the editor", "随时可在编辑器里清除")}</li></ul><a>${L("Learn more", "了解详情")}</a></div>`
		: "";
	const instant = s.instant
		? `<div class="sub"><button class="select">${L("WASD + mouse", "WASD 加鼠标")} ${icon("chevron-down")}</button>${seg(
				"instantDevice",
				[
					["auto", "Automatic", "自动"],
					["keys", "Keyboard and mouse", "键盘和鼠标"],
					["pad", "Gamepad", "手柄"],
				],
			)}</div>`
		: "";
	document.getElementById("rows").innerHTML = `
		<div><div class="rowcard"><div class="row"><b>${L("Record input", "记录操作输入")}</b><button class="ibtn ${s.info ? "on" : ""}" data-t="info" aria-label="info">${icon("info")}</button><span class="grow"></span>${toggle("record")}</div>${pop}</div><p class="caption">${L("Settings → Recording. The notice sits behind the info button; the row itself carries no sentence.", "设置 → 录制。声明收在信息按钮后面，这一行本身没有说明文字。")}</p></div>
		<div><div class="rowcard"><div class="row"><b>${L("Show input", "显示操作")}</b><span class="grow"></span>${toggle("instant")}</div>${instant}</div><p class="caption">${L("Instant settings. Off by default; device, preset, size, opacity and position appear only once it is on. The Settings page draws all five.", "即时录制设置。默认关闭，打开后才出现设备、预设、大小、不透明度和位置，五项都画在设置页里。")}</p></div>`;
};

const renderBoard = () => {
	const board = document.getElementById("board");
	if (m.kind === "pad") {
		const { view, body } = pad(s.pad, m.pad);
		board.innerHTML = `<svg class="io ${s.variant} pad" viewBox="-6 -2 ${view[0] + 12} ${view[1] + 4}">${body}</svg>`;
		return;
	}
	const keys = m.keys
		.map((k, i) =>
			key(k.x * U, k.y * U, k.w, k.label, k.code).replace(
				"<g ",
				`<g data-i="${i}" ${i === m.sel ? 'data-sel="1" ' : ""}`,
			),
		)
		.join("");
	const block = m.mouse.on
		? `<g class="block" data-i="mouse" ${m.sel === "mouse" ? 'data-sel="1"' : ""}><rect class="hit" x="${m.mouse.x * U}" y="${m.mouse.y * U}" width="${MOUSE_W * U}" height="${MOUSE_H * U}" rx="8"/>${mouse(m.mouse.x * U + 2, m.mouse.y * U, m.mouse.parts)}</g>`
		: "";
	board.innerHTML = `<svg class="io ${s.variant}" viewBox="0 0 ${COLS * U} ${ROWS * U}"><defs><pattern id="dots" width="${U / 2}" height="${U / 2}" patternUnits="userSpaceOnUse"><circle cx="1" cy="1" r="1" fill="rgba(255,255,255,0.16)"/></pattern></defs><rect width="${COLS * U}" height="${ROWS * U}" fill="url(#dots)"/>${keys}${block}</svg>`;
};
const renderMaker = () => {
	const flag = (key, on, en, zh) =>
		`<label class="flag">${L(en, zh)}<button class="switch ${on ? "on" : ""}" data-mt="${key}" aria-label="${pick(en, zh)}"></button></label>`;
	const chips = (attribute, parts, state) =>
		`<div class="chips">${parts.map(([id, en, zh]) => `<button class="tag chip ${state[id] ? "on" : ""}" ${attribute}="${id}" aria-pressed="${Boolean(state[id])}">${L(en, zh)}</button>`).join("")}</div>`;
	document.getElementById("maker-bar").innerHTML =
		`<span class="caption">${L("Start from", "起点")}</span><div class="seg">${STARTS.map(([id, en, zh]) => `<button data-mf="${id}" class="${m.from === id ? "on" : ""}">${L(en, zh)}</button>`).join("")}</div><span class="grow"></span>${
			m.kind === "keys"
				? `${flag("capture", m.capture, "Press keys to add", "按键添加")}${flag("mouse", m.mouse.on, "Mouse", "鼠标")}<button class="btn" data-md="all">${L("Clear", "清空")}</button>`
				: ""
		}`;
	renderBoard();
	const picked = m.keys[m.sel];
	document.getElementById("maker-key").innerHTML =
		m.kind === "pad"
			? `<span class="caption">${L("Shown", "显示")}</span>${chips("data-pp", PAD_PARTS, m.pad)}`
			: m.sel === "mouse"
				? `<span class="caption">${L("Mouse", "鼠标")}</span>${chips("data-mp", MOUSE_PARTS, m.mouse.parts)}<span class="grow"></span><button class="btn" data-md="mouse">${L("Remove", "移除")}</button>`
				: picked
					? `<input class="input" data-ml value="${picked.label}" aria-label="label"><div class="seg">${WIDTHS.map((w) => `<button data-mw="${w}" class="${picked.w === w ? "on" : ""}">${w}×</button>`).join("")}</div><button class="btn" data-md="one">${L("Delete", "删除")}</button>`
					: "";
	const { view, body } = mine();
	document.getElementById("maker-preview").innerHTML =
		`<div class="stage ${s.scene} mini"><div class="ground"></div><svg class="io ${s.variant}" style="opacity:${s.opacity / 100}" viewBox="-6 -2 ${view[0] + 12} ${view[1] + 4}" aria-hidden="true">${body}</svg></div>${seg("variant", VARIANTS).replace('class="seg"', 'class="seg block"')}${seg(
			"scene",
			[
				["dark", "Dark footage", "暗画面"],
				["bright", "Bright footage", "亮画面"],
			],
		).replace('class="seg"', 'class="seg block"')}`;
	document.getElementById("maker-save").innerHTML =
		`<input class="input name" value="${pick("My preset", "我的预设")}" aria-label="name"><span class="grow"></span><button class="btn">${L("Cancel", "取消")}</button><button class="btn primary">${L("Save", "保存")}</button>`;
};

const renderAll = () => {
	if (!s.manual) s.preset = s.used === "pad" ? "pad" : "wasd";
	document.getElementById("t-overlay").innerHTML = L(
		"Input display: three built-in styles",
		"操作显示：三种自带样式",
	);
	document.getElementById("l-overlay").innerHTML = L(
		"Spec #136. The same presets in three styles. Turning the display on picks the preset from the devices the recording used, so the common case is one click. Drag it on the picture; drag its corner to resize.",
		"Spec #136。同一套预设画成三种样式。打开显示时按这段录制用过的设备自动选预设，常见情况只要点一下。在画面上直接拖动它，拖右下角改大小。",
	);
	document.getElementById("t-disclosure").innerHTML = L(
		"Where the words go",
		"文字放在哪",
	);
	document.getElementById("l-disclosure").innerHTML = L(
		"The two settings rows, with the privacy notice one click away.",
		"两处设置行，隐私声明在点一下之后。",
	);
	renderTools();
	renderStage();
	renderInspector();
	renderRows();
	document.getElementById("t-maker").innerHTML = L("Make a preset", "制作预设");
	document.getElementById("l-maker").innerHTML = L(
		"Proposal for #186, not built yet. A preset starts from any built-in one. A keyboard preset holds keys and one mouse block that is dragged like a key and shows only the buttons chosen for it. A gamepad preset is the built-in pad with parts switched off; its parts are not moved one by one. The right side draws the result the way the video will, in the chosen style, and lights up as you press. Today's maker has only the key grid, a mouse switch and WASD as the start.",
		"#186 的方案，还没实现。预设可以从任何内置预设开始。键盘预设里有按键和一个鼠标块，鼠标块像按键一样拖动，只画选中的那些按钮。手柄预设是内置手柄去掉不要的部分，不逐个移动零件。右边按视频里的样子画出结果，用选中的样式，按下时会亮。现在的制作窗口只有按键网格、一个鼠标开关，并且只能从 WASD 开始。",
	);
	renderMaker();
};

document.addEventListener("click", (event) => {
	const made = event.target.closest(
		"[data-mt],[data-mw],[data-md],[data-mf],[data-mp],[data-pp]",
	);
	if (made) {
		const { mt, mw, md, mf, mp, pp } = made.dataset;
		if (mf) m = startFrom(mf);
		if (mt === "capture") m.capture = !m.capture;
		if (mt === "mouse") {
			m.mouse.on = !m.mouse.on;
			m.sel = m.mouse.on ? "mouse" : -1;
		}
		if (mp) m.mouse.parts[mp] = !m.mouse.parts[mp];
		if (pp) m.pad[pp] = !m.pad[pp];
		if (mw && m.keys[m.sel]) {
			m.keys[m.sel].w = Number(mw);
			m.keys[m.sel].x = Math.min(m.keys[m.sel].x, COLS - Number(mw));
		}
		if (md === "all") {
			m.keys = [];
			m.mouse.on = false;
		}
		if (md === "one") m.keys.splice(m.sel, 1);
		if (md === "mouse") m.mouse.on = false;
		if (md) m.sel = -1;
		renderAll();
		return;
	}
	const set = event.target.closest("[data-p]");
	if (set) {
		const [key, value] = set.dataset.p.split(":");
		s[key] = value;
		if (key === "preset") s.manual = true;
		if (key === "used") s.manual = false;
		renderAll();
		return;
	}
	const spot = event.target.closest("[data-xy]");
	if (spot) {
		[s.x, s.y] = spot.dataset.xy.split(":").map(Number);
		renderAll();
		return;
	}
	const flip = event.target.closest("[data-t]");
	if (flip) {
		s[flip.dataset.t] = !s[flip.dataset.t];
		renderAll();
	}
});

document.addEventListener("pointerdown", (event) => {
	const track = event.target.closest("[data-slide]");
	if (!track) return;
	const key = track.dataset.slide;
	const min = Number(track.dataset.min);
	const max = Number(track.dataset.max);
	const rect = track.getBoundingClientRect();
	const move = (e) => {
		const at = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
		s[key] = Math.round(min + at * (max - min));
		renderStage();
		renderInspector();
	};
	const stop = () => {
		document.removeEventListener("pointermove", move);
		document.removeEventListener("pointerup", stop);
	};
	document.addEventListener("pointermove", move);
	document.addEventListener("pointerup", stop);
	move(event);
});

document.addEventListener("change", (event) => {
	const field = event.target.closest("[data-n]");
	if (!field) return;
	s[field.dataset.n] = Math.min(1, Math.max(0, Number(field.value) / 100));
	renderAll();
});

document.addEventListener("pointerdown", (event) => {
	const wrap = event.target.closest(".io-wrap");
	if (!wrap) return;
	event.preventDefault();
	const resize = event.target.classList.contains("grip");
	const start = { x: event.clientX, y: event.clientY, sx: s.x, sy: s.y };
	const startSize = s.size;
	const startWidth = wrap.offsetWidth;
	wrap.classList.add("held");
	const move = (e) => {
		const dx = e.clientX - start.x;
		const dy = e.clientY - start.y;
		if (resize) {
			s.size = Math.round(
				Math.min(
					120,
					Math.max(30, (startSize * (startWidth + dx)) / startWidth),
				),
			);
			wrap.style.width = place().split(";")[0].slice(6);
		} else {
			const t = travel();
			s.x = snap(start.sx + dx / t.w);
			s.y = snap(start.sy + dy / t.h);
		}
		layout();
		renderInspector();
	};
	const stop = () => {
		wrap.classList.remove("held");
		document.removeEventListener("pointermove", move);
		document.removeEventListener("pointerup", stop);
	};
	document.addEventListener("pointermove", move);
	document.addEventListener("pointerup", stop);
});
window.addEventListener("resize", layout);

const HELD = ["Tab", "Space", "AltLeft", "AltRight", "ContextMenu"];
window.addEventListener("keydown", (event) => {
	if (event.target.tagName === "INPUT") return;
	if (m.capture && m.kind === "keys") {
		event.preventDefault();
		if (!m.keys.some((k) => k.code === event.code)) {
			m.keys.push({
				label: labelFor(event.code),
				code: event.code,
				w: 1,
				...freeSlot(),
			});
			m.sel = m.keys.length - 1;
			renderAll();
		}
	} else if (s.demo === "live" && HELD.includes(event.code))
		event.preventDefault();
	live.add(event.code);
});
document.addEventListener("input", (event) => {
	if (!event.target.matches("[data-ml]") || !m.keys[m.sel]) return;
	m.keys[m.sel].label = event.target.value;
	renderBoard();
	renderStage();
	renderInspector();
});
document.addEventListener("pointerdown", (event) => {
	const hit = event.target.closest("#board [data-i]");
	if (!hit) return;
	event.preventDefault();
	const onMouse = hit.dataset.i === "mouse";
	m.sel = onMouse ? "mouse" : Number(hit.dataset.i);
	const moved = onMouse ? m.mouse : m.keys[m.sel];
	const [wide, high] = onMouse ? [MOUSE_W, MOUSE_H] : [moved.w, 1];
	const board = document.getElementById("board").getBoundingClientRect();
	const unit = board.width / COLS;
	const start = {
		x: event.clientX,
		y: event.clientY,
		kx: moved.x,
		ky: moved.y,
	};
	const halves = (value) => Math.round(value * 2) / 2;
	const move = (e) => {
		moved.x = Math.min(
			COLS - wide,
			Math.max(0, halves(start.kx + (e.clientX - start.x) / unit)),
		);
		const down = start.ky + (e.clientY - start.y) / unit;
		moved.y = Math.min(
			ROWS - high,
			Math.max(0, onMouse ? halves(down) : Math.round(down)),
		);
		renderBoard();
	};
	const stop = () => {
		document.removeEventListener("pointermove", move);
		document.removeEventListener("pointerup", stop);
		renderAll();
	};
	document.addEventListener("pointermove", move);
	document.addEventListener("pointerup", stop);
	renderMaker();
});
window.addEventListener("keyup", (event) => live.delete(event.code));
window.addEventListener("blur", () => live.clear());
const stageEl = document.getElementById("stage");
stageEl.addEventListener("mousedown", (event) => {
	if (s.demo !== "live") return;
	event.preventDefault();
	live.add(`M${event.button}`);
});
window.addEventListener("mouseup", (event) => live.delete(`M${event.button}`));
stageEl.addEventListener("contextmenu", (event) => event.preventDefault());
stageEl.addEventListener(
	"wheel",
	(event) => {
		if (s.demo !== "live") return;
		event.preventDefault();
		flashes.set(
			event.deltaY < 0 ? "WheelUp" : "WheelDown",
			performance.now() + 120,
		);
	},
	{ passive: false },
);

shell({
	here: "input-display",
	depth: 1,
	sections: [
		["overlay", "Styles", "样式"],
		["maker", "Make a preset", "制作预设"],
		["disclosure", "Words", "文字"],
	],
});
onChange(renderAll);
renderAll();
requestAnimationFrame(paint);
