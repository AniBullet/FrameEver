const { L, icon, pick, shell, onChange } = FE;

const INK = {
	ink: "foreground",
	soft: "muted-foreground",
	accent: "link",
	red: "recording",
	green: "success-text",
};
const st = (d, ink, w, move, delay, time, fill = false, idle = "") => ({
	d,
	ink,
	w,
	move,
	delay,
	time,
	fill,
	idle,
});
const DOODLES = {
	TRACKS: [
		[120, 104],
		st("M 18 30 L 102 30", "accent", 9, "draw", 60, 500),
		st("M 18 52 L 84 52", "soft", 9, "draw", 260, 500),
		st("M 18 74 L 94 74", "green", 9, "draw", 460, 500),
		st("M 62 14 L 62 90", "red", 2.5, "fade", 900, 300),
	],
	SNAP: [
		[120, 104],
		st(
			"M 18 30 L 18 16 L 32 16 M 88 16 L 102 16 L 102 30 M 102 74 L 102 88 L 88 88 M 32 88 L 18 88 L 18 74",
			"ink",
			4,
			"draw",
			60,
			700,
		),
		st(
			"M 32 74 L 50 50 L 62 63 L 72 53 L 88 74",
			"accent",
			3.5,
			"draw",
			600,
			500,
		),
		st(
			"M 76 30 C 79.3 30 82 32.7 82 36 C 82 39.3 79.3 42 76 42 C 72.7 42 70 39.3 70 36 C 70 32.7 72.7 30 76 30 Z",
			"accent",
			0,
			"pop",
			1000,
			400,
			true,
		),
	],
	BOLT: [
		[120, 104],
		st(
			"M 26 60 L 68 12 L 61 44 L 94 44 L 52 92 L 59 60 Z",
			"ink",
			4,
			"draw",
			60,
			800,
		),
		st(
			"M 86 66 L 100 66 M 78 78 L 94 78",
			"accent",
			3.5,
			"fade",
			700,
			350,
			false,
			"bob",
		),
	],
	TICK: [[24, 24], st("M 4 13 L 9.5 18 L 20 6", "green", 3, "draw", 200, 300)],
	LENS: [
		[120, 104],
		st(
			"M 52 16 C 68 16 82 30 82 46 C 82 62 68 76 52 76 C 36 76 22 62 22 46 C 22 30 36 16 52 16 Z",
			"ink",
			4,
			"draw",
			60,
			700,
		),
		st("M 74 68 L 98 92", "accent", 5, "draw", 650, 300),
	],
	UP_ARROW: [
		[34, 44],
		st("M 12 40 C 6 30 8 16 22 6", "accent", 3, "draw", 200, 450),
		st("M 12 6 L 23 5 L 22 16", "accent", 3, "draw", 600, 250, false, "bob"),
	],
};
const IDLE = {
	bob: "1.9s ease-in-out",
	pulse: "1.2s ease-in-out",
	march: "1.1s linear",
};
let doodleId = 0;
const doodle = (name, width, offset = 0) => {
	const [view, ...strokes] = DOODLES[name];
	const id = `ob-d-${++doodleId}`;
	const reduced = s.reduced;
	const paths = strokes
		.map((s) => {
			const paint = s.fill
				? `fill="var(--${INK[s.ink]})" stroke="none"`
				: `fill="none" stroke="var(--${INK[s.ink]})" stroke-width="${s.w}" stroke-linecap="round" stroke-linejoin="round"`;
			const len =
				s.idle === "march"
					? 'stroke-dasharray="2 4.2"'
					: s.move === "draw"
						? 'pathLength="1"'
						: "";
			const enter = `doodle-${s.move} var(--time) cubic-bezier(0.33, 1, 0.68, 1) var(--delay) both`;
			const idle = s.idle
				? `, ob-${s.idle} ${IDLE[s.idle]} calc(var(--delay) + var(--time)) infinite`
				: "";
			return `<path class="${s.move}" d="${s.d}" ${paint} ${len} style="--delay:${s.delay + offset}ms;--time:${s.time}ms;${reduced ? "" : `animation:${enter}${idle}`}"/>`;
		})
		.join("");
	return `<svg class="doodle ${reduced ? "still" : ""}" width="${width}" height="${Math.round((width * view[1]) / view[0])}" viewBox="0 0 ${view[0]} ${view[1]}" aria-hidden="true"><defs><filter id="${id}" x="-10" y="-10" width="${view[0] + 20}" height="${view[1] + 20}" filterUnits="userSpaceOnUse"><feTurbulence type="fractalNoise" baseFrequency="0.05" numOctaves="1" seed="1"/><feDisplacementMap in="SourceGraphic" scale="2.5" xChannelSelector="R" yChannelSelector="G"/></filter></defs><g filter="url(#${id})">${paths}</g></svg>`;
};

