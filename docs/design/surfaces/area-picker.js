const { L, icon, pick, shell, onChange, recordGlyph } = FE;

const W = 1280;
const H = 720;
const WORK = 680;
const PX = 2;
const MIN = 150;
const SNAP = 7;
const GRAB = 18;

const wins = [
	{
		id: "notes",
		en: "Notes",
		zh: "笔记",
		x: 96,
		y: 56,
		w: 620,
		h: 440,
		cls: "",
	},
	{
		id: "browser",
		en: "Browser",
		zh: "浏览器",
		x: 420,
		y: 136,
		w: 760,
		h: 500,
		cls: "b",
	},
];
const ratios = [
	["Free", "自由", null],
	["16:9", "16:9", 16 / 9],
	["4:3", "4:3", 4 / 3],
	["1:1", "1:1", 1],
	["9:16", "9:16", 9 / 16],
	["16:10", "16:10", 1.6],
	["21:9", "21:9", 21 / 9],
	["3:2", "3:2", 1.5],
	["2:1", "2:1", 2],
];
const sizes = [
	["720p", 1280, 720],
	["1080p", 1920, 1080],
	["1440p", 2560, 1440],
];
const MICS = ["Default · Realtek Audio", "USB Microphone"];
const CAMS = ["Logitech C920", "OBS Virtual Camera"];

const s = {
	target: "area",
	mode: "studio",
	crop: { x: 300, y: 130, w: 640, h: 360 },
	ratio: null,
	locked: false,
	drag: null,
	guides: [],
	hover: null,
	pinned: null,
	phase: "pick",
	count: 3,
	countdown: 3,
	sec: 0,
	menu: null,
	focusWin: false,
	center: false,
	cam: -1,
	mic: 0,
};

const round = (r) => ({
	x: Math.round(r.x),
	y: Math.round(r.y),
	w: Math.round(r.w),
	h: Math.round(r.h),
});
const size = (r) => `${r.w * PX} × ${r.h * PX}`;
const valid = (st, r) =>
	!!r &&
	(st.mode === "screenshot" ? r.w >= 1 && r.h >= 1 : r.w >= MIN && r.h >= MIN);
const winOf = (id) => wins.find((w) => w.id === id) ?? null;
const rectOf = (st) => {
	if (st.target === "display") return { x: 0, y: 0, w: W, h: H };
	if (st.target === "window") return winOf(st.pinned ?? st.hover);
	return st.crop;
};
const ready = (st) =>
	st.target === "area" ? valid(st, st.crop) : !!rectOf(st);
const windowAt = (p) =>
	[...wins]
		.reverse()
		.find(
			(w) => p.x >= w.x && p.x <= w.x + w.w && p.y >= w.y && p.y <= w.y + w.h,
		) ?? null;
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
const keys = (...list) =>
	list.map((key) => `<span class="kbd">${key}</span>`).join(" ");
const sep = () => `<span class="rb-sep"></span>`;
const box = (r, extra = "") =>
	`left:${r.x}px;top:${r.y}px;width:${r.w}px;height:${r.h}px;${extra}`;

const wallHTML = `<div class="wall">${wins.map((w) => `<div class="wall-win ${w.cls}" style="${box(w)}"><header></header><p style="left:24px;top:56px;width:40%;height:14px;background:#777"></p><p style="left:24px;top:88px;width:80%;height:9px"></p><p style="left:24px;top:108px;width:72%;height:9px"></p><p style="left:24px;top:128px;width:76%;height:9px"></p><p style="left:24px;top:164px;width:44%;height:42%"></p><p style="left:50%;top:164px;width:44%;height:42%;background:#b0b0b0"></p></div>`).join("")}<div class="wall-bar"></div></div>`;

const scrim = (t) => {
	if (!t) return `<div class="sc full"></div>`;
	return `<div class="sc" style="left:0;top:0;width:${t.x}px;height:${H}px"></div><div class="sc" style="left:${t.x + t.w}px;top:0;width:${W - t.x - t.w}px;height:${H}px"></div><div class="sc" style="left:${t.x}px;top:0;width:${t.w}px;height:${t.y}px"></div><div class="sc" style="left:${t.x}px;top:${t.y + t.h}px;width:${t.w}px;height:${H - t.y - t.h}px"></div>`;
};

const frame = (st, t, editable) => {
	const moving = st.drag?.kind === "move";
	const bad = st.target === "area" && st.drag && !valid(st, t);
	const handles = editable
		? ["nw", "ne", "sw", "se"].map((h) => `<i class="h c ${h}"></i>`).join("") +
			["n", "s", "w", "e"].map((h) => `<i class="h ${h}"></i>`).join("")
		: "";
	return `<div class="frame ${moving ? "moving" : ""} ${bad ? "bad" : ""}" style="${box(t)}">${handles}</div>`;
};

const thirds = (t) =>
	[1, 2]
		.map(
			(n) =>
				`<div class="thirds" style="left:${t.x + (t.w * n) / 3}px;top:${t.y}px;width:1px;height:${t.h}px"></div><div class="thirds" style="left:${t.x}px;top:${t.y + (t.h * n) / 3}px;width:${t.w}px;height:1px"></div>`,
		)
		.join("");

const guides = (st) =>
	st.guides
		.map((g) =>
			g.axis === "v"
				? `<div class="guide" style="left:${g.pos}px;top:0;width:1px;height:${H}px"></div>`
				: `<div class="guide" style="left:0;top:${g.pos}px;height:1px;width:${W}px"></div>`,
		)
		.join("");

const ratioLabel = (st) => {
	const hit = ratios.find((r) => r[2] === st.ratio) ?? ratios[0];
	return L(hit[0], hit[1]);
};

const tagAt = (t) =>
	t.y >= 32
		? `left:${t.x}px;top:${t.y - 30}px`
		: `left:${t.x + 8}px;top:${t.y + 8}px`;

const readout = (st, t) => {
	if (st.target === "display")
		return `<div class="badge tnum" style="left:12px;top:12px">${L("Display 1", "显示器 1")} · 2560 × 1440 · 60 FPS</div>`;
	if (st.target === "window")
		return t
			? `<div class="badge tnum" style="${tagAt(t)}">${L(t.en, t.zh)} · ${size(t)}</div>`
			: "";
	if (!t) return "";
	const tooSmall = !valid(st, t);
	if (st.drag) {
		const below = t.y + t.h + 32 <= H;
		const pos = `right:${W - t.x - t.w}px;top:${below ? t.y + t.h + 8 : t.y + t.h - 32}px`;
		return `<div class="badge tnum ${tooSmall ? "bad" : ""}" style="${pos}">${size(t)}</div>`;
	}
	if (tooSmall)
		return `<div class="badge bad tnum" style="${tagAt(t)}">${L(`Too small. At least ${MIN * PX} × ${MIN * PX}`, `选区太小，至少 ${MIN * PX} × ${MIN * PX}`)}</div>`;
	return `<div class="badge tnum" style="${tagAt(t)}">${size(t)}${st.ratio ? ` · ${ratioLabel(st)}` : ""}</div>`;
};

const lockBadge = (st, t) =>
	st.locked
		? `<button class="badge ui" data-a="lock" style="left:${t.x + 8}px;top:${t.y + 8}px" title="${pick("Unlock area  L", "解锁区域  L")}">${icon("lock", "sm")}${L("Locked", "已锁定")}</button>`
		: "";

const hintCard = (st) => {
	const shot = st.mode === "screenshot";
	return `<div class="hint-card"><b>${L("Drag to select an area", "拖动选择区域")}</b><span>${L("Or click a window to fit it", "也可以点击窗口直接贴合")}</span><div class="hint-keys">${keys("Ctrl", "A")} ${L("Whole display", "整个屏幕")} ${keys("Esc")} ${shot ? L("Cancel", "取消") : L("Close", "关闭")}</div></div>`;
};

