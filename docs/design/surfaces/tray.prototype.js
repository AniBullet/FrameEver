const { L, icon, pick, shell, onChange } = FE;

const VARIANTS = {
	A: ["Native menu", "Windows 原生菜单"],
	B: ["Klein Field menu", "Klein Field 菜单"],
	C: ["Control flyout", "控制浮层"],
};
const STATES = [
	["idle", "Idle", "空闲"],
	["countdown", "Countdown", "倒计时"],
	["recording", "Recording", "录制中"],
	["paused", "Paused", "已暂停"],
	["exporting", "Exporting", "导出中"],
	["setup", "First run", "首次运行"],
];
const MODES = [
	["studio", "Studio", "Studio"],
	["instant", "Instant", "即时"],
	["screenshot", "Screenshot", "截图"],
];
const RECENT = [
	["Product demo", "产品演示", "clapperboard", "Studio"],
	["Bug report", "问题复现", "zap", "Instant"],
	["Pricing page", "定价页面", "image", "Screenshot"],
	["Weekly update", "每周更新", "clapperboard", "Studio"],
	["Login flow", "登录流程", "zap", "Instant"],
	["Settings panel", "设置面板", "image", "Screenshot"],
];

const initial = () => new URLSearchParams(location.search).get("variant");
const s = {
	variant: VARIANTS[initial()] ? initial() : "B",
	st: "idle", mode: "studio", win: "dark", set: "a", recent: true, last: true,
	open: null, sub: null, hidden: false, log: ["Press left click or right click on the tray icon.", "点左键或右键试试托盘图标。"],
};

const RED = "#FF3B30";
const KLEIN = "#1F3FD8";
const NAVY = "#000F5C";
const AMBER = "#F5A524";
const plate = (fill) => `<rect width="48" height="48" rx="10.8" fill="${fill}"/>`;
const ring = (stroke, r = 15) => `<circle cx="24" cy="24" r="${r}" fill="none" stroke="${stroke}" stroke-width="5"/>`;
const square = (fill, half = 8) => `<rect x="${24 - half}" y="${24 - half}" width="${half * 2}" height="${half * 2}" rx="2.5" fill="${fill}"/>`;
const bars = (fill) => `<rect x="15" y="13" width="7" height="22" rx="2" fill="${fill}"/><rect x="26" y="13" width="7" height="22" rx="2" fill="${fill}"/>`;
const arc = (pct, stroke) => {
	const len = 2 * Math.PI * 15;
	return `<circle cx="24" cy="24" r="15" fill="none" stroke="${stroke}" stroke-opacity="0.3" stroke-width="5"/><circle cx="24" cy="24" r="15" fill="none" stroke="${stroke}" stroke-width="5" stroke-linecap="round" stroke-dasharray="${(len * pct) / 100} ${len}" transform="rotate(-90 24 24)"/>`;
};
const logo = () => `${plate(KLEIN)}<rect x="9" y="25" width="30" height="7" rx="3.5" fill="#FFFDED"/><rect x="9" y="34" width="17" height="7" rx="3.5" fill="#87BFFF"/><circle cx="30.5" cy="13.5" r="9.5" fill="#fff"/><circle cx="30.5" cy="13.5" r="6.1" fill="${RED}"/>`;

const shapes = {
	a: { recording: () => plate(KLEIN) + ring("#fff") + square(RED), paused: () => plate(KLEIN) + ring("#fff") + bars(AMBER).replace(/x="15"/, 'x="17"').replace(/x="26"/, 'x="24"').replace(/width="7"/g, 'width="6"').replace(/height="22"/g, 'height="14"').replace(/y="13"/g, 'y="17"'), exporting: () => plate(KLEIN) + arc(62, "#fff") },
	b: { recording: () => plate(RED) + square("#fff", 11), paused: () => plate(AMBER) + bars("#1A1B24"), exporting: () => plate(KLEIN) + arc(62, "#fff") },
	c: { recording: (ink) => ring(ink, 20) + square(RED, 10), paused: (ink) => ring(ink, 20) + bars(AMBER).replace(/x="15"/, 'x="16"').replace(/x="26"/, 'x="26"').replace(/width="7"/g, 'width="6"').replace(/height="22"/g, 'height="16"').replace(/y="13"/g, 'y="16"'), exporting: (ink) => `<g transform="translate(24 24) scale(1.33) translate(-24 -24)">${arc(62, ink)}</g>` },
};
const trayIcon = (state, set, size, dark) => {
	const ink = dark ? "#fff" : "#1b1b1b";
	const body = ["idle", "countdown", "setup"].includes(state) ? logo() : shapes[set][state](ink);
	return `<svg width="${size}" height="${size}" viewBox="0 0 48 48" aria-hidden="true">${body}</svg>`;
};