const STEPS = [
	["modes", "Modes", "模式"],
	["studio", "Studio", "工作室"],
	["instant", "Instant", "即时"],
	["screenshot", "Screenshot", "截图"],
	["library", "Library", "资源库"],
	["setup", "Shortcuts", "快捷键"],
	["faq", "Questions", "常见问题"],
];
const MODES = [
	[
		"studio",
		"clapperboard",
		"TRACKS",
		"Studio",
		"工作室",
		"Record, then edit",
		"先录后剪",
	],
	[
		"instant",
		"zap",
		"BOLT",
		"Instant",
		"即时",
		"A video the moment you stop",
		"停止即得视频",
	],
];
const DETAIL = {
	studio: {
		doodle: "TRACKS",
		title: ["Studio", "工作室"],
		tag: ["Record, then edit", "先录后剪"],
		body: [
			"Screen, camera, microphone and cursor are kept as separate tracks, so you can polish the recording in the editor before you export it.",
			"屏幕、摄像头、麦克风和光标分轨保存，导出前可以在编辑器里慢慢打磨。",
		],
		ticks: [
			["Backgrounds, padding and auto zoom", "背景、边距和自动缩放"],
			["Captions made on this computer", "在本机生成字幕"],
			["Chapter marks while you record", "录制时打章节标记"],
			["Export to MP4 or GIF", "导出为 MP4 或 GIF"],
		],
		labels: [
			["Record", "录制"],
			["Edit", "编辑"],
			["Export", "导出"],
		],
	},
	instant: {
		doodle: "BOLT",
		title: ["Instant", "即时"],
		tag: ["A video the moment you stop", "停止即得视频"],
		body: [
			"Instant records straight to an MP4 on this computer. When you stop, it shows up in the library, ready to play or drag into a chat.",
			"即时录制直接存成本机上的 MP4。停止后它出现在资源库里，可以直接播放，或拖进聊天窗口。",
		],
		ticks: [
			["Ready to play, no rendering", "无需渲染，停止就能播放"],
			["Microphone, system audio and camera", "麦克风、系统声音和摄像头"],
			["Automatic compression if you want it", "需要时自动压缩一份"],
			["Or copy the file when you stop", "也可以设成停止后复制文件"],
		],
		labels: [
			["Record", "录制"],
			["Stop", "停止"],
			["In the library", "进入资源库"],
		],
	},
	screenshot: {
		doodle: "SNAP",
		title: ["Screenshot", "截图"],
		tag: ["Capture and annotate", "截取并标注"],
		body: [
			"Pick an area, mark it up in the screenshot editor, then copy or save the PNG.",
			"框选一块区域，在截图编辑器里标注，然后复制或保存成 PNG。",
		],
		ticks: [
			["A magnifier for pixel-exact edges", "放大镜，边缘对齐到像素"],
			["Arrows, step numbers and speech bubbles", "箭头、步骤编号和对话气泡"],
			["Spotlight, blur and highlighter", "聚光、模糊和荧光笔"],
			["Backgrounds and rounded corners", "背景和圆角"],
		],
		labels: [
			["Select area", "框选区域"],
			["Annotate", "标注"],
			["Copy", "复制"],
		],
	},
};
const LIB_LABELS = [
	["Expand", "展开"],
	["Preview", "预览"],
	["Drag out", "拖出"],
	["Find", "查找"],
];
const TIMING = {
	studio: [0, 600, 1600, 2600, 3600, 4400, 5000, 6000, 7200, 7400, 9200, 11000],
	instant: [0, 600, 1600, 2600, 3400, 4000, 5600, 8000],
	screenshot: [0, 500, 1700, 2300, 2900, 3500, 4100, 5000, 6800],
	library: [0, 900, 2200, 2700, 4200, 5600, 6600, 8400],
};
const FAQ = [
	[
		["Is it free?", "免费吗？"],
		[
			"Yes. FrameEver is free and open source, and everything runs on this computer. There is no account.",
			"免费。FrameEver 免费开源，全部在这台电脑上运行，不需要账号。",
		],
		null,
	],
	[
		["Where do my files go?", "文件存在哪里？"],
		[
			"Into the storage folder, Videos\\FrameEver unless you change it. Projects, screenshots and exports all live there, and nothing is uploaded.",
			"存在存储文件夹里，默认是 Videos\\FrameEver。项目、截图和导出都在这里，不会上传到任何地方。",
		],
		["folder-open", "Open storage folder", "打开存储文件夹"],
	],
	[
		[
			"I used Cap-aniBullet 2.x or Cap. How do I bring my recordings?",
			"以前用 Cap-aniBullet 2.x 或 Cap，录制怎么带过来？",
		],
		[
			"Run FrameEver Converter, a separate download on the releases page. It moves settings, projects and loose videos into the storage folder.",
			"运行 FrameEver Converter，它在发布页单独下载。它会把设置、项目和零散视频搬进存储文件夹。",
		],
		["arrow-up-right", "Open releases page", "打开发布页"],
	],
	[
		["Are captions uploaded?", "字幕会上传吗？"],
		[
			"No. FrameEver downloads a caption model once, then makes captions on this computer.",
			"不会。FrameEver 只下载一次字幕模型，之后都在本机生成字幕。",
		],
		null,
	],
];

const s = {
	step: 0,
	dir: 1,
	mode: "studio",
	phase: 0,
	reduced: false,
	revisit: false,
	conflict: false,
	rec: null,
	keys: { record: ["Alt", "Shift", "R"], shot: ["Alt", "Shift", "S"] },
	faq: [false, true, false, false],
	line: "",
};
let timers = [];

const stepId = () => STEPS[s.step][0];
const heading = (en, zh, subEn, subZh) =>
	`<div class="ob-heading"><span class="h">${L(en, zh)}</span><span class="sub">${L(subEn, subZh)}</span></div>`;
const ARROW = `<svg viewBox="0 0 14 18" width="14" height="18"><path d="M1 1 L1 15 L4.6 11.6 L7.2 17 L9.6 16 L7 10.6 L12 10.6 Z" fill="var(--foreground)" stroke="var(--background)" stroke-width="1.2" stroke-linejoin="round"/></svg>`;
const cursor = (fx, fy, tx, ty) =>
	`<span class="cursor" style="left:${tx};top:${ty};${fx === tx && fy === ty ? "" : `animation:cur 600ms var(--ease-move) both;--fx:${fx};--fy:${fy};--tx:${tx};--ty:${ty}`}">${ARROW}</span>`;
const desktop = () =>
	`<div class="lines">${[70, 54, 82, 46, 64, 38].map((w) => `<i style="width:${w}%"></i>`).join("")}</div>`;
const time = (n) => `0:0${n}`;
const labels = (list, on) =>
	`<div class="labels">${list.map((l, i) => `${i ? "<i>·</i>" : ""}<span class="${i === on ? "on" : ""}">${L(...l)}</span>`).join("")}</div>`;