const tbBtn = (a, ic, title, cls = "", extra = "") =>
	`<button class="tb-btn ${cls}" data-a="${a}" title="${title}" aria-label="${title}" ${extra}>${icon(ic)}</button>`;
const closeBtn = () => tbBtn("close", "x", pick("Close  Esc", "关闭  Esc"));
const targetSeg = (st) =>
	`<div class="tb-seg" role="radiogroup">${[
		["display", "monitor", "Display", "显示器"],
		["window", "app-window", "Window", "窗口"],
		["area", "scan", "Area", "区域"],
	]
		.map(([id, ic, en, zh]) =>
			tbBtn(
				`tgt`,
				ic,
				pick(en, zh),
				st.target === id ? "on" : "",
				`data-v="${id}"`,
			),
		)
		.join("")}</div>`;
const ratioBtn = (st) =>
	`<button class="tb-txt tnum" data-a="menu" data-v="ratio" title="${pick("Ratio and size", "比例和尺寸")}" ${st.locked ? "disabled" : ""}>${ratioLabel(st)}${icon("chevron-down", "sm")}</button>`;
const areaTools = (st) =>
	`${ratioBtn(st)}${tbBtn("clear", "rotate-ccw", pick("Clear  R", "清除选区  R"), "", st.locked || !st.crop ? "disabled" : "")}${tbBtn("fill", "maximize", pick("Fill display  F", "填满屏幕  F"), "", st.locked ? "disabled" : "")}${tbBtn("lock", st.locked ? "lock" : "lock-open", st.locked ? pick("Unlock area  L", "解锁区域  L") : pick("Lock area  L", "锁定区域  L"), st.locked ? "on" : "", valid(st, st.crop) ? "" : "disabled")}`;
const modeBtn = (st) =>
	`<button class="tb-txt" data-a="menu" data-v="mode" title="${pick("Mode", "模式")}">${icon({ studio: "clapperboard", instant: "zap", screenshot: "image" }[st.mode], "sm")}${icon("chevron-down", "sm")}</button>`;
const devices = (st) =>
	st.mode === "screenshot"
		? ""
		: `${tbBtn("menu", "webcam", st.cam < 0 ? pick("No camera", "无摄像头") : CAMS[st.cam], st.cam < 0 ? "" : "on", `data-v="cam"`)}<button class="tb-btn" data-a="menu" data-v="mic" title="${MICS[st.mic]}" aria-label="${pick("Microphone", "麦克风")}">${icon("mic")}<span class="rb-meter"><b></b></span></button>${tbBtn("menu", "sliders-horizontal", pick("Recording options", "录制选项"), "", `data-v="opts"`)}`;
const startLabel = (st) =>
	st.mode === "screenshot"
		? L("Take screenshot", "截图")
		: L("Start recording", "开始录制");
const startBtn = (st) =>
	`<button class="tb-start" data-a="start" ${ready(st) ? "" : "disabled"} title="${pick("Enter", "Enter")}">${recordGlyph()}${startLabel(st)}</button>`;

const targetTools = (st, t) => {
	if (st.target === "display")
		return `<span class="tb-info tnum">${L("Display 1", "显示器 1")} <em>2560 × 1440</em></span>`;
	return `<span class="tb-info tnum">${L(t.en, t.zh)} <em>${size(t)}</em></span><button class="tb-txt" data-a="menu" data-v="win" title="${pick("Window size and focus", "窗口尺寸和专注")}">${st.focusWin ? L("Focus on", "专注开") : L("Size", "尺寸")}${icon("chevron-down", "sm")}</button>`;
};

const barSpot = (st, t) => {
	if (st.target === "display")
		return `left:${W / 2}px;top:${WORK - 52}px;transform:translateX(-50%)`;
	const below = t.y + t.h + 44 <= WORK;
	const above = t.y - 74 >= 0;
	const top = below
		? t.y + t.h + 8
		: above
			? t.y - 44
			: Math.min(t.y + t.h, WORK) - 44;
	return `right:${Math.max(8, W - t.x - t.w)}px;top:${top}px`;
};

const bar = (st, t) => {
	if (st.drag) return "";
	const lead = `${closeBtn()}${targetSeg(st)}${sep()}`;
	const topBar = (body) =>
		`<div class="tb ui" style="left:50%;top:16px;transform:translateX(-50%)">${lead}${body}</div>`;
	if (st.target === "area" && !t)
		return topBar(`${ratioBtn(st)}${modeBtn(st)}`);
	if (st.target === "window" && !st.pinned)
		return topBar(
			`<span class="tb-info">${L("Click a window", "点击选定窗口")}</span>`,
		);
	const tools = devices(st);
	const inner = `${lead}${st.target === "area" ? areaTools(st) : targetTools(st, t)}${sep()}${modeBtn(st)}${tools}${tools ? sep() : ""}${startBtn(st)}`;
	return `<div class="tb ui anchored" style="${barSpot(st, t)}">${inner}</div>`;
};

const menuRow = (a, v, label, on, extra = "") =>
	`<button class="pop-row" data-a="${a}" data-v="${v}"><span class="grow">${label}</span>${extra}${on ? icon("check", "sm") : ""}</button>`;
const menuBody = (st) => {
	const id = st.menu?.id;
	if (id === "ratio") {
		return `${ratios.map((r, i) => menuRow("ratio", i, L(r[0], r[1]), st.ratio === r[2])).join("")}<hr><div class="pop-sub">${L("Exact size", "精确尺寸")}</div>${sizes.map(([n, w, h]) => menuRow("preset", `${w}x${h}`, `${n} · ${w} × ${h}`, st.crop?.w * PX === w && st.crop?.h * PX === h)).join("")}`;
	}
	if (id === "mode")
		return [
			["studio", "Studio", "工作室"],
			["instant", "Instant", "即时"],
			["screenshot", "Screenshot", "截图"],
		]
			.map(([v, en, zh]) => menuRow("set-mode", v, L(en, zh), st.mode === v))
			.join("");
	if (id === "cam")
		return `${menuRow("cam", -1, L("No camera", "无摄像头"), st.cam < 0)}${CAMS.map((c, i) => menuRow("cam", i, c, st.cam === i)).join("")}`;
	if (id === "mic")
		return MICS.map((c, i) => menuRow("mic", i, c, st.mic === i)).join("");
	if (id === "opts")
		return `<div class="pop-opt"><span class="grow">${L("Countdown", "倒计时")}</span><div class="seg">${[
			[0, L("Off", "关")],
			[3, "3"],
			[5, "5"],
			[10, "10"],
		]
			.map(
				([v, t]) =>
					`<button class="${st.countdown === v ? "on" : ""}" data-a="countdown" data-v="${v}">${t}</button>`,
			)
			.join(
				"",
			)}</div></div><hr>${menuRow("noop", 0, L("All recording settings", "全部录制设置"), false)}`;
	if (id === "win")
		return `<button class="pop-row" data-a="focus"><span class="grow">${L("Only this window", "只录这个窗口")}</span><span class="switch ${st.focusWin ? "on" : ""}"></span></button><button class="pop-row" data-a="center" ${st.focusWin ? "" : "disabled"}><span class="grow">${L("Center on screen", "移到屏幕中央")}</span><span class="switch ${st.center ? "on" : ""}"></span></button><hr><div class="pop-sub">${L("Resize the window", "调整窗口大小")}</div>${["16:9", "4:3", "1:1", "9:16"].map((r) => menuRow("noop", r, r, false)).join("")}${sizes
			.slice(0, 2)
			.map(([n, w, h]) => menuRow("noop", n, `${n} · ${w} × ${h}`, false))
			.join("")}`;
	return "";
};
const menu = (st) => {
	if (!st.menu) return "";
	const { x, top, bottom, id } = st.menu;
	const width = id === "opts" ? 280 : 240;
	const left = clamp(x, 8, W - width - 8);
	const pos = top > 360 ? `bottom:${H - top + 6}px` : `top:${bottom + 6}px`;
	return `<div class="pop ui" style="left:${left}px;width:${width}px;${pos}">${menuBody(st)}</div>`;
};