const label = () => {
	const m = MODES.find(([v]) => v === s.mode);
	return L(m[1], m[2]);
};
const verb = (t) =>
	s.mode === "screenshot"
		? { display: ["Screenshot display", "截取显示器"], window: ["Screenshot window", "截取窗口"], area: ["Screenshot area", "截取区域"] }[t]
		: { display: ["Record display", "录制显示器"], window: ["Record window", "录制窗口"], area: ["Record area", "录制区域"] }[t];

const idleRows = () => [
	{ t: ["Open FrameEver", "打开帧刻"], a: "open", bold: true },
	{ t: ["Open library", "打开资源库"], a: "library" },
	{ sep: 1 },
	{ modes: 1 },
	{ t: verb("display"), a: "display", ic: "monitor" },
	{ t: verb("window"), a: "window", ic: "app-window" },
	{ t: verb("area"), a: "area", ic: "box-select", key: "Ctrl Shift 4" },
	...(s.last ? [{ t: s.mode === "screenshot" ? ["Repeat last area", "重复上次区域"] : ["Record last area", "录制上次区域"], a: "last", ic: "rotate-ccw" }] : []),
	...(s.mode === "screenshot" ? [] : [{ t: ["Take a screenshot", "截图"], a: "shot", ic: "screenshot", key: "Ctrl Shift 2" }]),
	{ sep: 1 },
	...(s.recent ? [{ t: ["Recent", "最近"], sub: "recent", ic: "film" }] : []),
	{ t: ["Import media…", "导入媒体…"], a: "import", ic: "download" },
	{ t: ["Settings", "设置"], a: "settings", ic: "settings" },
	{ sep: 1 },
	{ t: ["Quit FrameEver", "退出帧刻"], a: "quit", ver: true },
];
const liveRows = () => {
	const hide = s.hidden ? ["Show controls", "显示控制条"] : ["Hide controls", "隐藏控制条"];
	if (s.st === "countdown") return [{ status: ["Starting in 3", "3 秒后开始"], dot: "" }, { sep: 1 }, { t: ["Cancel", "取消"], a: "cancel", bold: true, ic: "x" }];
	if (s.st === "paused")
		return [
			{ status: ["Paused · 1:23", "已暂停 · 1:23"], dot: "amber" },
			{ sep: 1 },
			{ t: ["Resume", "继续"], a: "resume", bold: true, ic: "play" },
			{ t: ["Stop recording", "停止录制"], a: "stop", ic: "square" },
			{ t: ["Restart…", "重新开始…"], a: "restart", ic: "rotate-ccw" },
			{ sep: 1 },
			{ t: hide, a: "hide", ic: "eye-off", key: "Ctrl Shift H" },
		];
	return [
		{ status: ["Recording · 1:23", "录制中 · 1:23"], dot: "rec" },
		{ sep: 1 },
		{ t: ["Stop recording", "停止录制"], a: "stop", bold: true, ic: "square" },
		{ t: ["Pause", "暂停"], a: "pause", ic: "pause" },
		{ t: ["Restart…", "重新开始…"], a: "restart", ic: "rotate-ccw" },
		{ sep: 1 },
		{ t: hide, a: "hide", ic: "eye-off", key: "Ctrl Shift H" },
	];
};
const rows = () => {
	if (s.st === "setup") return [{ t: ["Continue setup", "继续设置"], a: "setup", bold: true }, { sep: 1 }, { t: ["Quit FrameEver", "退出帧刻"], a: "quit", ver: true }];
	if (["recording", "paused", "countdown"].includes(s.st)) return liveRows();
	if (s.st === "exporting") return [{ exp: 1 }, { sep: 1 }, ...idleRows()];
	return idleRows();
};