const demoStudio = (p) => {
	const bar =
		p === 0
			? `<div class="bar"><span class="start">${L("Start recording", "开始录制")}</span></div>`
			: p <= 4
				? `<div class="bar"><span class="dot"></span><span class="tnum">${time(Math.min(3, p - 1))}</span>${icon("pause")}<span class="stop"></span></div>`
				: "";
	const chapter =
		p === 3 || p === 4
			? `<span class="chip" style="left:50%;bottom:52px;transform:translateX(-50%)">${L("Chapter marked", "已打章节标记")}</span>`
			: "";
	let editor = "";
	if (p >= 5) {
		const head = p === 5 ? 0 : 58;
		editor = `<div class="editor" style="${p > 5 ? "animation:none" : ""}"><div class="preview ${p >= 7 ? "zoom" : ""}"><div class="shot"></div>${p >= 6 ? `<span class="caption" style="${p > 6 ? "animation:none" : ""}">${L("Today we'll look at the new export page", "今天来看看新的导出页")}</span>` : ""}</div>
			<div class="tracks"><div class="track"><span class="seg-scene"></span><span class="mark" style="left:42%"></span></div><div class="track"><span class="seg-zoom"></span></div><div class="track"><span class="seg-text"></span></div><span class="head" style="left:${head}%;${p === 6 ? "animation:head 1000ms linear both" : ""}"></span></div></div>`;
	}
	let exp = "";
	if (p >= 8) {
		const done = p >= 10;
		exp = `<div class="export" style="${p > 8 ? "animation:none" : ""}"><div class="box"><b>${L("Export", "导出")}</b><div class="row"><span>MP4 · 1080p</span><span class="tnum">${done ? "100%" : p === 9 ? "" : "0%"}</span></div><div class="progress-line"><i style="${done ? "width:100%" : p === 9 ? "animation:fill 1700ms linear both" : ""}"></i></div>${done ? `<span class="done">${icon("circle-check")}${L("Exported", "已导出")}</span>` : ""}</div></div>`;
	}
	const label = p >= 8 ? 2 : p >= 5 ? 1 : 0;
	return [
		`<div class="screen">${desktop()}${bar}${chapter}${editor}${exp}</div>`,
		labels(DETAIL.studio.labels, label),
	];
};

const tilesHtml = (fresh, sel, drag, count = 8) =>
	Array.from({ length: count }, (_, i) => {
		const kind = ["zap", "clapperboard", "image", "zap"][i % 4];
		const cls = [
			i === 0 && fresh ? "fresh" : "",
			i === 0 && sel ? "sel" : "",
			i === 0 && drag ? "drag" : "",
		].join(" ");
		return `<div class="tile ${cls}" style="${i === 0 && drag === "run" ? "animation:drag 900ms var(--ease-move) both" : ""}"><div class="t">${icon(kind)}</div><span>${i === 0 ? pick("Instant 11-15", "即时 11-15") : pick(["Bug report", "Demo", "Login page", "Weekly"][i % 4], ["问题复现", "演示", "登录页", "周报"][i % 4])}</span></div>`;
	}).join("");
const libTabs = () =>
	`<div class="tabs"><span class="on">${pick("All", "全部")}</span><span>${pick("Studio", "工作室")}</span><span>${pick("Instant", "即时")}</span><span>${pick("Screenshots", "截图")}</span></div>`;

const demoInstant = (p) => {
	const bar =
		p === 0
			? `<div class="bar"><span class="start">${L("Start recording", "开始录制")}</span></div>`
			: p <= 4
				? `<div class="bar"><span class="dot"></span><span class="tnum">${time(Math.min(2, p - 1))}</span>${icon("pause")}<span class="stop"></span></div>`
				: "";
	const cur =
		p === 3
			? cursor("70%", "40%", "70%", "40%")
			: p === 4
				? cursor("70%", "40%", "61%", "84%")
				: p === 6
					? cursor("14%", "30%", "52%", "52%")
					: "";
	let lib = "";
	if (p >= 5) {
		lib = `<div class="lib" style="inset:8px;${p === 5 ? "animation:ob-rise 450ms var(--ease-fluent-out) both" : ""}">${libTabs()}<div class="grid">${tilesHtml(true, p >= 6, p === 6 ? "run" : p > 6 ? "drag" : "")}</div></div>`;
	}
	const chip =
		p >= 6
			? `<span class="chip" style="right:14px;bottom:14px">${icon("copy")}${L("Drop it into any app", "拖进任意应用")}</span>`
			: "";
	const label = p >= 5 ? 2 : p === 4 ? 1 : 0;
	return [
		`<div class="screen">${desktop()}${bar}${lib}${chip}${cur}</div>`,
		labels(DETAIL.instant.labels, label),
	];
};

const demoShot = (p) => {
	let body = desktop();
	if (p <= 1) {
		body += `<div class="sel-area ${p === 1 ? "full" : ""}">${p === 1 ? '<span class="size tnum">1280 × 720</span>' : ""}</div><div class="loupe" style="${p === 1 ? "left:68%;top:66%" : "left:16%;top:8%"}"></div>`;
	}
	if (p >= 2) {
		const tools = [
			"arrow-up-right",
			"num",
			"highlighter",
			"type",
			"eye-off",
			"search",
		]
			.map((t, i) =>
				t === "num"
					? `<span class="num ${p >= 3 && p < 5 ? "on" : ""}">1</span>`
					: icon(t, i === 0 ? "" : ""),
			)
			.join("");
		body += `<div class="shot-ed" style="${p > 2 ? "animation:none" : ""}"><div class="rail">${tools}</div><div class="canvas"><div class="img">${desktop()}${p >= 5 ? `<span class="spot" style="${p > 5 ? "animation:none" : ""}"></span>` : ""}${p >= 3 ? `<span class="badge-num" style="left:22%;top:26%;${p > 3 ? "animation:none" : ""}">1</span>` : ""}${p >= 4 ? `<span class="badge-num" style="left:58%;top:58%;${p > 4 ? "animation:none" : ""}">2</span>` : ""}${p >= 6 ? `<span class="bubble" style="${p > 6 ? "animation:none" : ""}">${pick("Click here", "点这里")}</span>` : ""}</div></div></div>`;
		if (p === 2) body += '<span class="shutter"></span>';
	}
	if (p >= 7)
		body += `<span class="chip ok" style="right:14px;bottom:14px">${icon("check")}${L("Screenshot copied", "截图已复制")}</span>`;
	const label = p >= 7 ? 2 : p >= 2 ? 1 : 0;
	return [
		`<div class="screen">${body}</div>`,
		labels(DETAIL.screenshot.labels, label),
	];
};