const fmt = (sec) =>
	`${Math.floor(sec / 60)}:${String(sec % 60).padStart(2, "0")}`;
const rbBtn = (a, ic, title, extra = "") =>
	`<button class="rb-btn" data-a="${a}" title="${title}" aria-label="${title}" ${extra}>${ic}</button>`;
const recBar = (st) => {
	const cd = st.phase === "countdown";
	const starting = st.phase === "starting";
	const off = cd || starting ? "disabled" : "";
	const time = cd
		? `<span class="rb-time quiet tnum">${L(`Starts in ${st.count}`, `${st.count} 秒后开始`)}</span>`
		: starting
			? `<span class="rb-time quiet">${icon("rotate-ccw", "sm rb-spin")}${L("Starting…", "正在开始…")}</span>`
			: `<span class="rb-time tnum"><i class="rb-dot"></i>${fmt(st.sec)}</span>`;
	const end = cd
		? rbBtn("cancel", icon("x"), pick("Cancel  Esc", "取消  Esc"))
		: `<button class="rb-stop" data-a="stop" title="${pick("Stop  Ctrl Shift S", "停止  Ctrl Shift S")}" ${starting ? "disabled" : ""}><span class="stop-glyph"></span></button>`;
	return `<div class="rec-at ui"><div class="rb" role="toolbar"><span class="rb-grip">${icon("grip-vertical", "sm")}</span>${time}<span class="rb-btn" style="cursor:default">${icon("mic")}<span class="rb-meter"><b></b></span></span>${rbBtn("noop", icon("pause", "solid"), pick("Pause", "暂停"), off)}${rbBtn("noop", icon("rotate-ccw"), pick("Restart", "重新开始"), off)}${sep()}${rbBtn("noop", icon("trash"), pick("Delete", "删除"), off)}${rbBtn("noop", icon("pencil"), pick("Draw on screen", "屏幕画笔"), off)}${rbBtn("noop", icon("sliders-horizontal"), pick("Recording options", "录制选项"))}${sep()}${end}</div></div>`;
};

const cdCaption = () =>
	`<div class="cd-cap">${keys("Esc")}${L("Cancel", "取消")}${keys("Enter")}${L("Start now", "立即开始")}</div>`;
const countdown = (st, t) => {
	const cx = t.x + t.w / 2;
	const visible = t.y + t.h > WORK - 70 ? WORK - 70 - t.y : t.h;
	const cy = t.y + Math.min(t.h, visible) / 2;
	const n = st.phase === "countdown" ? st.count : 0;
	return `<div class="cd" style="left:${cx}px;top:${cy}px"><div class="cd-mark"><div class="cd-ring">${n ? `<span class="cd-n" data-n="${n}">${n}</span>` : `<i class="cd-dot"></i>`}</div></div>${n ? cdCaption() : ""}</div>`;
};

const marks = (t) => {
	const corners = [
		["left", "top"],
		["right", "top"],
		["left", "bottom"],
		["right", "bottom"],
	];
	return `<div class="marks">${corners
		.map(([a, b]) => {
			const x = a === "left" ? t.x - 2 : t.x + t.w - 12;
			const y = b === "top" ? t.y - 2 : t.y + t.h - 12;
			return `<i style="left:${x}px;top:${y}px;border-${a}-width:2px;border-${b}-width:2px"></i>`;
		})
		.join("")}</div>`;
};

const overlay = (st) => {
	const t = rectOf(st);
	if (st.phase === "closed")
		return `<div class="closed ui"><button class="btn" data-a="reopen">${L("Open the picker again", "重新打开选择器")}</button></div>`;
	if (st.phase === "recording")
		return `${st.target !== "display" && t ? marks(t) : ""}${recBar(st)}`;
	const cd = st.phase === "countdown" || st.phase === "starting";
	let h = st.target === "display" ? "" : scrim(t);
	if (st.other)
		return `<div class="sc full"></div><div class="badge tnum" style="left:50%;top:50%;transform:translate(-50%,-50%);height:32px;font-size:16px">${L("Display 2", "显示器 2")}</div>`;
	if (t) h += frame(st, t, !cd && st.target === "area" && !st.locked);
	if (cd) return h + countdown(st, t) + recBar(st);
	if (st.drag && st.target === "area" && t) h += thirds(t);
	h += guides(st);
	if (st.target === "area" && st.hover && !st.drag && !st.locked) {
		const w = winOf(st.hover);
		if (w && !(t && st.hoverOutside === false))
			h += `<div class="candidate" style="${box(w)}"></div>`;
	}
	if (st.target === "area" && !t && !st.drag) h += hintCard(st);
	h += readout(st, t);
	if (st.target === "area" && t && !st.drag) h += lockBadge(st, t);
	h += bar(st, t);
	return h + menu(st);
};

const desk = document.getElementById("desk");
const stage = document.getElementById("stage");
const ov = document.getElementById("ov");
document.getElementById("wall").innerHTML = wallHTML;

const settle = (root) => {
	for (const tb of root.querySelectorAll(".tb.anchored")) {
		if (tb.offsetLeft >= 8) continue;
		tb.style.right = "auto";
		tb.style.left = "8px";
	}
};

const describe = () => {
	const t = rectOf(s);
	return `${s.phase}${s.phase === "countdown" ? ` ${s.count}` : ""} · ${s.mode} · ${s.target} · ${t ? `${t.x}, ${t.y}  ${size(t)} px` : "no target"} · ratio ${s.ratio ? s.ratio.toFixed(2) : "free"} · ${s.locked ? "locked" : "unlocked"} · snap ${s.guides.length} · countdown ${s.countdown} s`;
};

const paint = () => {
	const active = document.activeElement;
	const editing = active?.dataset?.f;
	desk.classList.toggle(
		"area",
		s.phase === "pick" && s.target === "area" && !s.locked,
	);
	if (!editing) {
		ov.innerHTML = overlay(s);
		settle(ov);
	}
	document.getElementById("state-line").textContent = describe();
};

const fit = () => {
	const k = stage.clientWidth / W;
	stage.style.setProperty("--k", k);
	for (const m of document.querySelectorAll(".mini")) {
		const inner = m.querySelector(":scope > .mini-in");
		if (inner) inner.style.transform = `scale(${m.clientWidth / W})`;
	}
};
new ResizeObserver(fit).observe(stage);

const point = (e) => {
	const r = desk.getBoundingClientRect();
	return {
		x: clamp(((e.clientX - r.left) / r.width) * W, 0, W),
		y: clamp(((e.clientY - r.top) / r.height) * H, 0, H),
	};
};

const zoneAt = (p) => {
	const c = s.crop;
	if (!c) return { kind: "draw" };
	const near = (a, b) => Math.abs(a - b) <= GRAB;
	const inX = p.x >= c.x - GRAB && p.x <= c.x + c.w + GRAB;
	const inY = p.y >= c.y - GRAB && p.y <= c.y + c.h + GRAB;
	if (!inX || !inY) return { kind: "draw" };
	const v = near(p.y, c.y) ? "n" : near(p.y, c.y + c.h) ? "s" : "";
	const h = near(p.x, c.x) ? "w" : near(p.x, c.x + c.w) ? "e" : "";
	if (v || h) return { kind: "resize", handle: v + h };
	if (p.x > c.x && p.x < c.x + c.w && p.y > c.y && p.y < c.y + c.h)
		return { kind: "move" };
	return { kind: "draw" };
};