const kbd = (text) => text.split(" ").map((k) => `<span class="kbd">${k}</span>`).join(" ");
const kleinRow = (r) => {
	if (r.sep) return "<hr>";
	if (r.status) return `<div class="status tnum">${r.dot === "" ? "" : `<i class="dot ${r.dot === "amber" ? "amber" : ""}"></i>`}<b>${L(...r.status)}</b></div>`;
	if (r.modes) return `<div class="pad"><div class="seg">${MODES.map(([v, en, zh]) => `<button class="${s.mode === v ? "on" : ""}" data-mode="${v}">${L(en, zh)}</button>`).join("")}</div></div>`;
	if (r.exp) return `<button data-a="export"><span class="grow tnum">${L("Exporting Product demo · 42%", "正在导出 产品演示 · 42%")}</span></button><div class="bar"><div class="progress"><i style="width:42%"></i></div></div>`;
	if (r.sub) return `<button data-sub="${r.sub}" class="${s.sub === r.sub ? "hover" : ""}">${icon(r.ic)}<span class="grow">${L(...r.t)}</span><span class="chev">${icon("chevron-right", "sm")}</span></button>`;
	return `<button data-a="${r.a}" class="${r.bold ? "bold" : ""}">${r.ic ? icon(r.ic) : ""}<span class="grow">${L(...r.t)}</span>${r.key ? kbd(r.key) : ""}${r.ver ? '<span class="ver tnum">1.0.0</span>' : ""}</button>`;
};
const kleinMenu = () => `<div class="menu tm" role="menu">${rows().map(kleinRow).join("")}</div>`;
const kleinSub = () => `<div class="menu tm" role="menu">${RECENT.map(([en, zh, ic]) => `<button data-a="recent"><span class="thumb">${icon(ic, "sm")}</span><span class="grow">${L(en, zh)}</span></button>`).join("")}<hr><button data-a="library"><span class="grow">${L("All in library", "在资源库里查看全部")}</span></button></div>`;

const nativeRow = (r) => {
	if (r.sep) return "<hr>";
	if (r.status) return `<button disabled class="tnum">${L(...r.status)}</button>`;
	if (r.exp) return `<button data-a="export" class="tnum">${L("Exporting Product demo · 42%", "正在导出 产品演示 · 42%")}</button>`;
	if (r.modes) return `<button data-sub="mode"><span>${pick("Mode", "模式")}: ${label()}</span><span class="right">›</span></button>`;
	if (r.sub) return `<button data-sub="${r.sub}"><span>${L(...r.t)}</span><span class="right">›</span></button>`;
	return `<button data-a="${r.a}" class="${r.bold ? "bold" : ""}"><span>${L(...r.t)}</span>${r.key ? `<span class="right">${r.key.replaceAll(" ", "+")}</span>` : ""}</button>`;
};
const shortRows = () =>
	s.st === "setup"
		? rows()
		: [{ t: ["Open FrameEver", "打开帧刻"], a: "open", bold: true }, { t: ["Open library", "打开资源库"], a: "library" }, { t: ["Settings", "设置"], a: "settings" }, { sep: 1 }, ...(["recording", "paused", "countdown"].includes(s.st) ? [] : [{ t: ["Quit FrameEver", "退出帧刻"], a: "quit" }])];
const nativeMenu = (list = rows()) => `<div class="nm" role="menu">${list.map(nativeRow).join("")}</div>`;
const nativeSub = () =>
	s.sub === "mode"
		? `<div class="nm" role="menu">${MODES.map(([v, en, zh]) => `<button data-mode="${v}"><span class="gut">${s.mode === v ? "✓" : ""}</span><span>${L(en, zh)}</span></button>`).join("")}</div>`
		: `<div class="nm" role="menu">${RECENT.map(([en, zh]) => `<button data-a="recent"><span class="native-thumb"></span><span>${L(en, zh)}</span></button>`).join("")}</div>`;