const demoLibrary = (p) => {
	let body = desktop();
	if (p === 0) {
		body += `<div class="compact"><div class="seg-mini"><i class="on"></i><i></i><i></i></div><div class="tiles"><i></i><i></i><i></i><i></i></div><div class="startbtn"></div><div class="foot"><span class="hl">${pick("Library", "资源库")} ›</span></div></div>${cursor("40%", "60%", "22%", "60%")}`;
	} else {
		body += `<div class="lib" style="inset:8px;${p === 1 ? "animation:expand 550ms var(--ease-move) both" : ""}">${libTabs()}<div class="grid">${tilesHtml(false, p >= 2, p === 4 ? "run" : "")}</div></div>`;
	}
	if (p === 3)
		body += `<div class="viewer"><div class="pv">${icon("play")}</div></div>`;
	if (p === 4)
		body += `<span class="chip" style="right:14px;bottom:14px">${icon("copy")}${L("The file itself, not a link", "拖出的是文件本身")}</span>`;
	if (p >= 5)
		body += `<div class="palette"><div class="q">${icon("search")}<span>${p === 5 ? "" : pick("compress", "压缩")}</span><span class="caret"></span></div>${(p ===
		5
			? [
					["Open settings", "打开设置", "设置"],
					["Show storage folder", "显示存储文件夹", ""],
					["Start recording", "开始录制", ""],
				]
			: [
					["Compress selected", "压缩所选", ""],
					["Automatic compression", "自动压缩", ""],
				]
		)
			.map(
				([en, zh], i) =>
					`<div class="r ${i === 0 ? "on" : ""}"><span>${pick(en, zh)}</span><span>${i === 0 ? "Enter" : ""}</span></div>`,
			)
			.join("")}</div>`;
	const label = p >= 5 ? 3 : p === 4 ? 2 : p >= 2 ? 1 : 0;
	return [`<div class="screen">${body}</div>`, labels(LIB_LABELS, label)];
};

const DEMO = {
	studio: demoStudio,
	instant: demoInstant,
	screenshot: demoShot,
	library: demoLibrary,
};

const renderModes = () => {
	const k = MODES.find((m) => m[0] === s.mode);
	return `${heading("Two ways to record", "两种录制方式", "Pick the one you'll use most. You can switch any time at the top of the main window. Screenshots have their own button and shortcuts.", "选一种你最常用的。之后随时可以在主窗口顶部切换。截图有自己的按钮和快捷键。")}
	<div class="modes">${MODES.map(
		([id, , dd, en, zh, ten, tzh], i) =>
			`<button class="paper mode-card rise ${s.mode === id ? "on" : ""}" data-a="mode" data-v="${id}" style="animation-duration:${700 + i * 100}ms"><span class="pick">${s.mode === id ? icon("check") : ""}</span>${doodle(dd, 96, i * 180)}<span class="name">${pick(en, zh)}</span><span class="tag-line">${pick(ten, tzh)}</span></button>`,
	).join("")}</div>
	<div class="where"><div class="seg">${MODES.map(([id, ic, , en, zh]) => `<button class="${s.mode === id ? "on" : ""}">${icon(ic, "sm")}${pick(en, zh)}</button>`).join("")}</div><small>${doodle("UP_ARROW", 14, 300)}${L(`${k[3]} is where the main window starts`, `主窗口会从“${k[4]}”开始`)}</small></div>`;
};

const renderDetail = (id) => {
	const d = DETAIL[id];
	const [screen, lab] = DEMO[id](s.phase);
	return `<div class="detail"><div class="side"><div class="title-row">${doodle(d.doodle, 58)}<div><span class="h">${L(...d.title)}</span><span class="sub">${L(...d.tag)}</span></div></div><p>${L(...d.body)}</p><div class="ticks">${d.ticks.map((t, i) => `<div>${doodle("TICK", 18, 400 + i * 140)}${L(...t)}</div>`).join("")}</div></div>
	<div class="paper demo-card" id="demo">${screen}${lab}</div></div>`;
};

const renderLibrary = () => {
	const [screen, lab] = DEMO.library(s.phase);
	return `${heading("Everything lands in the library", "录好的都在资源库", "Press Ctrl+L or click Library and the main window expands in place. Collapse it to record again.", "按 Ctrl+L 或点“资源库”，主窗口会原地展开；收回去就能接着录。")}
	<div class="paper demo-card wide" id="demo">${screen}${lab}</div>
	<div class="keys"><div><span class="kbd">Space</span>${L("Preview", "预览")}</div><div><span class="kbd">Ctrl+K</span>${L("Find any command", "查找任何命令")}</div><div><span class="kbd">${pick("Drag", "拖动")}</span>${L("Drop the file into any app", "把文件拖进任意应用")}</div></div>`;
};

const keyField = (id) => {
	const keys = s.keys[id];
	const rec = s.rec === id;
	const content = rec
		? `<span class="kf">${L("Press keys…", "按下快捷键…")}</span>`
		: keys.length
			? `<span class="kf">${keys.map((k) => `<span class="kbd">${k}</span>`).join(" ")}</span><span class="x" data-a="clear" data-v="${id}" title="${pick("Clear", "清除")}">${icon("x")}</span>`
			: `<span class="kf">${L("Not set", "未设置")}</span>`;
	return `<button class="keyfield ${rec ? "rec" : ""}" data-a="rec" data-v="${id}">${content}</button>`;
};

const renderSetup = () => {
	const recordNote =
		s.conflict && !s.revisit
			? `<small class="warn">${L("Alt+Shift+R is taken by another program. Pick other keys.", "Alt+Shift+R 已被其他程序占用，换一组键。")}</small>`
			: `<small>${L("Opens the picker for a display, window or area", "打开显示器、窗口或区域选择")}</small>`;
	return `${heading("Set two shortcuts", "设两个快捷键", s.revisit ? "These are your current shortcuts. Change them here or later in Settings → Shortcuts." : "Start from anywhere without opening the main window. Change them later in Settings → Shortcuts.", s.revisit ? "这是你现在的快捷键。可以在这里改，之后也能在 设置 → 快捷键 里改。" : "不用打开主窗口，随时就能开始。之后可在 设置 → 快捷键 里修改。")}
	<div class="paper setup rise">
		<div class="row">${icon("video")}<div class="grow"><b>${L("Start recording", "开始录制")}</b>${recordNote}</div>${keyField("record")}</div>
		<div class="row">${icon("scan")}<div class="grow"><b>${L("Screenshot an area", "截取区域")}</b><small>${L("Drag over the screen, then annotate", "在屏幕上框选，然后标注")}</small></div>${keyField("shot")}</div>
		<div class="row">${icon("folder")}<div class="grow"><b>${L("Storage folder", "存储文件夹")}</b><span class="path">C:\\Users\\you\\Videos\\FrameEver</span></div><button class="btn">${L("Change…", "更改…")}</button></div>
	</div>`;
};