const snapX = [0, W / 2, W, ...wins.flatMap((w) => [w.x, w.x + w.w])];
const snapY = [0, H / 2, WORK, H, ...wins.flatMap((w) => [w.y, w.y + w.h])];
const nearest = (v, list) => {
	let best = null;
	for (const c of list)
		if (
			Math.abs(c - v) <= SNAP &&
			(best === null || Math.abs(c - v) < Math.abs(best - v))
		)
			best = c;
	return best;
};
const snapPoint = (p, free, out) => {
	if (free) return p;
	const x = nearest(p.x, snapX);
	const y = nearest(p.y, snapY);
	if (x !== null) out.push({ axis: "v", pos: x });
	if (y !== null) out.push({ axis: "h", pos: y });
	return { x: x ?? p.x, y: y ?? p.y };
};

const applyDraw = (d, p, e, out) => {
	const q = snapPoint(p, e.ctrlKey, out);
	const r = s.ratio;
	const w = q.x - d.at.x;
	let h = q.y - d.at.y;
	if (r) h = (Math.abs(w) / r) * (h < 0 ? -1 : 1);
	const x = w < 0 ? d.at.x + w : d.at.x;
	const y = h < 0 ? d.at.y + h : d.at.y;
	s.crop = round({
		x: clamp(x, 0, W),
		y: clamp(y, 0, H),
		w: Math.min(Math.abs(w), W - clamp(x, 0, W)),
		h: Math.min(Math.abs(h), H - clamp(y, 0, H)),
	});
};

const applyMove = (d, p, e, out) => {
	const c = d.start;
	let x = c.x + p.x - d.at.x;
	let y = c.y + p.y - d.at.y;
	if (!e.ctrlKey) {
		for (const [edge, off] of [
			[x, 0],
			[x + c.w, c.w],
			[x + c.w / 2, c.w / 2],
		]) {
			const hit = nearest(edge, snapX);
			if (hit !== null) {
				x = hit - off;
				out.push({ axis: "v", pos: hit });
				break;
			}
		}
		for (const [edge, off] of [
			[y, 0],
			[y + c.h, c.h],
			[y + c.h / 2, c.h / 2],
		]) {
			const hit = nearest(edge, snapY);
			if (hit !== null) {
				y = hit - off;
				out.push({ axis: "h", pos: hit });
				break;
			}
		}
	}
	s.crop = round({ ...c, x: clamp(x, 0, W - c.w), y: clamp(y, 0, H - c.h) });
};

const applyResize = (d, p, e, out) => {
	const c = d.start;
	const q = snapPoint(p, e.ctrlKey, out);
	let l = c.x;
	let t = c.y;
	let r = c.x + c.w;
	let b = c.y + c.h;
	const hd = d.handle;
	if (hd.includes("w")) l = Math.min(q.x, r - 1);
	if (hd.includes("e")) r = Math.max(q.x, l + 1);
	if (hd.includes("n")) t = Math.min(q.y, b - 1);
	if (hd.includes("s")) b = Math.max(q.y, t + 1);
	const ratio = s.ratio ?? (e.shiftKey ? c.w / c.h : null);
	if (ratio) {
		if (hd.length === 2) {
			const w = r - l;
			const h = w / ratio;
			if (hd.includes("n")) t = b - h;
			else b = t + h;
		} else if (hd === "w" || hd === "e") {
			const h = (r - l) / ratio;
			const mid = (c.y + c.y + c.h) / 2;
			t = mid - h / 2;
			b = mid + h / 2;
		} else {
			const w = (b - t) * ratio;
			const mid = (c.x + c.x + c.w) / 2;
			l = mid - w / 2;
			r = mid + w / 2;
		}
	}
	const x = clamp(l, 0, W);
	const y = clamp(t, 0, H);
	s.crop = round({ x, y, w: Math.min(r, W) - x, h: Math.min(b, H) - y });
};

let reshapeId = 0;
const reshape = (to, from = s.crop) => {
	reshapeId += 1;
	const id = reshapeId;
	if (!from || matchMedia("(prefers-reduced-motion: reduce)").matches) {
		s.crop = round(to);
		paint();
		return;
	}
	const begin = performance.now();
	const step = (now) => {
		if (id !== reshapeId) return;
		const p = Math.min(1, (now - begin) / 240);
		const e = 1 - (1 - p) ** 3;
		s.crop = round({
			x: from.x + (to.x - from.x) * e,
			y: from.y + (to.y - from.y) * e,
			w: from.w + (to.w - from.w) * e,
			h: from.h + (to.h - from.h) * e,
		});
		paint();
		if (p < 1) requestAnimationFrame(step);
	};
	requestAnimationFrame(step);
};

const withRatio = (c, r) => {
	const cx = c.x + c.w / 2;
	const cy = c.y + c.h / 2;
	let w = c.w;
	let h = w / r;
	if (h > H) {
		h = H;
		w = h * r;
	}
	if (w > W) {
		w = W;
		h = w / r;
	}
	return {
		x: clamp(cx - w / 2, 0, W - w),
		y: clamp(cy - h / 2, 0, H - h),
		w,
		h,
	};
};
const fillRect = () => {
	if (!s.ratio) return { x: 0, y: 0, w: W, h: H };
	const w = Math.min(W, H * s.ratio);
	const h = w / s.ratio;
	return { x: (W - w) / 2, y: (H - h) / 2, w, h };
};

desk.addEventListener("pointerdown", (e) => {
	if (e.target.closest(".ui")) return;
	if (s.phase !== "pick") return;
	const had = !!s.menu;
	s.menu = null;
	desk.focus();
	const p = point(e);
	if (had) {
		paint();
		return;
	}
	if (s.target === "window") {
		const w = windowAt(p);
		s.pinned = w?.id ?? null;
		s.hover = w?.id ?? null;
		paint();
		return;
	}
	if (s.target !== "area" || s.locked) return;
	reshapeId += 1;
	const zone = zoneAt(p);
	s.drag = {
		...zone,
		at: p,
		start: s.crop && { ...s.crop },
		previous: s.crop,
		moved: false,
	};
	desk.setPointerCapture(e.pointerId);
	paint();
});

desk.addEventListener("pointermove", (e) => {
	const p = point(e);
	if (!s.drag) {
		if (e.target.closest(".ui") || s.phase !== "pick") return;
		const w = windowAt(p);
		const next = w?.id ?? null;
		const outside = s.crop ? zoneAt(p).kind === "draw" : true;
		if (next !== s.hover || outside !== s.hoverOutside) {
			s.hover = next;
			s.hoverOutside = outside;
			paint();
		}
		if (s.target === "area")
			desk.style.cursor = s.locked
				? ""
				: ({ move: "move", draw: "crosshair" }[zoneAt(p).kind] ?? "");
		return;
	}
	const d = s.drag;
	if (!d.moved && Math.hypot(p.x - d.at.x, p.y - d.at.y) < 3) return;
	d.moved = true;
	const out = [];
	if (d.kind === "draw") applyDraw(d, p, e, out);
	if (d.kind === "move") applyMove(d, p, e, out);
	if (d.kind === "resize") applyResize(d, p, e, out);
	s.guides = out;
	paint();
});

desk.addEventListener("pointerup", (e) => {
	const d = s.drag;
	if (!d) return;
	s.drag = null;
	s.guides = [];
	if (!d.moved && d.kind === "draw") {
		const w = windowAt(point(e));
		if (w) {
			const p = point(e);
			const to = s.ratio ? withRatio(w, s.ratio) : w;
			reshape(
				{ x: to.x, y: to.y, w: to.w, h: to.h },
				{ x: p.x, y: p.y, w: 0, h: 0 },
			);
			return;
		}
	}
	if (d.moved && d.kind === "draw" && s.crop && s.crop.w < 4 && s.crop.h < 4)
		s.crop = d.previous;
	paint();
});