const flyout = () => {
	const foot = `<div class="foot"><button class="btn" data-a="library">${icon("layout-grid")}${L("Library", "资源库")}</button><button class="btn" data-a="settings">${icon("settings")}${L("Settings", "设置")}</button></div>`;
	if (["recording", "paused", "countdown"].includes(s.st)) {
		const paused = s.st === "paused";
		if (s.st === "countdown") return `<div class="fly"><div class="head"><b>${L("Starting in 3", "3 秒后开始")}</b></div><div class="foot"><button class="btn" data-a="cancel">${L("Cancel", "取消")}</button></div></div>`;
		return `<div class="fly"><div class="head"><i class="dot ${paused ? "amber" : ""}"></i><b>${paused ? L("Paused", "已暂停") : L("Recording", "录制中")}</b><span class="dim">${label()}</span></div><div class="time tnum">1:23</div><div class="foot" style="margin:10px 0 6px"><button class="btn primary" data-a="stop">${L("Stop", "停止")}</button><button class="btn" data-a="${paused ? "resume" : "pause"}">${paused ? L("Resume", "继续") : L("Pause", "暂停")}</button><button class="btn" data-a="restart">${L("Restart", "重新开始")}</button></div><div class="foot"><button class="btn" data-a="hide">${s.hidden ? L("Show controls", "显示控制条") : L("Hide controls", "隐藏控制条")}</button></div></div>`;
	}
	if (s.st === "setup") return `<div class="fly"><div class="head"><b>${L("Finish setting up", "完成设置")}</b></div><div class="foot"><button class="btn primary" data-a="setup">${L("Continue setup", "继续设置")}</button></div></div>`;
	const exportRow = s.st === "exporting" ? `<div class="label tnum">${L("Exporting Product demo · 42%", "正在导出 产品演示 · 42%")}</div><div class="progress" style="margin:6px 0 10px"><i style="width:42%"></i></div>` : "";
	const strip = s.recent ? `<div class="label">${L("Recent", "最近")}</div><div class="strip">${RECENT.slice(0, 4).map(([en, zh, ic]) => `<button data-a="recent" title="${pick(en, zh)}">${icon(ic, "sm")}</button>`).join("")}</div>` : "";
	return `<div class="fly"><div class="head"><b>FrameEver</b><span class="dim">${L("Ready", "就绪")}</span></div>${exportRow}<div class="seg">${MODES.map(([v, en, zh]) => `<button class="${s.mode === v ? "on" : ""}" data-mode="${v}">${L(en, zh)}</button>`).join("")}</div><div class="tiles"><button data-a="display">${icon("monitor", "lg")}${L("Display", "显示器")}</button><button data-a="window">${icon("app-window", "lg")}${L("Window", "窗口")}</button><button data-a="area">${icon("box-select", "lg")}${L("Area", "区域")}</button></div>${strip}${foot}</div>`;
};

const popup = () => {
	if (!s.open) return "";
	const narrow = s.variant !== "B";
	let body = "";
	if (s.open === "flyout") body = flyout();
	if (s.open === "menu" && s.variant === "A") body = (s.sub ? nativeSub() : "") + nativeMenu();
	if (s.open === "menu" && s.variant === "B") body = (s.sub ? kleinSub() : "") + kleinMenu();
	if (s.open === "menu" && s.variant === "C") body = nativeMenu(shortRows());
	return `<div class="anchor ${s.sub ? "sub-open" : ""}" data-narrow="${narrow}">${body}</div>`;
};

const seg = (name, value, options) => `<div class="seg" data-tool="${name}">${options.map(([v, en, zh]) => `<button class="${value === v ? "on" : ""}" data-v="${v}">${zh === undefined ? en : L(en, zh)}</button>`).join("")}</div>`;
const tool = (en, zh, html) => `<div class="tool"><span>${L(en, zh)}</span>${html}</div>`;

const compare = [
	[["Left click, idle", "空闲时左键"], ["Opens the compact main window", "打开紧凑主窗口"], ["Opens the compact main window", "打开紧凑主窗口"], ["Opens this flyout", "打开这个浮层"]],
	[["Left click, recording", "录制时左键"], ["Stops, as today", "停止，和现在一样"], ["Stops, as today", "停止，和现在一样"], ["Opens the flyout; Stop is one more click", "打开浮层，再点一次才停止"]],
	[["Right click", "右键"], ["This menu", "这个菜单"], ["This menu", "这个菜单"], ["A short native menu", "一个很短的原生菜单"]],
	[["Look", "外观"], ["Windows': Segoe UI, system colours. No Inter or HarmonyOS", "Windows 的样子：Segoe UI 和系统颜色，用不了 Inter 和 HarmonyOS"], ["Our tokens and fonts, same as the app", "我们的 token 和字体，和应用一致"], ["Ours on left click, Windows' on right click", "左键是我们的，右键是 Windows 的"]],
	[["Theme", "主题"], ["Our code never asks for a dark menu, and the only way to ask is an undocumented uxtheme call. Can't follow the app's own theme", "我们的代码没有请求深色菜单，能请求的只有未公开的 uxtheme 调用。也无法跟随应用自己的主题设置"], ["Follows the app", "跟随应用"], ["Mixed", "两种混在一起"]],
	[["Seen in a recording", "会不会录进去"], ["Yes: FrameEver can't exclude a system menu from capture", "会：帧刻无法把系统菜单排除在录制外"], ["No: it's our window, so WDA_EXCLUDEFROMCAPTURE applies", "不会：这是我们自己的窗口，可以用 WDA_EXCLUDEFROMCAPTURE"], ["Flyout no, right-click menu yes", "浮层不会，右键菜单会"]],
	[["Mode switch", "切换模式"], ["A submenu, one more step", "子菜单，多一步"], ["Inline segments", "直接分段切换"], ["Inline segments", "直接分段切换"]],
	[["Overlap with the compact window", "和紧凑窗口重复"], ["None", "没有"], ["Small", "很小"], ["Large: it's a second compact window", "很大：等于第二个紧凑窗口"]],
	[["Build", "实现"], ["Smallest. Keyboard and dismiss come free", "最小，键盘和点外面关闭都是现成的"], ["A popup window, position from the tray rect, dismiss, keyboard, submenu. Native menu stays as fallback", "一个弹出窗口，要做定位、点外面关闭、键盘和子菜单。原生菜单留作回退"], ["Both of the others", "两边都要做"]],
];

