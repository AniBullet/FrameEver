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
	instantCorner: "bl",
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

const mouse = (x, y) => `
	<rect class="body" x="${x}" y="${y}" width="96" height="150" rx="44"/>
	<g class="k" data-code="M0"><path d="M${x + 4} ${y + 66} V${y + 46} A42 42 0 0 1 ${x + 45} ${y + 4} V${y + 66} Z"/></g>
	<g class="k" data-code="M2"><path d="M${x + 92} ${y + 66} V${y + 46} A42 42 0 0 0 ${x + 51} ${y + 4} V${y + 66} Z"/></g>
	<g class="k" data-code="WheelUp M1"><path d="M${x + 41} ${y + 33} V${y + 25} a7 7 0 0 1 14 0 V${y + 33} Z"/></g>
	<g class="k" data-code="WheelDown M1"><path d="M${x + 41} ${y + 35} V${y + 43} a7 7 0 0 0 14 0 V${y + 35} Z"/></g>
	<g class="k" data-code="M3"><rect x="${x - 4}" y="${y + 74}" width="7" height="18" rx="3"/></g>
	<g class="k" data-code="M4"><rect x="${x - 4}" y="${y + 96}" width="7" height="18" rx="3"/></g>`;

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
const pad = (kind) => {
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
			${trigger(90, "lt", shoulders[2])}${trigger(284, "rt", shoulders[3])}
			<g class="k small" data-code="GpLB"><rect x="76" y="30" width="74" height="18" rx="9"/><text x="113" y="39">${shoulders[0]}</text></g>
			<g class="k small" data-code="GpRB"><rect x="270" y="30" width="74" height="18" rx="9"/><text x="307" y="39">${shoulders[1]}</text></g>
			<path class="body" d="M90 62 Q210 44 330 62 Q372 70 388 152 Q400 217 372 238 Q345 252 318 207 Q300 180 270 180 L150 180 Q120 180 102 207 Q75 252 48 238 Q20 217 32 152 Q48 70 90 62 Z"/>
			${stick(leftStick[0], leftStick[1], "l", "GpL3")}
			${dpad(cross[0], cross[1])}
			${stick(255, 152, "r", "GpR3")}
			${round(300, 130, 11, bottom, "GpA")}${round(322, 108, 11, right, "GpB")}${round(278, 108, 11, left, "GpX")}${round(300, 86, 11, top, "GpY")}
			${round(185, 108, 7, "", "GpView")}${round(235, 108, 7, "", "GpMenu")}${round(210, 84, 9, "", "GpGuide")}`,
	};
};

const COLS = 14;
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
const seed = () =>
	WASD.flatMap((row, y) => {
		let x = 0;
		return row.map(([label, code, w = 1]) => {
			const placed = { label, code, x, y, w };
			x += w;
			return placed;
		});
	});
const m = { keys: seed(), sel: -1, capture: false, mouse: true };
const mine = () => {
	if (m.keys.length === 0 && !m.mouse) return { view: [U, U], body: "" };
	const left = Math.min(...m.keys.map((k) => k.x), COLS);
	const top = Math.min(...m.keys.map((k) => k.y), ROWS);
	const width = Math.max(0, ...m.keys.map((k) => k.x + k.w - left)) * U;
	const height = Math.max(0, ...m.keys.map((k) => k.y + 1 - top)) * U;
	const total = m.mouse ? Math.max(height, 150) : height;
	const drop = (total - height) / 2;
	const keys = m.keys
		.map((k) =>
			key((k.x - left) * U, (k.y - top) * U + drop, k.w, k.label, k.code),
		)
		.join("");
	const gap = width > 0 ? 24 : 0;
	return {
		view: [width + (m.mouse ? gap + 100 : 0), total],
		body: `${keys}${m.mouse ? mouse(width + gap, (total - 150) / 2) : ""}`,
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
	for (const el of document.querySelectorAll("#board [data-code]"))
		el.classList.toggle("on", live.has(el.dataset.code));
	requestAnimationFrame(paint);
};

const seg = (key, options) =>
	`<div class="seg">${options.map(([value, en, zh]) => `<button data-p="${key}:${value}" class="${String(s[key]) === value ? "on" : ""}">${L(en, zh)}</button>`).join("")}</div>`;
const tool = (en, zh, body) =>
	`<div class="tool">${L(en, zh)}${body}</div>`;
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
	const exact = s.xy ? `<div class="exact">${number("x")}${number("y")}</div>` : "";
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
				"instantCorner",
				[
					["tl", "Top left", "左上"],
					["tr", "Top right", "右上"],
					["bl", "Bottom left", "左下"],
					["br", "Bottom right", "右下"],
				],
			)}</div>`
		: "";
	document.getElementById("rows").innerHTML = `
		<div><div class="rowcard"><div class="row"><b>${L("Record input", "记录操作输入")}</b><button class="ibtn ${s.info ? "on" : ""}" data-t="info" aria-label="info">${icon("info")}</button><span class="grow"></span>${toggle("record")}</div>${pop}</div><p class="caption">${L("Settings → Recording. The notice sits behind the info button; the row itself carries no sentence.", "设置 → 录制。声明收在信息按钮后面，这一行本身没有说明文字。")}</p></div>
		<div><div class="rowcard"><div class="row"><b>${L("Show input", "显示操作")}</b><span class="grow"></span>${toggle("instant")}</div>${instant}</div><p class="caption">${L("Instant settings. Off by default; the two choices appear only once it is on.", "即时录制设置。默认关闭，打开后才出现两个选项。")}</p></div>`;
};