const timers = { tick: null };
const clearTimer = () => {
	clearInterval(timers.tick);
	timers.tick = null;
};
const toRecording = () => {
	clearTimer();
	if (s.mode === "screenshot") {
		s.phase = "pick";
		paint();
		return;
	}
	s.phase = "recording";
	s.sec = 0;
	timers.tick = setInterval(() => {
		s.sec += 1;
		paint();
	}, 1000);
	paint();
};
const toStarting = () => {
	clearTimer();
	s.phase = "starting";
	paint();
	timers.tick = setTimeout(toRecording, 1000);
};
const start = () => {
	if (!ready(s)) return;
	s.menu = null;
	if (s.mode === "screenshot" || s.countdown === 0) {
		toStarting();
		return;
	}
	s.phase = "countdown";
	s.count = s.countdown;
	paint();
	timers.tick = setInterval(() => {
		s.count -= 1;
		if (s.count <= 0) toStarting();
		else paint();
	}, 1000);
};
const cancel = () => {
	clearTimer();
	clearTimeout(timers.tick);
	s.phase = "pick";
	paint();
};

const act = (a, v, el) => {
	if (a === "start") start();
	if (a === "cancel") cancel();
	if (a === "stop") cancel();
	if (a === "close") {
		s.phase = "closed";
		s.menu = null;
	}
	if (a === "reopen") s.phase = "pick";
	if (a === "clear") {
		s.crop = null;
		s.locked = false;
	}
	if (a === "fill") reshape(fillRect());
	if (a === "lock" && valid(s, s.crop)) s.locked = !s.locked;
	if (a === "tgt") {
		s.target = v;
		s.pinned = null;
		s.hover = null;
		s.menu = null;
	}
	if (a === "set-mode") {
		s.mode = v;
		s.menu = null;
	}
	if (a === "cam") {
		s.cam = Number(v);
		s.menu = null;
	}
	if (a === "mic") {
		s.mic = Number(v);
		s.menu = null;
	}
	if (a === "countdown") s.countdown = Number(v);
	if (a === "focus") s.focusWin = !s.focusWin;
	if (a === "center") s.center = !s.center;
	if (a === "ratio") {
		s.ratio = ratios[Number(v)][2];
		s.menu = null;
		if (s.ratio && s.crop) reshape(withRatio(s.crop, s.ratio));
	}
	if (a === "preset") {
		const [w, h] = v.split("x").map((n) => Number(n) / PX);
		const c = s.crop ?? { x: 0, y: 0, w: W, h: H };
		s.ratio = null;
		s.menu = null;
		reshape({
			x: clamp(c.x + c.w / 2 - w / 2, 0, W - w),
			y: clamp(c.y + c.h / 2 - h / 2, 0, H - h),
			w,
			h,
		});
	}
	if (a === "menu") {
		if (s.menu?.id === v) s.menu = null;
		else {
			const r = el.getBoundingClientRect();
			const d = desk.getBoundingClientRect();
			const k = d.width / W;
			s.menu = {
				id: v,
				x: (r.left - d.left) / k,
				top: (r.top - d.top) / k,
				bottom: (r.bottom - d.top) / k,
			};
		}
	}
	paint();
};

ov.addEventListener("click", (e) => {
	const btn = e.target.closest("[data-a]");
	if (btn && !btn.disabled) {
		act(btn.dataset.a, btn.dataset.v, btn);
		if (s.menu === null && s.phase === "pick") desk.focus();
	}
});

desk.addEventListener("keydown", (e) => {
	if (s.phase === "countdown") {
		if (e.key === "Escape") {
			e.preventDefault();
			cancel();
		}
		if (e.key === "Enter") {
			e.preventDefault();
			toStarting();
		}
		return;
	}
	if (s.phase !== "pick") return;
	const k = e.key;
	if (k === "Escape") {
		e.preventDefault();
		if (s.drag) {
			s.crop = s.drag.previous;
			s.drag = null;
			s.guides = [];
		} else if (s.menu) s.menu = null;
		else s.phase = "closed";
		paint();
		return;
	}
	if (k === "Enter" && !e.target.closest("button")) {
		e.preventDefault();
		start();
		return;
	}
	if (s.target !== "area") return;
	if (e.ctrlKey && k.toLowerCase() === "a") {
		e.preventDefault();
		if (!s.locked) reshape(fillRect());
		return;
	}
	if (!e.ctrlKey && k.toLowerCase() === "f" && !s.locked) {
		reshape(fillRect());
		return;
	}
	if (!e.ctrlKey && k.toLowerCase() === "r" && !s.locked) {
		s.crop = null;
		paint();
		return;
	}
	if (!e.ctrlKey && k.toLowerCase() === "l" && valid(s, s.crop)) {
		s.locked = !s.locked;
		paint();
		return;
	}
	const dir = {
		ArrowLeft: [-1, 0],
		ArrowRight: [1, 0],
		ArrowUp: [0, -1],
		ArrowDown: [0, 1],
	}[k];
	if (!dir || !s.crop || s.locked) return;
	e.preventDefault();
	e.stopPropagation();
	const step = e.shiftKey ? 10 : 1;
	const c = s.crop;
	s.crop = e.ctrlKey
		? {
				...c,
				w: clamp(c.w + dir[0] * step, 1, W - c.x),
				h: clamp(c.h + dir[1] * step, 1, H - c.y),
			}
		: {
				...c,
				x: clamp(c.x + dir[0] * step, 0, W - c.w),
				y: clamp(c.y + dir[1] * step, 0, H - c.h),
			};
	paint();
});

const seg = (name, value, options) =>
	`<div class="seg" data-seg="${name}">${options.map(([v, en, zh]) => `<button class="${value === v ? "on" : ""}" data-v="${v}">${zh === undefined ? en : L(en, zh)}</button>`).join("")}</div>`;

const renderTools = () => {
	document.getElementById("live-tools").innerHTML = `${seg("target", s.target, [
		["display", "Display", "显示器"],
		["window", "Window", "窗口"],
		["area", "Area", "区域"],
	])}${seg("mode", s.mode, [
		["studio", "Studio", "工作室"],
		["instant", "Instant", "即时"],
		["screenshot", "Screenshot", "截图"],
	])}${seg("countdown", String(s.countdown), [
		["0", "No countdown", "无倒计时"],
		["3", "3 s", "3 秒"],
		["10", "10 s", "10 秒"],
	])}`;
};