const renderFaq = () =>
	`${heading("Good to know", "开始前了解一下", "Short answers to what people ask first.", "大家最先问的几个问题。")}
	<div class="paper faq rise">${FAQ.map(
		([q, a, action], i) =>
			`<div class="item"><button class="q" data-a="faq" data-v="${i}">${L(...q)}${icon(s.faq[i] ? "chevron-up" : "chevron-down", "sm")}</button>${s.faq[i] ? `<div class="a">${L(...a)}${action ? `<div><button class="btn">${icon(action[0], "sm")}${L(action[1], action[2])}</button></div>` : ""}</div>` : ""}</div>`,
	).join("")}</div>`;

const renderContent = () => {
	const id = stepId();
	if (id === "modes") return renderModes();
	if (id === "library") return renderLibrary();
	if (id === "setup") return renderSetup();
	if (id === "faq") return renderFaq();
	return renderDetail(id);
};

const renderNav = () => {
	const last = s.step === STEPS.length - 1;
	return `<div class="ob-nav"><div>${s.step > 0 ? `<button class="back" data-a="back">← ${pick("Back", "上一步")}</button>` : ""}</div>
	<div class="mid"><div class="dots">${STEPS.map((_, i) => `<i class="${i === s.step ? "on" : i < s.step ? "done" : ""}"></i>`).join("")}</div><span class="hint">${L("Press Enter or use ← →", "按 Enter 或 ← → 键")}</span></div>
	<div class="right">${last ? "" : `<button class="skip" data-a="skip">${L("Skip the tour", "跳过引导")}</button>`}<button class="btn primary" data-a="next">${last ? L("Start using", "开始使用") : L("Continue", "继续")}</button></div></div>`;
};

const STEP_LINES = {
	modes: [
		"Clicking a card sets the recording mode at once, as the old Toggle step did. The segmented row under the cards is the main window's own mode row.",
		"点卡片立刻设定录制模式，和原来的切换模式页一样。卡片下面那排就是主窗口自己的模式切换条。",
	],
	studio: [
		"The demo draws the real recording bar, the editor's preview over its timeline (scene, zoom and caption lanes, a chapter mark) and the export page's progress.",
		"演示里画的是真实的录制条、编辑器预览加时间轴（场景、缩放、字幕三道和一个章节标记），以及导出页的进度。",
	],
	instant: [
		"After an Instant recording the app shows it in the library by default (Settings: Show in library, Copy file, Do nothing). The demo ends there, then drags the tile out as a file.",
		"即时录制结束后默认在资源库里显示（设置里可选：在资源库中显示、复制文件、不做任何事）。演示就停在资源库，再把卡片当文件拖出去。",
	],
	screenshot: [
		"Area picker with its magnifier, then the screenshot editor's rail: step numbers, spotlight and a speech bubble, then the copied message.",
		"区域选择带放大镜，然后是截图编辑器左侧工具栏：步骤编号、聚光、对话气泡，最后是已复制提示。",
	],
	library: [
		"Compact window → Library expands in place → Space preview → drag the file out → Ctrl+K command palette.",
		"小窗 → 资源库原地展开 → 空格预览 → 把文件拖出去 → Ctrl+K 命令面板。",
	],
	setup: [
		"Click a key field and press keys to try it. Keys are written only when you press Continue on this step; Skip writes nothing. Use the toggles above for the conflict and revisit states.",
		"点快捷键框再按键可以试。只有在这一步点“继续”才写入；跳过什么都不写。上面的开关可以看冲突和重看两种状态。",
	],
	faq: [
		"Start using closes onboarding and shows the compact main window with its first-run line.",
		"“开始使用”关闭引导，打开小窗，底栏显示首次提示。",
	],
};

const render = () => {
	const stage = document.getElementById("stage");
	stage.innerHTML = `<div class="ob ${s.dir < 0 ? "back" : ""} ${s.reduced ? "reduced" : ""}"><div class="ob-head"><span>${icon("minus", "sm")}</span><span>${icon("x", "sm")}</span></div><div class="ob-scroll"><div class="ob-content">${renderContent()}</div></div>${renderNav()}</div>`;
	document.getElementById("line").innerHTML =
		s.line || L(...STEP_LINES[stepId()]);
	renderTools();
};

const renderDemo = () => {
	const card = document.getElementById("demo");
	const id = stepId();
	if (!card || !DEMO[id]) return;
	const [screen, lab] = DEMO[id](s.phase);
	card.innerHTML = screen + lab;
};

const runDemo = () => {
	for (const t of timers) clearTimeout(t);
	timers = [];
	const plan = TIMING[stepId()];
	if (!plan) return;
	if (s.reduced) {
		s.phase = plan.length - 2;
		renderDemo();
		return;
	}
	const cycle = () => {
		plan.slice(0, -1).forEach((at, i) => {
			timers.push(
				setTimeout(() => {
					s.phase = i;
					renderDemo();
				}, at),
			);
		});
		timers.push(setTimeout(cycle, plan[plan.length - 1]));
	};
	cycle();
};

const go = (step) => {
	if (step < 0 || step >= STEPS.length) return;
	s.dir = step >= s.step ? 1 : -1;
	s.step = step;
	s.phase = 0;
	s.rec = null;
	s.line = "";
	render();
	runDemo();
};