const renderBoard = () => {
	const keys = m.keys
		.map((k, i) =>
			key(k.x * U, k.y * U, k.w, k.label, k.code).replace(
				"<g ",
				`<g data-i="${i}" ${i === m.sel ? 'data-sel="1" ' : ""}`,
			),
		)
		.join("");
	document.getElementById("board").innerHTML =
		`<svg class="io ${s.variant}" viewBox="0 0 ${COLS * U} ${ROWS * U}"><defs><pattern id="dots" width="${U / 2}" height="${U / 2}" patternUnits="userSpaceOnUse"><circle cx="1" cy="1" r="1" fill="rgba(255,255,255,0.16)"/></pattern></defs><rect width="${COLS * U}" height="${ROWS * U}" fill="url(#dots)"/>${keys}</svg>`;
};
const renderMaker = () => {
	const flag = (key, en, zh) =>
		`<label class="flag">${L(en, zh)}<button class="switch ${m[key] ? "on" : ""}" data-mt="${key}" aria-label="${key}"></button></label>`;
	document.getElementById("maker-bar").innerHTML =
		`${flag("capture", "Press keys to add", "按键添加")}${flag("mouse", "Mouse", "鼠标")}<span class="grow"></span><button class="btn" data-md="all">${L("Clear", "清空")}</button>`;
	renderBoard();
	const picked = m.keys[m.sel];
	document.getElementById("maker-key").innerHTML = picked
		? `<input class="input" data-ml value="${picked.label}" aria-label="label"><div class="seg">${WIDTHS.map((w) => `<button data-mw="${w}" class="${picked.w === w ? "on" : ""}">${w}×</button>`).join("")}</div><button class="btn" data-md="one">${L("Delete", "删除")}</button>`
		: "";
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
	document.getElementById("t-maker").innerHTML = L(
		"Make a preset",
		"制作预设",
	);
	document.getElementById("l-maker").innerHTML = L(
		"Opened from New… in the preset list, or from the pencil on a preset you made. Switch on Press keys to add and press the keys you want; drag them on the grid. The result is My preset above.",
		"从预设列表的「新建…」打开，或者点自己做的预设上的铅笔。打开「按键添加」，按下想显示的键；在网格上拖动摆放。结果就是上面的「我的预设」。",
	);
	renderMaker();
};

document.addEventListener("click", (event) => {
	const made = event.target.closest("[data-mt],[data-mw],[data-md]");
	if (made) {
		const { mt, mw, md } = made.dataset;
		if (mt) m[mt] = !m[mt];
		if (mw && m.keys[m.sel]) {
			m.keys[m.sel].w = Number(mw);
			m.keys[m.sel].x = Math.min(m.keys[m.sel].x, COLS - Number(mw));
		}
		if (md === "all") m.keys = [];
		if (md === "one") m.keys.splice(m.sel, 1);
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
				Math.min(120, Math.max(30, (startSize * (startWidth + dx)) / startWidth)),
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
	if (m.capture) {
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
	m.sel = Number(hit.dataset.i);
	const moved = m.keys[m.sel];
	const board = document.getElementById("board").getBoundingClientRect();
	const unit = board.width / COLS;
	const start = { x: event.clientX, y: event.clientY, kx: moved.x, ky: moved.y };
	const move = (e) => {
		moved.x = Math.min(
			COLS - moved.w,
			Math.max(
				0,
				Math.round((start.kx + (e.clientX - start.x) / unit) * 2) / 2,
			),
		);
		moved.y = Math.min(
			ROWS - 1,
			Math.max(0, Math.round(start.ky + (e.clientY - start.y) / unit)),
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