const renderMinis = () => {
	const base = {
		...s,
		menu: null,
		drag: null,
		guides: [],
		hover: null,
		pinned: null,
		target: "area",
		phase: "pick",
		locked: false,
		ratio: null,
		count: 3,
		countdown: 3,
		crop: { x: 300, y: 130, w: 640, h: 360 },
	};
	const smallCrop = { x: 520, y: 240, w: 150, h: 150 };
	const cards = [
		["Empty, before drawing", "空状态，还没画", { crop: null }],
		[
			"Hovering a window",
			"悬停在窗口上",
			{ crop: null, hover: "notes", hoverOutside: true },
		],
		[
			"Drawing, snapped to a window",
			"正在画，吸附到窗口",
			{
				crop: { x: 240, y: 136, w: 476, h: 300 },
				drag: { kind: "draw" },
				guides: [
					{ axis: "v", pos: 716 },
					{ axis: "h", pos: 136 },
				],
			},
		],
		["Selected", "已选好", { ratio: 16 / 9 }],
		["Locked", "已锁定", { locked: true }],
		["Too small", "太小", { crop: { x: 520, y: 240, w: 110, h: 90 } }],
		[
			"Area at the display edge",
			"选区贴着屏幕边",
			{ crop: { x: 0, y: 0, w: W, h: 680 } },
		],
		["Window picked", "已选定窗口", { target: "window", pinned: "browser" }],
		["Whole display", "整个显示器", { target: "display" }],
		["Screenshot mode", "截图模式", { mode: "screenshot" }],
		["Countdown over an area", "选区上的倒计时", { phase: "countdown" }],
		[
			"Countdown, smallest area",
			"最小选区上的倒计时",
			{ phase: "countdown", crop: smallCrop },
		],
		[
			"Countdown over a window",
			"窗口上的倒计时",
			{ phase: "countdown", target: "window", pinned: "notes", count: 2 },
		],
		[
			"Countdown, whole display",
			"整个显示器的倒计时",
			{ phase: "countdown", target: "display", count: 1 },
		],
		["Ten seconds", "10 秒", { phase: "countdown", countdown: 10, count: 10 }],
		["Starting, cue playing", "正在开始，提示音", { phase: "starting" }],
		["Recording an area", "正在录制选区", { phase: "recording", sec: 12 }],
	];
	const mini = (st) =>
		`<div class="mini"><div class="mini-in">${wallHTML}${overlay(st)}</div></div>`;
	const items = cards.map(
		([en, zh, patch]) =>
			`<div class="mini-card"><p class="label">${L(en, zh)}</p>${mini({ ...base, ...patch })}</div>`,
	);
	items.splice(
		9,
		0,
		`<div class="mini-card"><p class="label">${L("Two displays: the one under the cursor is clear", "两台显示器：光标所在的那台是亮的")}</p><div class="mini pair">${mini({ ...base, target: "display" })}${mini({ ...base, target: "display", other: true })}</div></div>`,
	);
	const grid = document.getElementById("mini-grid");
	grid.innerHTML = items.join("");
	settle(grid);
	fit();
};