const renderTools = () => {
	document.getElementById("tools").innerHTML = `
		<div class="tool">${L("Step", "步骤")}<div class="seg">${STEPS.map(([, en, zh], i) => `<button class="${i === s.step ? "on" : ""}" data-a="go" data-v="${i}">${i + 1} ${pick(en, zh)}</button>`).join("")}</div></div>
		<div class="tool">${L("Motion", "动效")}<div class="seg"><button class="${s.reduced ? "" : "on"}" data-a="reduced" data-v="0">${L("Full", "完整")}</button><button class="${s.reduced ? "on" : ""}" data-a="reduced" data-v="1">${L("Reduced", "减少")}</button></div></div>
		<div class="tool">${L("Opened", "打开方式")}<div class="seg"><button class="${s.revisit ? "" : "on"}" data-a="revisit" data-v="0">${L("First run", "首次")}</button><button class="${s.revisit ? "on" : ""}" data-a="revisit" data-v="1">${L("From Help", "从帮助重看")}</button></div></div>
		<div class="tool">${L("Shortcut", "快捷键")}<div class="seg"><button class="${s.conflict ? "" : "on"}" data-a="conflict" data-v="0">${L("Free", "可用")}</button><button class="${s.conflict ? "on" : ""}" data-a="conflict" data-v="1">${L("Taken", "被占用")}</button></div></div>
		<div class="tool">&nbsp;<button class="btn" data-a="replay">${icon("rotate-ccw", "sm")}${L("Replay", "重播")}</button></div>`;
};

const applyFlags = () => {
	if (s.revisit) s.keys = { record: [], shot: ["Ctrl", "Shift", "4"] };
	else if (s.conflict) s.keys = { record: [], shot: ["Alt", "Shift", "S"] };
	else s.keys = { record: ["Alt", "Shift", "R"], shot: ["Alt", "Shift", "S"] };
};

document.addEventListener("click", (e) => {
	const el = e.target.closest("[data-a]");
	if (!el) return;
	const { a, v } = el.dataset;
	if (a === "go") return go(Number(v));
	if (a === "next") {
		if (s.step === STEPS.length - 1) {
			s.line = L(...STEP_LINES.faq);
			return render();
		}
		return go(s.step + 1);
	}
	if (a === "back") return go(s.step - 1);
	if (a === "skip") {
		s.line = L(
			"Skip the tour: closes onboarding, writes no shortcuts and opens the compact main window.",
			"跳过引导：关闭引导，不写入任何快捷键，打开小窗。",
		);
		return render();
	}
	if (a === "mode") {
		s.mode = v;
		return render();
	}
	if (a === "clear") {
		e.stopPropagation();
		s.keys[v] = [];
		s.rec = null;
		return render();
	}
	if (a === "rec") {
		s.rec = s.rec === v ? null : v;
		return render();
	}
	if (a === "faq") {
		s.faq[v] = !s.faq[v];
		return render();
	}
	if (a === "reduced") {
		s.reduced = v === "1";
		render();
		return runDemo();
	}
	if (a === "revisit") {
		s.revisit = v === "1";
		applyFlags();
		return go(s.revisit && s.step === 0 ? 0 : s.step);
	}
	if (a === "conflict") {
		s.conflict = v === "1";
		applyFlags();
		return go(5);
	}
	if (a === "replay") return go(s.step);
});

document.addEventListener("keydown", (e) => {
	if (e.target.closest("input,textarea")) return;
	if (s.rec) {
		e.preventDefault();
		if (e.key === "Escape") {
			s.rec = null;
			return render();
		}
		if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return;
		const mods = [
			e.ctrlKey && "Ctrl",
			e.altKey && "Alt",
			e.shiftKey && "Shift",
		].filter(Boolean);
		if (!mods.length) return;
		const key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
		s.keys[s.rec] = [...mods, key];
		s.rec = null;
		return render();
	}
	if (!document.getElementById("stage").matches(":hover")) return;
	if (e.key === "ArrowRight" || e.key === "Enter") {
		e.preventDefault();
		go(Math.min(STEPS.length - 1, s.step + 1));
	}
	if (e.key === "ArrowLeft") {
		e.preventDefault();
		go(s.step - 1);
	}
});

setInterval(() => {
	if (s.reduced) return;
	const seed = String(1 + Math.floor(Math.random() * 6));
	for (const f of document.querySelectorAll(".ob feTurbulence"))
		f.setAttribute("seed", seed);
}, 130);

const style = document.createElement("style");
style.textContent =
	"@keyframes ob-bob{50%{transform:translateY(-4px)}}@keyframes ob-pulse{50%{opacity:.3}}@keyframes ob-march{to{stroke-dashoffset:-6.2}}@keyframes cur{from{left:var(--fx);top:var(--fy)}to{left:var(--tx);top:var(--ty)}}@keyframes head{from{left:0}to{left:58%}}@keyframes fill{from{width:0}to{width:100%}}@keyframes drag{to{transform:translate(150px,40px) rotate(-4deg);opacity:.85}}@keyframes expand{from{inset:16px auto auto 16px;width:132px;height:176px}to{inset:8px;width:calc(100% - 16px);height:calc(100% - 16px)}}";
document.head.append(style);