const render = () => {
	const dark = s.win === "dark";
	document.getElementById("root").innerHTML = `
		<div class="tools">
			${tool("State", "状态", seg("st", s.st, STATES))}
			${tool("Mode", "模式", seg("mode", s.mode, MODES))}
			${tool("Windows theme", "Windows 主题", seg("win", s.win, [["dark", "Dark", "深色"], ["light", "Light", "浅色"]]))}
			${tool("Icon set", "图标组", seg("set", s.set, [["a", "1 Stop button", "1 停止按钮"], ["b", "2 Status plate", "2 状态色底"], ["c", "3 No plate", "3 无底板"]]))}
			${tool("Recent items", "最近项目", seg("recent", s.recent ? "y" : "n", [["y", "Some", "有"], ["n", "None", "没有"]]))}
		</div>
		<div class="stage win-${s.win}" id="stage">
			${popup()}
			<div class="taskbar">
				<button id="tray" aria-label="${pick("FrameEver in the tray", "托盘里的帧刻")}">${trayIcon(s.st, s.set, 20, dark)}</button>
				<div class="clock tnum">14:32<br>2026/10/31</div>
			</div>
		</div>
		<div class="clicks"><button class="btn" data-click="left">${L("Left click the icon", "左键点图标")}</button><button class="btn" data-click="right">${L("Right click the icon", "右键点图标")}</button><button class="btn ghost" data-click="close">${L("Click elsewhere", "点别处")}</button></div>
		<p class="log"><b>${L("Result", "结果")}:</b> ${L(...s.log)}</p>
		<h2>${L("Icon states at real size", "图标状态，真实大小")}</h2>
		<p class="note">${L("Idle, recording, paused, exporting at 16, 20 and 24 px on both taskbars. Nothing blinks. Exporting steps through eight frames at most. The real frames get drawn pixel by pixel, as the app icon's are.", "空闲、录制中、已暂停、导出中，16、20、24 px，两种任务栏。没有闪烁。导出最多走八帧。真实图标会像应用图标一样逐像素画。")}</p>
		<div class="icons">${[["a", "1 Stop button", "1 停止按钮"], ["b", "2 Status plate", "2 状态色底"], ["c", "3 No plate", "3 无底板"]].map(([set, en, zh]) => `<div class="iset"><h3>${L(en, zh)}</h3>${["dark", "light"].map((bar) => `<div class="strip-bar ${bar}">${["idle", "recording", "paused", "exporting"].map((st) => [16, 20, 24].map((px) => trayIcon(st, set, px, bar === "dark")).join("")).join("")}</div>`).join("")}</div>`).join("")}</div>
		<h2>${L("Comparison", "对比")}</h2>
		<div class="wrap"><table class="cmp"><tr><th></th><th>A ${L(...VARIANTS.A)}</th><th>B ${L(...VARIANTS.B)}</th><th>C ${L(...VARIANTS.C)}</th></tr>${compare.map(([k, a, b, c]) => `<tr><th>${L(...k)}</th><td>${L(...a)}</td><td>${L(...b)}</td><td>${L(...c)}</td></tr>`).join("")}</table></div>`;
	const name = VARIANTS[s.variant];
	document.getElementById("switcher").innerHTML = `<button data-go="-1" aria-label="Previous">←</button><span>${s.variant} · ${pick(...name)}</span><button data-go="1" aria-label="Next">→</button>`;
};