const table = (id, head, rows) => {
	document.getElementById(id).innerHTML =
		`<table class="inv"><thead><tr>${head.map(([en, zh]) => `<th>${L(en, zh)}</th>`).join("")}</tr></thead><tbody>${rows.map((row) => `<tr>${row.map((cell, i) => `<td class="${i ? "dim" : ""}">${Array.isArray(cell) ? L(...cell) : cell}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
};

const copy = () => {
	const text = {
		"t-live": ["Try it", "上手试试"],
		"l-live": [
			"This stage is a 2560 × 1440 display at 200%, shrunk to fit the page. Draw, move and resize; click a window to fit the area to it; press Start to run the countdown.",
			"这个舞台是一台 2560 × 1440、200% 缩放的显示器，按页面宽度缩小显示。可以画、移动、调整大小，点窗口让选区贴合，点开始会走完倒计时。",
		],
		"t-states": ["Every state", "所有状态"],
		"l-states": [
			"The bar follows the target: below it, above it when there's no room below, and inside it when the target fills the screen. It hides while you drag. For a whole display it sits at the bottom centre.",
			"控制条跟着目标走：放在下方，下方放不下就到上方，目标占满屏幕时放到里面。拖动时隐藏。选整个显示器时放在底部中央。",
		],
		"t-shared": ["Rules", "规则"],
		"l-shared": ["Why it looks and behaves this way.", "为什么这样设计。"],
		"t-keys": ["Keyboard", "键盘"],
		"l-keys": [
			"Every action works without a mouse. Upstream's keys are kept; the rest fills the gaps the keyboard floor needs.",
			"所有操作都能不用鼠标完成。上游的按键保留，其余是为了满足键盘底线补上的。",
		],
		"t-inv": ["Where everything went", "每个功能的去向"],
		"l-inv": [
			"Compared with our picker and upstream f7d351611. Upstream has moved well past our 0.6.1 merge here: its area picker gained ratios, a lock, window snapping, click to fit, and the window picker gained Focus and a size menu.",
			"对照了我们的选择器和上游 f7d351611。这一块上游已经远远超过我们合并的 0.6.1：区域选择加了比例、锁定、吸附窗口、点击贴合，窗口选择加了专注和尺寸菜单。",
		],
		"t-add": ["Added from other products", "从其他产品加入"],
		"l-add": [
			"Open-source and popular tools already have these, so they go in unless the cost says otherwise.",
			"这些开源和热门工具已经有了，除非代价不合算，否则都加入。",
		],
		"t-ref": ["What this is built on", "这份设计的依据"],
		"l-ref": [
			"Our own decisions come first, then primary sources. Where a source couldn't be confirmed, it isn't cited.",
			"先看我们自己的决定，再看一手资料。查不到出处的说法没有列入。",
		],
	};
	for (const [id, pair] of Object.entries(text))
		document.getElementById(id).innerHTML = L(...pair);
	table(
		"rules",
		[
			["Rule", "规则"],
			["Why", "原因"],
		],
		[
			[
				[
					"What's lit is what's recorded. The target stays clear, everything else takes the overlay scrim. No blue highlight",
					"亮的就是要录的。目标保持原样，其余盖上 overlay 遮罩。不用蓝色高亮",
				],
				[
					"One rule for display, window and area. The blue wash coloured your judgement of the picture and isn't on the Klein list",
					"显示器、窗口、区域同一条规则。蓝色覆盖会影响对画面的判断，也不在蓝色白名单里",
				],
			],
			[
				[
					"Selection edge is 2 px white with a 1 px dark line outside; handles and snap guides are built the same way",
					"选区边框是 2 px 白线加外侧 1 px 深色线；手柄和吸附线同样构造",
				],
				[
					"It has to show on any screen content, and shadows aren't allowed",
					"它要在任何画面上都看得见，而且不能用阴影",
				],
			],
			[
				[
					"Sizes show the pixels that get recorded, not logical points",
					"尺寸显示实际录出来的像素，不是逻辑点",
				],
				[
					"At 200%, upstream's readout says 640 × 360 for a 1280 × 720 video",
					"200% 缩放下，上游显示 640 × 360，而录出来是 1280 × 720",
				],
			],
			[
				[
					"Moving and resizing never animate the window. A click to fit and a ratio change reshape the selection for 240 ms",
					"移动和调整大小都不动画窗口。点击贴合和改比例时，选区用 240 ms 过渡",
				],
				[
					"Upstream's timing. It's content in a window that's already open, which GPUI can do",
					"这是上游的时长。动的是已打开窗口里的内容，GPUI 做得到",
				],
			],
			[
				[
					"The looping hint is replaced by a still card",
					"循环动画提示换成静止卡片",
				],
				[
					"Each frame repaints a full-screen window, and continuous motion isn't allowed near a recording",
					"每帧都要重绘全屏窗口，而且录制附近不允许持续动画",
				],
			],
			[
				[
					"The countdown changes once a second, with a 120 ms fade on the number only. Font size never animates",
					"倒计时每秒变一次，只有数字淡入 120 ms。字号不做动画",
				],
				[
					"The capture pipeline warms up during the last second, so nothing should repaint then",
					"最后一秒采集管线在预热，那时不应该有重绘",
				],
			],
			[
				[
					"The overlay stays through the countdown, then goes. During recording an area gets four still corner marks and no scrim",
					"倒计时期间覆盖层还在，之后消失。录制选区时只留四个静止的角标，没有遮罩",
				],
				[
					"Today's scrim stays dark over your whole screen while you work. The marks show the edge without getting in the way",
					"现在的遮罩在你操作时一直压暗整个屏幕。角标能标出边界，又不碍事",
				],
			],
			[
				[
					"The countdown shows only on the recorded display and in the recording bar",
					"倒计时只出现在被录的显示器和录制控制条上",
				],
				[
					"The bar's Starts in 3 from Recording controls stays. If you started from a hotkey the overlay has no focus, so Cancel is the bar's X or the stop shortcut",
					"录制控制条的“3 秒后开始”保留。从快捷键开始时覆盖层没有焦点，取消要用控制条的 X 或停止快捷键",
				],
			],
			[
				[
					"The controls sit against the target, so the mouse travels least after drawing",
					"控件贴着目标，画完后鼠标走得最近",
				],
				[
					"Upstream, Flameshot and most Chinese screenshot tools work this way, and the bar reads as one object with the selection",
					"上游、Flameshot 和大多数国产截图工具都这样，控制条和选区是一个整体",
				],
			],
			[
				[
					"The countdown reuses Start's ring, navy gap and Klein plate at 128 px with the number inside. On the last beat the number becomes the red mark while the cue plays",
					"倒计时沿用开始按钮的白环、海军蓝间隙和克莱因底，放大到 128 px，数字在里面。最后一拍数字换成红色标记，同时播放提示音",
				],
				[
					"It ties the countdown to the button you just pressed and marks the moment recording starts. This is the one new entry on the Klein blue list",
					"把倒计时和刚按下的按钮连在一起，并标出录制开始的那一刻。这是蓝色白名单新增的唯一一项",
				],
			],
			[
				[
					"Klein blue appears only on Start and the countdown mark",
					"克莱因蓝只用在开始按钮和倒计时标记上",
				],
				[
					"Selected ratios and targets use the neutral selection fill",
					"选中的比例和目标用中性的 selection 底色",
				],
			],
		],
	);
	table(
		"keys",
		[
			["Action", "操作"],
			["Key", "按键"],
			["Source", "来源"],
		],
		[
			[
				["Start, or take the screenshot", "开始录制或截图"],
				keys("Enter"),
				["Upstream", "上游"],
			],
			[
				["Close the picker, or put a drag back", "关闭选择器，或撤回拖动"],
				keys("Esc"),
				["Upstream", "上游"],
			],
			[
				["Move the area", "移动选区"],
				`${keys("←")} ${keys("↑")} ${keys("↓")} ${keys("→")}`,
				["Upstream; Shift for 10", "上游；Shift 每次 10"],
			],
			[
				["Resize the area", "调整选区大小"],
				`${keys("Ctrl")} ${keys("←")} ${keys("↑")} ${keys("↓")} ${keys("→")}`,
				["New; Shift for 10", "新增；Shift 每次 10"],
			],
			[
				["Whole display", "选中整个屏幕"],
				`${keys("Ctrl", "A")} ${L("or", "或")} ${keys("F")}`,
				["New", "新增"],
			],
			[["Clear the area", "清除选区"], keys("R"), ["New", "新增"]],
			[["Lock or unlock", "锁定或解锁"], keys("L"), ["New", "新增"]],
			[
				["Snap off while dragging", "拖动时关闭吸附"],
				keys("Ctrl"),
				["Upstream", "上游"],
			],
			[
				["Keep the ratio while resizing", "调整大小时保持比例"],
				keys("Shift"),
				["New", "新增"],
			],
			[
				["Reach the bar's controls", "切到控制条上的按钮"],
				keys("Tab"),
				["New", "新增"],
			],
			[["Cancel the countdown", "取消倒计时"], keys("Esc"), ["New", "新增"]],
			[["Skip the countdown", "跳过倒计时"], keys("Enter"), ["New", "新增"]],
		],
	);
	table(
		"inv",
		[
			["Capability", "功能"],
			["Today", "现状"],
			["In this design", "这份设计"],
		],
		[
			[
				["Display picker", "显示器选择"],
				[
					"Ours and upstream: blue wash, monitor art, name, size and rate",
					"我们和上游：蓝色覆盖、显示器插画、名称、尺寸和刷新率",
				],
				[
					"The display stays clear with a white edge; name, size and rate in a tag. Other displays dim",
					"显示器保持原样加白边；名称、尺寸和刷新率放标签里。其他显示器压暗",
				],
			],
			[
				["Window picker", "窗口选择"],
				[
					"Ours and upstream: blue wash, 96 px icon, name, size",
					"我们和上游：蓝色覆盖、96 px 图标、名称、尺寸",
				],
				[
					"The window is cut out of the scrim, and a tag names it. Click to pin it",
					"窗口从遮罩里挖出来，标签写名称。点击选定",
				],
			],
			[
				[
					"Window size menu: ratios and 720p–4K",
					"窗口尺寸菜单：比例和 720p–4K",
				],
				["Upstream only", "仅上游"],
				["Port. It resizes the real window", "移植。调整的是真实窗口"],
			],
			[
				["Focus and Center on screen", "专注和移到屏幕中央"],
				["Upstream only", "仅上游"],
				["Port, in the window menu", "移植，放进窗口菜单"],
			],
			[
				["Draw, move and eight-way resize", "绘制、移动、八向调整"],
				["Ours and upstream", "我们和上游都有"],
				[
					"Kept, with square corner handles and bars on the edges",
					"保留，四角方块手柄，四边条形手柄",
				],
			],
			[
				["Snap to display edges and centres", "吸附到屏幕边缘和中线"],
				["Ours and upstream", "我们和上游都有"],
				["Kept; the pink guide becomes white", "保留；粉色吸附线改成白色"],
			],
			[
				["Snap to windows and the work area", "吸附到窗口和工作区"],
				["Upstream only", "仅上游"],
				["Port, with Ctrl to turn snapping off", "移植，Ctrl 关闭吸附"],
			],
			[
				["Click a window to fit the area", "点击窗口贴合选区"],
				["Upstream only", "仅上游"],
				[
					"Port, with a dashed outline on hover so you can see it coming",
					"移植，悬停时显示虚线框，先看到再点",
				],
			],
			[
				["Ratios, reset, fill", "比例、重置、填满"],
				[
					"Upstream only; the 0.6.1 code says they're deferred",
					"仅上游；0.6.1 的代码里写着延后",
				],
				[
					"Port. One ratio menu holds all nine ratios and the exact sizes",
					"移植。一个比例菜单放下全部九种比例和精确尺寸",
				],
			],
			[
				["Lock an area for next time", "锁定区域供下次使用"],
				["Upstream only", "仅上游"],
				[
					"Port, including the Locked badge on the area",
					"移植，包括选区上的“已锁定”标记",
				],
			],
			[
				["Rule of thirds while dragging", "拖动时显示三分线"],
				["Upstream only", "仅上游"],
				["Port", "移植"],
			],
			[
				["Area size readout", "选区尺寸显示"],
				["Ours: at the top, in points", "我们：在顶部，单位是点"],
				[
					"In pixels, in a tag above the area; while dragging it sits at the bottom right",
					"单位是像素，放在选区上方的标签里；拖动时放在右下角",
				],
			],
			[
				["Minimum size message", "最小尺寸提示"],
				["Ours and upstream", "我们和上游都有"],
				[
					"Kept, in the tag, rewritten to the copy guide",
					"保留，放在标签里，按文案规范改写",
				],
			],
			[
				["Camera bubble in the area", "选区里的摄像头气泡"],
				["Ours and upstream", "我们和上游都有"],
				[
					"Kept; its look belongs to Camera and teleprompter",
					"保留；样子归摄像头和提词器那张票",
				],
			],
			[
				["Camera only target", "仅摄像头目标"],
				["Ours and upstream", "我们和上游都有"],
				[
					"Kept; it belongs to Camera and teleprompter too",
					"保留；同样归摄像头和提词器那张票",
				],
			],
			[
				[
					"Mode, camera and microphone in the picker",
					"选择器里的模式、摄像头和麦克风",
				],
				[
					"Ours and upstream: a 416 px glass card",
					"我们和上游：416 px 的玻璃卡片",
				],
				[
					"Icons on the bar, opening the same lists, with a live level",
					"条上的图标，打开同样的列表，带实时音量",
				],
			],
			[
				["Countdown setting in the picker", "选择器里的倒计时设置"],
				["Ours and upstream: gear menu", "我们和上游：齿轮菜单"],
				[
					"Moves into Recording options, as in the compact window",
					"移到录制选项里，和紧凑窗口一致",
				],
			],
			[
				["Full-screen countdown", "全屏倒计时"],
				[
					"Neither has one: the number sits in the bar and the main window",
					"都没有：数字只在控制条和主窗口里",
				],
				["New, on the recorded display", "新增，出现在被录的显示器上"],
			],
			[
				["Selection hint", "选区提示"],
				["Ours and upstream: looping animation", "我们和上游：循环动画"],
				["Still card", "静止卡片"],
			],
			[
				["Liquid glass and its backdrop capture", "液态玻璃和背景抓取"],
				["Upstream", "上游"],
				["Dropped: no glass", "去掉：不用玻璃"],
			],
			[
				["Sign-in check before Instant", "即时录制前的登录检查"],
				["Upstream", "上游"],
				["Dropped: no accounts", "去掉：没有账号"],
			],
		],
	);
	table(
		"add",
		[
			["Feature", "功能"],
			["Who has it", "谁有"],
			["Cost", "代价"],
			["Recommendation", "建议"],
		],
		[
			[
				["Type an exact size", "输入精确尺寸"],
				"Screen Studio",
				[
					"Medium: a field doesn't fit a bar that follows the selection",
					"中：跟随选区的控制条放不下输入框",
				],
				[
					"Leave out: the ratio menu has exact sizes",
					"不加：比例菜单里有精确尺寸",
				],
			],
			[
				["Exact size presets for an area", "选区的精确尺寸预设"],
				["Upstream has them for windows only", "上游只有窗口有"],
				["Small: reuses the same sizes", "小：复用同一组尺寸"],
				["Add, in the ratio menu", "加入，放在比例菜单里"],
			],
			[
				["Resize from the keyboard", "用键盘调整大小"],
				"Flameshot",
				["Small", "小"],
				["Add", "加入"],
			],
			[
				["Keep the ratio with a modifier", "按住修饰键保持比例"],
				"Flameshot",
				["Small", "小"],
				[
					"Add, on Shift, because Ctrl turns off snapping",
					"加入，用 Shift，因为 Ctrl 用来关吸附",
				],
			],
			[
				["Magnifier while dragging", "拖动时的放大镜"],
				["ShareX, on by default", "ShareX，默认开启"],
				[
					"Medium: needs a frozen frame of the display",
					"中：需要一张冻结的屏幕画面",
				],
				["Add for Screenshot mode only", "只在截图模式加入"],
			],
			[
				["Audible countdown", "有声倒计时"],
				"Screencastify",
				["Small: we have a cue at the end", "小：结尾已有提示音"],
				["Leave out: the cue is enough", "不加：结尾提示音够了"],
			],
			[
				["Skip the countdown with Enter", "按 Enter 跳过倒计时"],
				["No source found; our idea", "没找到出处，是我们的想法"],
				["Small", "小"],
				["Add", "加入"],
			],
			[
				["Scrolling capture", "滚动截图"],
				"ShareX",
				[
					"Large, and a new capture feature rather than picker design",
					"大，而且是新的采集功能，不是选择器设计",
				],
				["Leave out", "不加"],
			],
		],
	);
	table(
		"refs",
		[
			["Source", "来源"],
			["What it decided here", "决定了什么"],
		],
		[
			[
				'<a href="https://github.com/AniBullet/FrameEver/issues/57">Recording controls</a>',
				[
					"Bar size, radius and construction; the countdown state in the bar; the full-screen countdown left to this ticket",
					"条的尺寸、圆角和构造；条内的倒计时状态；全屏倒计时留给这张票",
				],
			],
			[
				'<a href="https://github.com/AniBullet/FrameEver/issues/25">What can GPUI animate</a>',
				[
					"No window animation; each frame repaints the window, so no loops on a full-screen overlay",
					"窗口不动画；每帧都重绘窗口，所以全屏覆盖层不做循环动画",
				],
			],
			[
				'<a href="https://github.com/AniBullet/FrameEver/blob/main/docs/design/index.html">Design reference</a>, <a href="https://github.com/AniBullet/FrameEver/blob/main/docs/brand/app-icon.md">App icon</a>',
				[
					"Klein blue list, never-animated list, the ring, gap and red mark",
					"蓝色白名单、从不动画清单、白环、间隙和红色标记",
				],
			],
			[
				'<a href="https://github.com/AniBullet/FrameEver/blob/main/docs/copy-style.md">Copy style</a>',
				[
					"Tooltips are a name and a shortcut, so upstream's hint sentences go",
					"提示只写名称和快捷键，所以上游的说明句子去掉",
				],
			],
			[
				["Upstream Cap f7d351611", "上游 Cap f7d351611"],
				[
					"Ratios, lock, snap targets, click to fit, thirds, Focus, window sizes, Enter and arrow keys, 240 ms reshape, 7 px snap, 18 px grab",
					"比例、锁定、吸附目标、点击贴合、三分线、专注、窗口尺寸、Enter 和方向键、240 ms 过渡、7 px 吸附、18 px 抓取范围",
				],
			],
			[
				'<a href="https://flameshot.org/docs/guide/key-bindings/">Flameshot</a>',
				[
					"Arrow keys, a modifier to keep the ratio, Esc to quit",
					"方向键、保持比例的修饰键、Esc 退出",
				],
			],
			[
				'<a href="https://github.com/ShareX/ShareX/blob/develop/ShareX.ScreenCaptureLib/RegionCaptureOptions.cs">ShareX</a>',
				[
					"Magnifier on by default, 15 pixels across",
					"放大镜默认开启，15 像素宽",
				],
			],
			[
				'<a href="https://github.com/wulkano/Kap/blob/main/renderer/components/cropper/handles.js">Kap</a>',
				["Eight handles around the crop", "选区周围八个手柄"],
			],
			[
				'<a href="https://www.pcworld.com/article/2532666/windows-11-has-an-easy-tool-for-recording-screencasts.html">Windows Snipping Tool</a>, <a href="https://learn.screencastify.com/hc/en-us/articles/360050332833">Screencastify</a>',
				[
					"A visible 3-2-1 before recording is the norm; the setting has Off, 3, 5 and 10",
					"录制前显示 3-2-1 是常见做法；设置项是关、3、5、10",
				],
			],
		],
	);
};

const renderAll = () => {
	renderTools();
	copy();
	renderMinis();
	paint();
};

document.addEventListener("click", (e) => {
	const b = e.target.closest("[data-seg] button");
	if (b && e.target.closest("#live-tools")) {
		const name = b.closest("[data-seg]").dataset.seg;
		cancel();
		if (name === "target") {
			s.target = b.dataset.v;
			s.pinned = null;
			s.hover = null;
		}
		if (name === "mode") s.mode = b.dataset.v;
		if (name === "countdown") s.countdown = Number(b.dataset.v);
		s.phase = "pick";
		renderAll();
		return;
	}
});

setInterval(() => {
	for (const bar of document.querySelectorAll("#ov .rb-meter b"))
		bar.style.width = `${20 + Math.random() * 60}%`;
}, 140);

shell({
	here: "area-picker",
	depth: 1,
	sections: [
		["live", "Try it", "试用"],
		["states", "States", "状态"],
		["shared", "Rules", "规则"],
		["keyboard", "Keyboard", "键盘"],
		["inventory", "Everything", "去向"],
		["additions", "Additions", "新增"],
		["references", "References", "参考"],
	],
});
renderAll();
onChange(renderAll);