const text = (id, en, zh) => {
	document.getElementById(id).innerHTML = L(en, zh);
};
const table = (id, head, rows) => {
	document.getElementById(id).innerHTML =
		`<table class="inv"><thead><tr>${head.map((h) => `<th>${L(...h)}</th>`).join("")}</tr></thead><tbody>${rows.map((r) => `<tr>${r.map((c) => `<td>${Array.isArray(c) ? L(...c) : c}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
};

const sections = () => {
	text("t-steps", "Onboarding steps", "引导步骤");
	text(
		"l-steps",
		"The steps after the welcome page, in the 860 × 690 onboarding window. The welcome page stays as it is on the tokens page. Paper cards, doodles, boil and entrances are unchanged; what changes is what each step says and draws. Hover the window and use ← → or Enter.",
		"欢迎页之后的步骤，窗口 860 × 690。欢迎页保持 Token 页上的样子不变。纸卡片、手绘、抖动和入场动画都不变，变的是每一步讲什么、画什么。鼠标停在窗口上可用 ← → 或 Enter。",
	);
	text("t-teach", "What each step teaches", "每一步讲什么");
	text(
		"l-teach",
		"Every step points at a surface that exists in 1.0. macOS keeps its Permissions step first.",
		"每一步都对应 1.0 里真实存在的界面。macOS 仍然先显示权限页。",
	);
	table(
		"teach-table",
		[
			["Step", "步骤"],
			["Teaches", "讲什么"],
			["Draws", "画的真实界面"],
			["Can set", "能设置"],
		],
		[
			[
				["1 Modes", "1 模式"],
				[
					"Studio and Instant, and where to switch",
					"工作室和即时两种方式，以及在哪切换",
				],
				["Main window mode row", "主窗口模式切换条"],
				["Recording mode", "录制模式"],
			],
			[
				["2 Studio", "2 工作室"],
				[
					"Separate tracks, editor, captions, auto zoom, chapter marks, export",
					"分轨、编辑器、字幕、自动缩放、章节标记、导出",
				],
				["Recording bar, editor, export page", "录制条、编辑器、导出页"],
				["—", "—"],
			],
			[
				["3 Instant", "3 即时"],
				[
					"MP4 when you stop, lands in the library, compression, copy instead",
					"停止即得 MP4，进入资源库，压缩，或改成复制文件",
				],
				["Recording bar, library", "录制条、资源库"],
				["—", "—"],
			],
			[
				["4 Screenshot", "4 截图"],
				[
					"Area with magnifier, annotation tools, copy or save",
					"带放大镜的框选、标注工具、复制或保存",
				],
				["Area picker, screenshot editor", "区域选择、截图编辑器"],
				["—", "—"],
			],
			[
				["5 Library", "5 资源库"],
				[
					"Ctrl+L expand, Space preview, drag the file out, Ctrl+K",
					"Ctrl+L 展开、空格预览、拖出文件、Ctrl+K",
				],
				[
					"Compact window, library, preview, command palette",
					"小窗、资源库、预览、命令面板",
				],
				["—", "—"],
			],
			[
				["6 Shortcuts", "6 快捷键"],
				[
					"Two global shortcuts and the storage folder",
					"两个全局快捷键和存储文件夹",
				],
				["Settings rows", "设置里的行"],
				[
					"Start recording, Screenshot an area, storage folder",
					"开始录制、截取区域、存储文件夹",
				],
			],
			[
				["7 Questions", "7 常见问题"],
				[
					"Free, where files go, bringing old data, captions stay local",
					"免费、文件在哪、旧数据怎么带、字幕不上传",
				],
				["—", "—"],
				["—", "—"],
			],
		],
	);
	text("t-demos", "Demo timelines", "演示时间线");
	text(
		"l-demos",
		"Phases in milliseconds from the step's start; each loop then repeats. With reduced motion, or while the window is in the background, the demo shows its last phase and doodles appear complete.",
		"从进入这一步算起的毫秒数，然后循环。减少动态效果或窗口在后台时，演示显示最后一个阶段，手绘直接画完。",
	);
	table(
		"demo-table",
		[
			["Step", "步骤"],
			["Phases", "阶段"],
			["Loop", "循环"],
		],
		[
			[
				["Studio", "工作室"],
				[
					"0 bar · 600 recording 0:00 · 2600 Chapter marked · 4400 editor · 5000 playhead and caption · 6000 auto zoom · 7200 export · 7400 progress · 9200 Exported",
					"0 录制条 · 600 录制 0:00 · 2600 已打章节标记 · 4400 编辑器 · 5000 播放头和字幕 · 6000 自动缩放 · 7200 导出 · 7400 进度 · 9200 已导出",
				],
				"11000",
			],
			[
				["Instant", "即时"],
				[
					"0 bar · 600 recording · 3400 cursor to Stop · 4000 library with the new tile ticked · 5600 drag it out",
					"0 录制条 · 600 录制 · 3400 光标点停止 · 4000 资源库里新卡片打勾 · 5600 拖出去",
				],
				"8000",
			],
			[
				["Screenshot", "截图"],
				[
					"0 cursor · 500 selection with magnifier and size · 1700 shutter, editor · 2300 step 1 · 2900 step 2 · 3500 spotlight · 4100 bubble · 5000 Screenshot copied",
					"0 光标 · 500 框选带放大镜和尺寸 · 1700 快门、编辑器 · 2300 步骤 1 · 2900 步骤 2 · 3500 聚光 · 4100 气泡 · 5000 截图已复制",
				],
				"6800",
			],
			[
				["Library", "资源库"],
				[
					"0 compact window, Library highlighted · 900 expands · 2200 select · 2700 Space preview · 4200 drag out · 5600 Ctrl+K · 6600 typed",
					"0 小窗，资源库高亮 · 900 展开 · 2200 选中 · 2700 空格预览 · 4200 拖出 · 5600 Ctrl+K · 6600 输入",
				],
				"8400",
			],
		],
	);
	text("t-rules", "Rules", "规则");
	text(
		"l-rules",
		"Decided with the owner on 2026-11-15.",
		"2026-11-15 和 owner 定下。",
	);
	table(
		"rule-table",
		[
			["Rule", "规则"],
			["Detail", "说明"],
		],
		[
			[
				["Shortcuts are written on Continue", "点继续才写入快捷键"],
				[
					"Leaving step 6 with Continue registers both rows. Back keeps the edits without writing. Skip the tour, on any step, writes nothing: no record or screenshot shortcut is set by default.",
					"在第 6 步点“继续”才注册两行快捷键。“上一步”保留修改但不写入。任何一步“跳过引导”都不写入：录制和截图默认没有快捷键。",
				],
			],
			[
				["Suggested keys", "建议键"],
				[
					"First run fills Alt+Shift+R and Alt+Shift+S. Not Print Screen, which Windows 11 gives to Snipping Tool.",
					"首次打开预填 Alt+Shift+R 和 Alt+Shift+S。不用 Print Screen，Windows 11 把它给了截图工具。",
				],
			],
			[
				["Taken keys", "被占用的键"],
				[
					"If registering fails, the row says so and is left empty; Continue still moves on. A clash with another FrameEver action moves it to this row with a note, as Settings does.",
					"注册失败时这一行显示被占用并清空，“继续”照样往下走。和帧刻自己的其他操作冲突时，像设置里一样移到这一行并提示。",
				],
			],
			[
				["Storage folder", "存储文件夹"],
				[
					"Change… opens the folder picker and writes storagePath at once, as Settings → Storage does.",
					"“更改…”打开文件夹选择，立刻写入 storagePath，和 设置 → 存储 一样。",
				],
			],
			[
				["Opened again", "重看引导"],
				[
					"Help in the compact window's title bar and the command palette gain Show onboarding again. It skips the welcome page, starts at step 1 and shows the current shortcuts with no suggestions.",
					"小窗标题栏的“帮助”和命令面板加“重看引导”。跳过欢迎页，从第 1 步开始，显示当前快捷键，不预填建议键。",
				],
			],
			[
				["Finish", "结束"],
				[
					"Start using closes onboarding and shows the compact main window with its first-run line. Nothing starts recording.",
					"“开始使用”关闭引导，打开小窗，底栏显示首次提示。不会自动开始录制。",
				],
			],
			[
				["Copy", "文案"],
				[
					"Sentence case, no exclamation marks, the error is Couldn't save your progress. Try again. (docs/copy-style.md).",
					"句首大写，不用感叹号，错误提示是“无法保存进度。再试一次。”（docs/copy-style.md）。",
				],
			],
		],
	);
	text("t-layout", "Layout", "布局");
	text(
		"l-layout",
		"Today every step sits at the top with empty space below, because the content's min-height has no definite height inside the scroll container.",
		"现在每一步都贴在顶部、下面空一大块，因为内容的 min-height 在滚动容器里没有确定高度。",
	);
	table(
		"layout-table",
		[
			["Element", "元素"],
			["Today", "现在"],
			["Should be", "应该"],
		],
		[
			[
				"<code>onboarding-scroll</code>",
				"<code>flex_1 · overflow_y_scroll</code>",
				"<code>flex_1 · min_h_0 · flex · flex_col · overflow_y_scroll</code>",
			],
			[
				["Step wrapper", "步骤外层"],
				"<code>min_h_full</code>",
				"<code>flex_grow · flex_shrink_0</code>",
			],
			[
				["Step content", "步骤内容"],
				"<code>justify_center · min_h_full</code>",
				[
					"<code>justify_center</code>, taller content scrolls",
					"<code>justify_center</code>，内容更高时滚动",
				],
			],
		],
	);
	text("t-changes", "What changes from today", "和现在比改了什么");
	text(
		"l-changes",
		"Everything onboarding showed that no longer matches the app.",
		"引导里和现在的应用对不上的地方。",
	);
	table(
		"change-table",
		[
			["Today", "现在"],
			["New", "改成"],
		],
		[
			[
				[
					"Overview: three cards, empty space below",
					"总览：三张卡片，下面空白",
				],
				[
					"Modes: cards pick the recording mode, with the real mode row under them",
					"模式：卡片直接选录制模式，下面是真实的模式切换条",
				],
			],
			[
				[
					"Instant demo ends on Video saved! Recording.mp4 · Copy (Cap's share card)",
					"即时演示停在“Video saved! Recording.mp4 · Copy”（Cap 的分享卡）",
				],
				[
					"Ends in the library with the new tile ticked, then dragged out",
					"停在资源库，新卡片打勾，再拖出去",
				],
			],
			[
				[
					"Switch modes anytime: three 80 px round buttons",
					"切换模式：三个 80 px 圆按钮",
				],
				[
					"Folded into step 1; the library gets its own step",
					"并入第 1 步，资源库单独一步",
				],
			],
			[
				[
					"Make FrameEver yours: two generic cards",
					"Make FrameEver yours：两张空泛卡片",
				],
				[
					"Shortcuts: two real shortcut rows and the storage folder",
					"快捷键：两行真实快捷键加存储文件夹",
				],
			],
			[
				["FAQ: generic answers", "FAQ：空泛回答"],
				[
					"Four answers: free, files, Converter, captions",
					"四条：免费、文件、Converter、字幕",
				],
			],
			[
				["Studio and Screenshot lists from Cap", "工作室和截图的特性沿用 Cap"],
				[
					"Captions, chapter marks, magnifier, step numbers, spotlight",
					"字幕、章节标记、放大镜、步骤编号、聚光",
				],
			],
			[
				["Title Case, exclamation marks", "首字母大写、感叹号"],
				["Sentence case per copy style", "按 copy-style 句首大写"],
			],
			[
				["No way to see it again", "没法重看"],
				["Help menu and command palette", "帮助菜单和命令面板"],
			],
		],
	);
	text("t-ref", "References", "参考");
	text(
		"l-ref",
		"What other recorders do on first run, and what we took.",
		"其他录屏软件首次启动怎么做，我们借了什么。",
	);
	table(
		"ref-table",
		[
			["Product", "产品"],
			["First run", "首次启动"],
			["Taken", "借鉴"],
		],
		[
			[
				"Raycast",
				[
					"Sets its hotkey in onboarding and asks you to press it",
					"引导里设热键并让你按一下",
				],
				["Step 6 sets real shortcuts", "第 6 步设真实快捷键"],
			],
			[
				"CleanShot X",
				[
					"Asks to take over the screenshot keys; onboarding is mostly settings",
					"问要不要接管截图键，引导基本就是设置",
				],
				["Settable step, suggested keys", "可设置的一步、建议键"],
			],
			[
				"Snagit",
				[
					"Print Screen by default, with a Windows 11 Snipping Tool conflict",
					"默认 Print Screen，和 Windows 11 截图工具冲突",
				],
				["Avoid Print Screen", "避开 Print Screen"],
			],
			[
				"OBS Studio",
				[
					"One-question wizard that writes real settings, rerun from Tools",
					"一问一答的向导写入真实设置，可在工具菜单重跑",
				],
				["Show onboarding again", "重看引导"],
			],
			[
				"Loom, Screen Studio",
				[
					"Permissions, devices, then a first recording",
					"权限、设备，然后第一段录制",
				],
				["Short steps that show the real surfaces", "短步骤，画真实界面"],
			],
			[
				"ShareX",
				["No tour", "没有引导"],
				["Skip the tour stays one click", "跳过引导保持一键"],
			],
		],
	);
};

shell({
	here: "onboarding",
	depth: 1,
	sections: [
		["steps", "Steps", "步骤"],
		["teach", "What each teaches", "讲什么"],
		["demos", "Demos", "演示"],
		["rules", "Rules", "规则"],
		["layout", "Layout", "布局"],
		["changes", "Changes", "改动"],
		["references", "References", "参考"],
	],
});
onChange(() => {
	sections();
	render();
	runDemo();
});
sections();
render();
runDemo();