const results = {
	open: ["Compact main window opens.", "打开紧凑主窗口。"],
	library: ["The main window opens expanded on the library.", "主窗口展开到资源库。"],
	display: ["Target armed: display.", "选好目标：显示器。"],
	window: ["Window picker opens.", "打开窗口选择。"],
	area: ["Area picker opens.", "打开区域选择。"],
	last: ["Starts from the last area, with no picker.", "直接用上次的区域开始，不打开选择器。"],
	shot: ["Screenshot of the display under the cursor.", "截取光标所在的显示器。"],
	recent: ["The library opens with that project selected.", "打开资源库并选中这个项目。"],
	import: ["File picker opens.", "打开文件选择。"],
	settings: ["Settings opens.", "打开设置。"],
	quit: ["FrameEver quits.", "帧刻退出。"],
	setup: ["Onboarding opens.", "打开引导。"],
	export: ["The export page comes to the front.", "导出页面来到前面。"],
	restart: ["The bar's Restart confirmation shows, and the controls appear if hidden.", "显示控制条的重新开始确认，控制条隐藏时会先显示。"],
};
const act = (a) => {
	s.open = null;
	s.sub = null;
	if (a === "stop") { s.st = "idle"; s.log = ["Recording stops and is saved.", "录制停止并保存。"]; return; }
	if (a === "cancel") { s.st = "idle"; s.log = ["Countdown cancelled.", "倒计时已取消。"]; return; }
	if (a === "pause") { s.st = "paused"; s.log = ["Paused. The icon changes.", "已暂停，图标跟着变。"]; return; }
	if (a === "resume") { s.st = "recording"; s.log = ["Recording again.", "继续录制。"]; return; }
	if (a === "hide") { s.hidden = !s.hidden; s.log = s.hidden ? ["Controls hidden.", "控制条已隐藏。"] : ["Controls back.", "控制条已恢复。"]; return; }
	s.log = results[a];
};
const leftClick = () => {
	if (s.variant === "C" && s.st !== "setup") { s.open = s.open === "flyout" ? null : "flyout"; s.log = ["Flyout toggles.", "浮层开关。"]; return; }
	if (s.st === "recording" || s.st === "paused") return act("stop");
	if (s.st === "countdown") return act("cancel");
	if (s.st === "exporting") return act("export");
	if (s.st === "setup") return act("setup");
	return act("open");
};

document.addEventListener("click", (event) => {
	const tgt = event.target;
	const tv = tgt.closest("[data-tool] [data-v]");
	if (tv) {
		const name = tv.parentElement.dataset.tool;
		const v = tv.dataset.v;
		if (name === "recent") s.recent = v === "y";
		else s[name] = v;
		s.sub = null;
		render();
		return;
	}
	const mode = tgt.closest("[data-mode]");
	if (mode) {
		s.mode = mode.dataset.mode;
		if (s.variant === "A") { s.open = null; s.sub = null; }
		s.log = ["Mode changes; the capture rows follow.", "模式切换，下面的捕获项跟着变。"];
		render();
		return;
	}
	const sub = tgt.closest("[data-sub]");
	if (sub) {
		s.sub = s.sub === sub.dataset.sub ? null : sub.dataset.sub;
		render();
		return;
	}
	const a = tgt.closest("[data-a]");
	if (a) {
		act(a.dataset.a);
		render();
		return;
	}
	const go = tgt.closest("[data-go]");
	if (go) return cycle(Number(go.dataset.go));
	if (tgt.closest("#tray")) {
		leftClick();
		render();
		return;
	}
	const click = tgt.closest("[data-click]");
	if (click) {
		if (click.dataset.click === "left") leftClick();
		if (click.dataset.click === "right") {
			s.open = "menu";
			s.sub = null;
			s.log = ["Menu opens above the icon.", "菜单在图标上方打开。"];
		}
		if (click.dataset.click === "close") {
			s.open = null;
			s.sub = null;
		}
		render();
	}
});
document.addEventListener("contextmenu", (event) => {
	if (!event.target.closest("#tray")) return;
	event.preventDefault();
	s.open = "menu";
	s.sub = null;
	render();
});
const cycle = (by) => {
	const list = Object.keys(VARIANTS);
	s.variant = list[(list.indexOf(s.variant) + by + list.length) % list.length];
	s.open = null;
	s.sub = null;
	try {
		history.replaceState(null, "", `?variant=${s.variant}${location.hash}`);
	} catch {}
	render();
};
document.addEventListener("keydown", (event) => {
	if (event.target.closest("input,textarea")) return;
	if (event.key === "ArrowLeft") cycle(-1);
	if (event.key === "ArrowRight") cycle(1);
});

shell({ here: "tray", depth: 1 });
onChange(render);
render();
