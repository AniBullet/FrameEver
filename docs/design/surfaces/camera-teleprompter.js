const { L, icon, pick, shell, onChange, state: page } = FE;

const W = 1280;
const H = 720;
const WORK = 680;
const MARGIN = 16;
const SNAP = 18;
const MIN = 150;
const MAX = 600;
const WIDE = 16 / 9;

const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
const keys = (...list) => list.map((key) => `<span class="kbd">${key}</span>`).join(" ");
const wallHTML = `<div class="wall"><div class="wall-win" style="left:96px;top:56px;width:620px;height:440px"><header></header><p style="left:24px;top:56px;width:40%;height:14px;background:#777"></p><p style="left:24px;top:88px;width:80%;height:9px"></p><p style="left:24px;top:108px;width:72%;height:9px"></p><p style="left:24px;top:128px;width:76%;height:9px"></p><p style="left:24px;top:164px;width:44%;height:42%"></p><p style="left:50%;top:164px;width:44%;height:42%;background:#b0b0b0"></p></div><div class="wall-win b" style="left:420px;top:136px;width:760px;height:500px"><header></header><p style="left:24px;top:56px;width:50%;height:14px;background:#777"></p><p style="left:24px;top:88px;width:88%;height:9px"></p><p style="left:24px;top:108px;width:80%;height:9px"></p><p style="left:24px;top:128px;width:84%;height:9px"></p></div><div class="wall-bar"></div></div>`;

const c = {
	v: "A", x: 934, y: 356, size: 230, shape: "round", mirror: false, blur: 0, phase: "idle", issue: "", hover: false, focus: false, card: false, drag: null, guides: [], sec: 0,
};
const camVariants = { A: ["Bar above", "气泡上方的条"], B: ["Bar inside", "画面里的条"], C: ["Menu button", "菜单按钮"] };

const rectOf = (st) => ({ x: st.x, y: st.y, w: st.shape === "wide" ? Math.round(st.size * WIDE) : st.size, h: st.size });
const radiusOf = (st, r) => (st.shape === "round" ? r.h / 2 : 20);

const picture = (st) => {
	const blur = [0, 4, 10][st.blur];
	return `<svg class="pic" viewBox="0 0 400 225" preserveAspectRatio="xMidYMid slice" style="${st.mirror ? "transform:scaleX(-1)" : ""}" aria-hidden="true"><g style="${blur ? `filter:blur(${blur}px)` : ""}"><rect x="-30" y="-30" width="460" height="285" fill="#565656"/><rect x="14" y="42" width="96" height="150" fill="#6e6e6e"/><rect x="24" y="56" width="20" height="62" fill="#3f3f3f"/><rect x="50" y="70" width="16" height="48" fill="#8e8e8e"/><rect x="72" y="64" width="26" height="54" fill="#4b4b4b"/><rect x="312" y="30" width="70" height="46" fill="#7a7a7a"/></g><circle cx="200" cy="92" r="42" fill="#b9b9b9"/><path d="M116 225c4-58 40-82 84-82s80 24 84 82z" fill="#2e2e2e"/></svg>`;
};

const issues = {
	loading: ["quiet", "", ""],
	access: ["", ["Camera access needed", "需要摄像头权限"], ["Allow it in Windows Settings.", "在 Windows 设置里允许。"]],
	busy: ["", ["Couldn't start camera", "无法启动摄像头"], ["It may be in use by another app.", "可能正被其他应用占用。"]],
	lost: ["", ["Camera disconnected", "摄像头已断开"], ["Recording continues without it.", "录制继续，只是没有摄像头画面。"]],
};
const issueLayer = (st) => {
	if (!st.issue) return "";
	const [cls, title, msg] = issues[st.issue];
	if (cls) return `<div class="cam-issue quiet">${icon("video", "lg")}</div>`;
	return `<div class="cam-issue"><b>${L(...title)}</b><span>${L(...msg)}</span></div>`;
};

const barButtons = (st, n) => {
	const shapeIcon = { round: "circle", square: "square", wide: "rectangle-horizontal" }[st.shape];
	const blurDots = st.blur ? `<span class="blur-dots">${"<i></i>".repeat(st.blur)}</span>` : "";
	const list = [
		`<button class="rb-btn" data-a="shape" title="${pick("Shape  S", "形状  S")}" aria-label="${pick("Shape", "形状")}">${icon(shapeIcon)}</button>`,
		`<button class="rb-btn ${st.size >= 400 ? "on" : ""}" data-a="size" title="${pick("Size  +", "大小  +")}" aria-label="${pick("Size", "大小")}">${icon("maximize-2")}</button>`,
		`<button class="rb-btn ${st.mirror ? "on" : ""}" data-a="mirror" title="${pick("Mirror  M", "镜像  M")}" aria-label="${pick("Mirror", "镜像")}">${icon("flip-horizontal-2")}</button>`,
		`<button class="rb-btn ${st.blur ? "on" : ""}" data-a="blur" title="${pick("Background blur  B", "背景模糊  B")}" aria-label="${pick("Background blur", "背景模糊")}">${icon("person-standing")}${blurDots}</button>`,
	].slice(0, n - 1);
	return `${list.join("")}<span class="rb-sep"></span><button class="rb-btn" data-a="close" title="${pick("Turn camera off  Del", "关闭摄像头  Del")}" aria-label="${pick("Turn camera off", "关闭摄像头")}">${icon("x")}</button>`;
};
const barWidth = (n) => n * 28 + (n - 1) * 2 + 7 + 10;

const brackets = (r, st) => {
	const inset = 6;
	const spots = st.shape === "round" ? 2 : inset;
	return [["nw", r.x + spots, r.y + spots], ["ne", r.x + r.w - spots - 14, r.y + spots], ["sw", r.x + spots, r.y + r.h - spots - 14], ["se", r.x + r.w - spots - 14, r.y + r.h - spots - 14]]
		.map(([id, x, y]) => `<i class="cam-br ${id}" style="left:${x}px;top:${y}px"></i>`).join("");
};

const chrome = (st, r) => {
	if (!st.hover && !st.drag && !st.card) return "";
	if (st.phase === "instant") return "";
	const handles = brackets(r, st);
	if (st.v === "A") {
		const n = r.w >= barWidth(5) ? 5 : r.w >= barWidth(4) ? 4 : 3;
		const bw = barWidth(n);
		const flip = r.y < 44;
		const top = flip ? r.y + r.h + 8 : r.y - 44;
		return `${handles}<div class="rb cam-bar ui" role="toolbar" style="left:${clamp(r.x + r.w / 2 - bw / 2, 4, W - bw - 4)}px;top:${top}px">${barButtons(st, n)}</div>`;
	}
	if (st.v === "B") {
		const y = 0.76;
		const chord = st.shape === "round" ? 2 * Math.sqrt(0.25 - (y - 0.5) ** 2) * r.h - 8 : r.w - 16;
		const n = chord >= barWidth(5) ? 5 : chord >= barWidth(4) ? 4 : 3;
		const bw = barWidth(n);
		return `${handles}<div class="rb cam-bar inside ui" role="toolbar" style="left:${r.x + r.w / 2 - bw / 2}px;top:${r.y + r.h * y - 18}px">${barButtons(st, n)}</div>`;
	}
	const offset = st.shape === "round" ? r.h * 0.146 : 8;
	const more = `<button class="cam-more ui" data-a="card" title="${pick("Camera options", "摄像头选项")}" aria-label="${pick("Camera options", "摄像头选项")}" style="left:${r.x + r.w - offset - 28}px;top:${r.y + offset}px">${icon("more-horizontal", "sm")}</button>`;
	if (!st.card) return handles + more;
	const left = r.x + r.w + 8 + 232 <= W ? r.x + r.w + 8 : r.x - 240;
	const top = clamp(r.y, 8, WORK - 268);
	const seg = (a, value, options) => `<div class="seg">${options.map(([v, label]) => `<button class="${value === v ? "on" : ""}" data-a="${a}" data-v="${v}">${label}</button>`).join("")}</div>`;
	return `${handles}${more}<div class="cam-card ui" style="left:${left}px;top:${top}px">
		<div class="row"><span class="grow">${L("Shape", "形状")}</span>${seg("set-shape", st.shape, [["round", L("Round", "圆形")], ["square", L("Square", "方形")], ["wide", L("Wide", "宽屏")]])}</div>
		<div class="row"><span class="grow">${L("Size", "大小")}</span>${seg("set-size", st.size, [[230, L("Small", "小")], [400, L("Large", "大")]])}</div>
		<div class="row"><span class="grow">${L("Background blur", "背景模糊")}</span>${seg("set-blur", st.blur, [[0, L("Off", "关")], [1, L("Light", "轻")], [2, L("Heavy", "重")]])}</div>
		<button class="row pop-row" style="padding:0" data-a="mirror"><span class="grow">${L("Mirror", "镜像")}</span><span class="switch ${st.mirror ? "on" : ""}"></span></button>
		<hr><button class="btn" data-a="close">${L("Turn camera off", "关闭摄像头")}</button></div>`;
};

const bubble = (st, r) => `<div class="cam ${st.focus ? "focus" : ""}" style="left:${r.x}px;top:${r.y}px;width:${r.w}px;height:${r.h}px;border-radius:${radiusOf(st, r)}px">${picture(st)}${issueLayer(st)}</div>`;

const fmt = (sec) => `${Math.floor(sec / 60)}:${String(sec % 60).padStart(2, "0")}`;
const recBar = (sec) => `<div class="rec-at ui"><div class="rb" role="toolbar"><span class="rb-grip">${icon("grip-vertical", "sm")}</span><span class="rb-time tnum"><i class="rb-dot"></i>${fmt(sec)}</span><span class="rb-btn" style="cursor:default">${icon("mic")}<span class="rb-meter"><b style="width:46%"></b></span></span><span class="rb-btn">${icon("pause", "solid")}</span><span class="rb-sep"></span><span class="rb-stop"><span class="stop-glyph"></span></span></div></div>`;

const camOverlay = (st) => {
	const r = rectOf(st);
	if (st.off) return `<div class="closed ui"><button class="btn" data-a="on">${L("Turn camera on", "打开摄像头")}</button></div>`;
	const guides = st.guides.map((g) => (g.axis === "v" ? `<div class="cam-guide" style="left:${g.pos}px;top:0;width:1px;height:${WORK}px"></div>` : `<div class="cam-guide" style="left:0;top:${g.pos}px;width:${W}px;height:1px"></div>`)).join("");
	return `${guides}${bubble(st, r)}${chrome(st, r)}${st.phase === "idle" ? "" : recBar(st.sec)}`;
};

const snap = (r) => {
	const out = [];
	const xs = [[MARGIN, 0], [W / 2, r.w / 2], [W - MARGIN, r.w]];
	const ys = [[MARGIN, 0], [WORK / 2, r.h / 2], [WORK - MARGIN, r.h]];
	let { x, y } = r;
	for (const [line, off] of xs) if (Math.abs(x + off - line) <= SNAP) { x = line - off; out.push({ axis: "v", pos: line }); break; }
	for (const [line, off] of ys) if (Math.abs(y + off - line) <= SNAP) { y = line - off; out.push({ axis: "h", pos: line }); break; }
	return { x, y, guides: out };
};

const camDesk = document.getElementById("cam-desk");
const camOv = document.getElementById("cam-ov");
const tpDesk = document.getElementById("tp-desk");
const tpOv = document.getElementById("tp-ov");
for (const host of document.querySelectorAll(".wall-host")) host.innerHTML = wallHTML;

const pointIn = (desk, e) => {
	const r = desk.getBoundingClientRect();
	return { x: ((e.clientX - r.left) / r.width) * W, y: ((e.clientY - r.top) / r.height) * H };
};
const inRect = (p, r, pad = 0) => p.x >= r.x - pad && p.x <= r.x + r.w + pad && p.y >= r.y - pad && p.y <= r.y + r.h + pad;
const hitsWindow = (st, p) => {
	const r = rectOf(st);
	const strip = st.v === "A" ? { x: r.x, y: r.y - 44, w: r.w, h: r.h + 88 } : r;
	return inRect(p, strip);
};
const cornerAt = (st, p) => {
	const r = rectOf(st);
	const hit = 28;
	if (p.x < r.x + hit && p.y < r.y + hit) return "nw";
	if (p.x > r.x + r.w - hit && p.y < r.y + hit) return "ne";
	if (p.x < r.x + hit && p.y > r.y + r.h - hit) return "sw";
	if (p.x > r.x + r.w - hit && p.y > r.y + r.h - hit) return "se";
	return null;
};

const camDescribe = () => {
	const r = rectOf(c);
	const windowH = c.v === "A" ? r.h + 44 : r.h;
	return `camera ${c.v} · ${c.phase} · ${c.shape} · bubble ${r.w} × ${r.h} · window ${r.w} × ${windowH} · ${r.x}, ${r.y} · mirror ${c.mirror ? "on" : "off"} · blur ${["off", "light", "heavy"][c.blur]} · ${c.issue || "live"} · snap ${c.guides.length}`;
};
const paintCam = () => {
	camOv.innerHTML = camOverlay(c);
	document.getElementById("cam-line").textContent = camDescribe();
};

camDesk.addEventListener("pointerdown", (e) => {
	if (e.target.closest(".ui") || c.off) return;
	const p = pointIn(camDesk, e);
	if (!inRect(p, rectOf(c))) { c.card = false; c.focus = false; paintCam(); return; }
	c.focus = true;
	const corner = cornerAt(c, p);
	c.drag = corner ? { kind: "resize", corner, at: p, size: c.size, sx: c.x, sy: c.y } : { kind: "move", at: p, sx: c.x, sy: c.y };
	camDesk.setPointerCapture(e.pointerId);
	paintCam();
});
camDesk.addEventListener("pointermove", (e) => {
	const p = pointIn(camDesk, e);
	const d = c.drag;
	if (!d) {
		const next = !c.off && hitsWindow(c, p);
		if (next !== c.hover) { c.hover = next; paintCam(); }
		return;
	}
	if (d.kind === "move") {
		const r = rectOf(c);
		const free = { x: clamp(d.sx + p.x - d.at.x, 0, W - r.w), y: clamp(d.sy + p.y - d.at.y, 0, H - r.h), w: r.w, h: r.h };
		const hit = e.ctrlKey ? { x: free.x, y: free.y, guides: [] } : snap(free);
		c.x = Math.round(hit.x);
		c.y = Math.round(hit.y);
		c.guides = hit.guides;
	} else {
		const dx = d.corner.includes("e") ? p.x - d.at.x : d.at.x - p.x;
		const dy = d.corner.includes("s") ? p.y - d.at.y : d.at.y - p.y;
		const next = Math.round(clamp(d.size + Math.max(dx, dy), MIN, MAX));
		c.size = next;
		if (d.corner.includes("w")) c.x = d.sx + (d.size - next) * (c.shape === "wide" ? WIDE : 1);
		if (d.corner.includes("n")) c.y = d.sy + d.size - next;
	}
	paintCam();
});
camDesk.addEventListener("pointerup", () => { c.drag = null; c.guides = []; paintCam(); });
camDesk.addEventListener("pointerleave", () => { if (!c.drag && c.hover) { c.hover = false; paintCam(); } });

const keepOn = () => {
	const r = rectOf(c);
	c.x = clamp(c.x, 0, W - r.w);
	c.y = clamp(c.y, 0, H - r.h);
};
const camAct = (a, v) => {
	if (a === "shape") c.shape = { round: "square", square: "wide", wide: "round" }[c.shape];
	if (a === "set-shape") c.shape = v;
	if (a === "size") c.size = c.size < 400 ? 400 : 230;
	if (a === "set-size") c.size = Number(v);
	if (a === "mirror") c.mirror = !c.mirror;
	if (a === "blur") c.blur = (c.blur + 1) % 3;
	if (a === "set-blur") c.blur = Number(v);
	if (a === "card") c.card = !c.card;
	if (a === "close") { c.off = true; c.card = false; c.hover = false; }
	if (a === "on") c.off = false;
	keepOn();
	paintCam();
};
camOv.addEventListener("click", (e) => {
	const b = e.target.closest("[data-a]");
	if (b) camAct(b.dataset.a, b.dataset.v);
});
camDesk.addEventListener("keydown", (e) => {
	const dir = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[e.key];
	if (dir) {
		e.preventDefault();
		e.stopPropagation();
		const step = e.shiftKey ? 10 : 1;
		c.x += dir[0] * step;
		c.y += dir[1] * step;
		c.focus = true;
		keepOn();
		paintCam();
		return;
	}
	const k = e.key.toLowerCase();
	if (k === "s") camAct("shape");
	if (k === "m") camAct("mirror");
	if (k === "b") camAct("blur");
	if (k === "+" || k === "=") { c.size = clamp(c.size + 10, MIN, MAX); keepOn(); paintCam(); }
	if (k === "-") { c.size = clamp(c.size - 10, MIN, MAX); keepOn(); paintCam(); }
	if (k === "delete") camAct("close");
	if (k === "escape") { c.focus = false; c.card = false; paintCam(); }
});

const scripts = {
	en: "Hi, I'm Lin. Today I'll show you how to record a product demo in five minutes. Open FrameEver and pick the window you want to show. Turn your camera on so people can see who's talking. Paste your script here, then press play and look straight at the lens. If you lose your place, pause the recording. The script waits for you. When you finish, the editor opens with your camera on its own track, so you can move it anywhere. Thanks for watching.",
	zh: "大家好，我是林。今天用五分钟演示怎么录一段产品介绍。打开 FrameEver，选中要展示的窗口。打开摄像头，让观众看到是谁在讲。把文稿贴到这里，按下播放，看着镜头说话。如果忘了说到哪，就暂停录制，文稿会等你。录完后，编辑器会打开，摄像头单独占一条轨道，放在哪里都可以。谢谢观看。",
};
const sentences = (text) => text.match(/[^.。!?！？,，]+[.。!?！？,，]?\s*/g) ?? [];
const han = /[㐀-鿿]/g;
const countWords = (text) => {
	const hanCount = (text.match(han) ?? []).length;
	const rest = text.replace(han, " ").trim();
	return hanCount + (rest ? rest.split(/\s+/).length : 0);
};
const isHan = (text) => (text.match(han) ?? []).length > text.length / 3;

const t = {
	v: "B", x: 340, y: 0, script: null, view: "read", playing: false, mode: "fixed", pos: 0, wpmEn: 150, wpmZh: 260, font: 28, line: 1.35, cue: true, mirror: false, align: "center", startWith: true, hideShare: true, sets: false, phase: "idle", speaking: true, ended: false, sec: 0,
};
const tpVariants = { A: ["Small window", "小窗口"], B: ["Strip under the lens", "镜头下的条"], C: ["Tall column", "竖向窄栏"] };
const tpSizes = {
	A: { read: [560, 320], edit: [560, 360] },
	B: { read: [660, 156], edit: [660, 380] },
	C: { read: [340, 560], edit: [340, 560] },
};
const scriptText = (st) => st.script ?? scripts[page.lang === "en" ? "en" : "zh"];
const wpmOf = (st, text) => (isHan(text) ? st.wpmZh : st.wpmEn);
const unit = (text) => (isHan(text) ? pick("chars/min", "字/分") : pick("wpm", "词/分"));
const totalSec = (st, text) => Math.round((countWords(text) / wpmOf(st, text)) * 60);

const statusOf = (st) => {
	if (st.phase === "streamed") return ["warn", "triangle-alert", ["Can't hide here, recordings may show this", "这台电脑上无法隐藏，可能会录进去"]];
	if (st.phase === "paused") return ["warn", "pause", ["Paused with the recording", "随录制暂停"]];
	if (st.phase === "recording") return ["", "eye-off", ["Hidden from this recording", "这次录制看不到这个窗口"]];
	if (st.hideShare) return ["", "eye-off", ["Hidden from recordings and sharing", "录制和屏幕共享都看不到"]];
	return ["", "eye-off", ["Hidden from recordings", "录制时隐藏"]];
};
const status = (st) => {
	const [cls, ic, text] = statusOf(st);
	return `<span class="tp-status ${cls}">${icon(ic, "sm")}${L(...text)}</span>`;
};
const caps = () => `<span class="tp-cap"><button aria-label="${pick("Minimize", "最小化")}" title="${pick("Minimize", "最小化")}">${icon("minus", "sm")}</button><button aria-label="${pick("Close", "关闭")}" title="${pick("Close", "关闭")}">${icon("x", "sm")}</button></span>`;
const tBtn = (a, ic, title, cls = "", extra = "") => `<button class="rb-btn ${cls}" data-a="${a}" title="${title}" aria-label="${title}" ${extra}>${ic}</button>`;
const playBtn = (st, text) => {
	const label = st.ended ? pick("Start over  Space", "重新开始  Space") : st.playing ? pick("Pause  Space", "暂停  Space") : pick("Play  Space", "播放  Space");
	return tBtn("play", st.ended ? icon("rotate-ccw") : st.playing ? icon("pause", "solid") : icon("play", "solid"), label, "", text.trim() ? "" : "disabled");
};
const speed = (st, text) => `${tBtn("slower", icon("minus", "sm"), pick("Slower  ↓", "慢一点  ↓"))}<span class="num tnum">${wpmOf(st, text)} ${unit(text)}</span>${tBtn("faster", icon("plus", "sm"), pick("Faster  ↑", "快一点  ↑"))}`;
const font = (st) => `${tBtn("smaller", icon("minus", "sm"), pick("Smaller text  -", "字小一点  -"))}<span class="num short tnum">${st.font}</span>${tBtn("larger", icon("plus", "sm"), pick("Larger text  +", "字大一点  +"))}`;
const left = (st, text) => `<span class="left tnum" data-left>${fmt(Math.max(0, totalSec(st, text)))}</span>`;
const voiceChip = (st) => (st.mode === "voice" && st.playing ? `<span class="left">${st.speaking ? L("Following you", "跟着你说") : L("Waiting for you", "等你开口")}</span>` : "");

const textBlock = (st, text, lineH) => {
	const cls = st.align === "left" || st.v === "C" ? "left" : "center";
	return `<div class="tp-text ${cls} ${st.mirror ? "flip" : ""}" style="font-size:${st.font}px;line-height:${lineH}px">${sentences(text).map((part) => `<span>${part}</span>`).join("")}</div>`;
};
const bandAt = (st, boxH, lineH) => {
	const ratio = st.v === "B" ? 0.2 : st.v === "C" ? 0.24 : 0.36;
	return Math.round(boxH * ratio);
};

const settings = (st) => {
	const seg = (a, value, options) => `<div class="seg">${options.map(([v, label]) => `<button class="${value === v ? "on" : ""}" data-a="${a}" data-v="${v}">${label}</button>`).join("")}</div>`;
	const sw = (a, label, on) => `<button class="pop-row" data-a="${a}"><span class="grow">${label}</span><span class="switch ${on ? "on" : ""}"></span></button>`;
	return `<div class="tp-sets ui"><div class="pop-opt"><span class="grow">${L("Scroll", "滚动")}</span>${seg("mode", st.mode, [["fixed", L("Fixed speed", "固定速度")], ["voice", L("When I speak", "说话时")]])}</div><div class="pop-opt"><span class="grow">${L("Line spacing", "行距")}</span>${seg("line", String(st.line), [["1.2", "1.2"], ["1.35", "1.35"], ["1.6", "1.6"]])}</div>${st.v === "C" ? "" : `<div class="pop-opt"><span class="grow">${L("Align", "对齐")}</span>${seg("align", st.align, [["center", L("Center", "居中")], ["left", L("Left", "靠左")]])}</div>`}${sw("startWith", L("Start with recording", "随录制开始"), st.startWith)}${sw("cue", L("Reading line", "阅读线"), st.cue)}${sw("mirror", L("Mirror text", "镜像文字"), st.mirror)}${sw("hideShare", L("Hide from screen sharing", "屏幕共享时隐藏"), st.hideShare)}</div>`;
};

const editView = (st, text, words) => `<div class="tp-edit ui"><textarea id="tp-area" spellcheck="false" aria-label="${pick("Script", "文稿")}" placeholder="${pick("Paste or type your script…", "粘贴或输入文稿…")}">${text}</textarea></div><div class="tp-foot ui">${tBtn("play", icon("play", "solid"), pick("Read  Ctrl Enter", "开始读  Ctrl Enter"), "", text.trim() ? "" : "disabled")}<span class="left tnum" id="tp-count">${words}</span><span class="gap"></span>${tBtn("sets", icon("sliders-horizontal"), pick("Teleprompter options", "提词器选项"), st.sets ? "on" : "")}</div>${st.sets ? settings(st) : ""}`;

const tpWindow = (st, geom) => {
	const text = scriptText(st);
	const [w, h] = (geom ?? tpSizes[st.v])[st.view];
	const lineH = Math.round(st.font * st.line);
	const words = `${countWords(text)} ${isHan(text) ? pick("chars", "字") : pick("words", "词")} · ${pick("about", "约")} ${fmt(totalSec(st, text))}`;
	const place = `left:${st.x}px;top:${st.y}px;width:${w}px;height:${h}px`;
	const top = st.v === "B" && st.y === 0 ? "top" : "";
	const titleBar = `<div class="tp-title" data-drag>${status(st)}${caps()}</div>`;
	if (st.view === "edit") return `<div class="tp ${top}" style="${place}">${titleBar}${editView(st, text, words)}</div>`;
	const boxH = st.v === "B" ? h : h - 76;
	const bandTop = bandAt(st, boxH, lineH);
	const band = st.cue ? `<div class="tp-band" style="top:${bandTop}px;height:${lineH}px"></div><div class="tp-cue" style="left:6px;top:${bandTop}px;height:${lineH}px">${icon("chevron-right", "sm")}</div><div class="tp-cue" style="right:6px;top:${bandTop}px;height:${lineH}px">${icon("chevron-left", "sm")}</div>` : "";
	const read = `<div class="tp-read" data-band="${bandTop}" data-line="${lineH}" ${st.v === "B" ? "data-drag" : ""}>${band}${textBlock(st, text, lineH)}${st.v === "B" ? `<span class="tp-grab"></span>` : ""}</div>`;
	const controls = `${playBtn(st, text)}${voiceChip(st)}${speed(st, text)}<span class="rb-sep"></span>${font(st)}<span class="rb-sep"></span>${left(st, text)}${tBtn("edit", icon("pencil"), pick("Edit script  E", "编辑文稿  E"))}`;
	if (st.v === "B") {
		const show = st.hover || !st.playing;
		const hide = `<div class="rb tp-float ui" role="toolbar">${controls}${tBtn("hide", icon("x"), pick("Close", "关闭"))}</div>`;
		return `<div class="tp ${top}" style="${place}">${read}${show ? hide : ""}</div>`;
	}
	if (st.v === "C") {
		return `<div class="tp" style="${place}">${titleBar}${read}<div class="tp-foot ui">${playBtn(st, text)}${speed(st, text)}<span class="gap"></span>${left(st, text)}${tBtn("edit", icon("pencil"), pick("Edit script  E", "编辑文稿  E"))}</div></div>`;
	}
	return `<div class="tp" style="${place}">${titleBar}${read}<div class="tp-foot ui">${controls.replace('<span class="rb-sep"></span>', '<span class="gap"></span>')}</div></div>`;
};

const lens = () => `<div class="lens"><i></i>${L("Camera lens", "摄像头镜头")}</div>`;
const tpOverlay = (st) => {
	const rec = st.phase === "recording" || st.phase === "paused" || st.phase === "streamed";
	return `${lens()}${st.closed ? `<div class="closed ui"><button class="btn" data-a="reopen">${L("Open teleprompter", "打开提词器")}</button></div>` : tpWindow(st)}${rec ? recBar(st.sec) : ""}`;
};

const place = (root, st) => {
	const read = root.querySelector(".tp-read");
	if (!read) return;
	const inner = read.querySelector(".tp-text");
	const band = Number(read.dataset.band);
	const line = Number(read.dataset.line);
	inner.style.transform = `translateY(${band - st.pos}px)`;
	for (const span of inner.children) span.classList.toggle("past", span.offsetTop + span.offsetHeight < st.pos + line * 0.2);
};
const scrollMax = (root) => {
	const inner = root.querySelector(".tp-text");
	return inner ? Math.max(0, inner.offsetHeight) : 0;
};
const updateLeft = () => {
	const max = scrollMax(tpOv);
	const text = scriptText(t);
	const el = tpOv.querySelector("[data-left]");
	if (el && max) el.textContent = fmt(Math.max(0, Math.round(totalSec(t, text) * (1 - t.pos / max))));
};

const tpDescribe = () => {
	const text = scriptText(t);
	const [w, h] = tpSizes[t.v][t.view];
	return `teleprompter ${t.v} · ${t.view} · ${t.phase} · ${t.playing ? "playing" : "stopped"} · ${t.mode} · ${w} × ${h} at ${t.x}, ${t.y} · ${countWords(text)} ${isHan(text) ? "chars" : "words"} · ${wpmOf(t, text)} per min · font ${t.font} · line ${t.line}`;
};
const paintTp = () => {
	tpOv.innerHTML = tpOverlay(t);
	place(tpOv, t);
	updateLeft();
	document.getElementById("tp-line").textContent = tpDescribe();
};

let last = 0;
let voiceClock = 0;
const tick = (now) => {
	const dt = Math.min(0.05, (now - last) / 1000);
	last = now;
	const running = t.playing && t.view === "read" && t.phase !== "paused";
	if (running) {
		const max = scrollMax(tpOv);
		const text = scriptText(t);
		voiceClock = (voiceClock + dt) % 4;
		const speaking = t.mode === "fixed" || voiceClock < 2.8;
		if (speaking !== t.speaking) { t.speaking = speaking; paintTp(); }
		if (speaking && max) {
			t.pos = Math.min(max, t.pos + (max / Math.max(1, totalSec(t, text))) * dt);
			place(tpOv, t);
			updateLeft();
			if (t.pos >= max) { t.playing = false; t.ended = true; paintTp(); }
		}
	}
	requestAnimationFrame(tick);
};
requestAnimationFrame(tick);

const stepWpm = (st, text, d) => {
	if (isHan(text)) st.wpmZh = clamp(st.wpmZh + d, 60, 420);
	else st.wpmEn = clamp(st.wpmEn + d, 60, 350);
};
const tpAct = (a, v) => {
	const text = scriptText(t);
	if (a === "play") {
		if (t.view === "edit") { t.view = "read"; t.sets = false; t.pos = 0; t.playing = true; t.ended = false; }
		else if (t.ended) { t.pos = 0; t.ended = false; t.playing = true; }
		else t.playing = !t.playing;
	}
	if (a === "edit") { t.view = "edit"; t.playing = false; }
	if (a === "sets") t.sets = !t.sets;
	if (a === "slower") stepWpm(t, text, -10);
	if (a === "faster") stepWpm(t, text, 10);
	if (a === "smaller") t.font = clamp(t.font - 2, 22, 52);
	if (a === "larger") t.font = clamp(t.font + 2, 22, 52);
	if (a === "mode") t.mode = v;
	if (a === "line") t.line = Number(v);
	if (a === "align") t.align = v;
	if (a === "cue" || a === "mirror" || a === "startWith" || a === "hideShare") t[a] = !t[a];
	if (a === "hide") t.closed = true;
	if (a === "reopen") t.closed = false;
	paintTp();
};
tpOv.addEventListener("click", (e) => {
	const b = e.target.closest("[data-a]");
	if (b && !b.disabled) tpAct(b.dataset.a, b.dataset.v);
});
tpOv.addEventListener("input", (e) => {
	if (e.target.id !== "tp-area") return;
	t.script = e.target.value;
	const text = t.script;
	document.getElementById("tp-count").textContent = `${countWords(text)} ${isHan(text) ? pick("chars", "字") : pick("words", "词")} · ${pick("about", "约")} ${fmt(totalSec(t, text))}`;
});
let tpDrag = null;
tpDesk.addEventListener("pointerdown", (e) => {
	if (e.target.closest(".ui") || !e.target.closest("[data-drag]")) return;
	tpDrag = { at: pointIn(tpDesk, e), x: t.x, y: t.y };
	tpDesk.setPointerCapture(e.pointerId);
});
tpDesk.addEventListener("pointermove", (e) => {
	const p = pointIn(tpDesk, e);
	const [w, h] = tpSizes[t.v][t.view];
	if (tpDrag) {
		t.x = Math.round(clamp(tpDrag.x + p.x - tpDrag.at.x, 0, W - w));
		const y = clamp(tpDrag.y + p.y - tpDrag.at.y, 0, H - h);
		t.y = Math.round(y <= SNAP ? 0 : y);
		const centred = W / 2 - w / 2;
		if (Math.abs(t.x - centred) <= SNAP) t.x = centred;
		paintTp();
		return;
	}
	const hover = p.x >= t.x && p.x <= t.x + w && p.y >= t.y && p.y <= t.y + h;
	if (hover !== t.hover) { t.hover = hover; if (t.v === "B") paintTp(); }
});
tpDesk.addEventListener("pointerup", () => { tpDrag = null; });
tpDesk.addEventListener("keydown", (e) => {
	if (e.target.id === "tp-area") {
		if (e.key === "Enter" && e.ctrlKey) { e.preventDefault(); tpAct("play"); }
		if (e.key === "Escape") { t.view = "read"; paintTp(); tpDesk.focus(); }
		e.stopPropagation();
		return;
	}
	const text = scriptText(t);
	if (e.key === " ") { e.preventDefault(); tpAct("play"); }
	if (e.key === "ArrowUp") { e.preventDefault(); e.stopPropagation(); tpAct("faster"); }
	if (e.key === "ArrowDown") { e.preventDefault(); e.stopPropagation(); tpAct("slower"); }
	if (e.key === "+" || e.key === "=") tpAct("larger");
	if (e.key === "-") tpAct("smaller");
	if (e.key.toLowerCase() === "e") tpAct("edit");
	if (e.key === "PageDown" || e.key === "PageUp") {
		e.preventDefault();
		const line = Math.round(t.font * t.line);
		t.pos = clamp(t.pos + (e.key === "PageDown" ? 3 : -3) * line, 0, scrollMax(tpOv));
		t.ended = false;
		place(tpOv, t);
		updateLeft();
	}
	if (text && e.key === "Home") { t.pos = 0; t.ended = false; paintTp(); }
});

const defaultPlace = () => {
	const [w] = tpSizes[t.v].read;
	t.x = Math.round(W / 2 - w / 2);
	t.y = t.v === "B" ? 0 : 28;
	if (t.v === "C") t.x = Math.round(W / 2 - w / 2);
};

const segHTML = (name, value, options) => `<div class="seg" data-seg="${name}">${options.map(([v, en, zh]) => `<button class="${value === v ? "on" : ""}" data-v="${v}">${L(en, zh)}</button>`).join("")}</div>`;
const renderTools = () => {
	document.getElementById("cam-tools").innerHTML = `${segHTML("cam-phase", c.phase, [["idle", "Before recording", "录制前"], ["studio", "Recording Studio", "录制 Studio"], ["instant", "Recording Instant", "录制即时"]])}${segHTML("cam-issue", c.issue, [["", "Live", "正常"], ["loading", "Starting", "启动中"], ["access", "No access", "无权限"], ["busy", "In use", "被占用"], ["lost", "Disconnected", "已断开"]])}`;
	document.getElementById("tp-tools").innerHTML = `${segHTML("tp-view", t.view, [["read", "Read", "阅读"], ["edit", "Edit", "编辑"]])}${segHTML("tp-phase", t.phase, [["idle", "Not recording", "未录制"], ["recording", "Recording", "录制中"], ["paused", "Paused", "已暂停"], ["streamed", "Remote desktop", "远程桌面"]])}`;
};
const renderBar = () => {
	const row = (name, label, list, value) => `<div><span>${label}</span><button data-p="${name}" data-d="-1" aria-label="${pick("Previous", "上一个")}">${icon("chevron-left", "sm")}</button><b>${value} · ${pick(...list[value])}</b><button data-p="${name}" data-d="1" aria-label="${pick("Next", "下一个")}">${icon("chevron-right", "sm")}</button></div>`;
	document.getElementById("proto-bar").innerHTML = `${row("cam", pick("Camera", "摄像头"), camVariants, c.v)}${row("tp", pick("Teleprompter", "提词器"), tpVariants, t.v)}`;
	document.getElementById("cam-note").innerHTML = {
		A: L("<b>Recommended.</b> A 36 px bar in the recording bar's style, 8 px above the bubble. The window is 44 px taller than the bubble.", "<b>推荐。</b>36 px 的条，样式同录制控制条，在气泡上方 8 px。窗口比气泡高 44 px。"),
		B: L("The bar sits on the picture. The window is exactly the bubble, but the bar covers your chin and a small round bubble fits only three buttons.", "条压在画面上。窗口和气泡一样大，但条会挡住下巴，小的圆形气泡只放得下三个按钮。"),
		C: L("One button on the picture opens a card with direct choices instead of cycling. It's the cleanest at rest, but every change takes two clicks and the card needs its own window.", "画面上只有一个按钮，点开卡片直接选，不用循环切换。平时最干净，但每次改动要点两下，卡片还要单独一个窗口。"),
	}[c.v];
	document.getElementById("tp-note").innerHTML = {
		A: L("Today's size and layout at the top of the display. The most familiar, but the reading line sits a third of the way down, further from the lens.", "现在的尺寸和布局，放在屏幕顶端。最熟悉，但阅读线在窗口三分之一处，离镜头更远。"),
		B: L("<b>Recommended.</b> A 660 × 156 strip attached to the top edge, right under a laptop's lens. No title bar; controls show on hover and while stopped. It opens to 660 × 380 for editing.", "<b>推荐。</b>660 × 156 的条贴在屏幕上沿，正好在笔记本镜头下方。没有标题栏；控件在悬停和停止时显示。编辑时展开到 660 × 380。"),
		C: L("A 340 × 560 column with left-aligned short lines, placed under the lens. It shows the most text at once, and suits Chinese, but covers the most screen.", "340 × 560 的竖栏，短行靠左，放在镜头下方。一次看到的字最多，也适合中文，但挡住的屏幕最多。"),
	}[t.v];
};

const fit = () => {
	for (const stage of document.querySelectorAll(".stage")) stage.style.setProperty("--k", stage.clientWidth / W);
	for (const m of document.querySelectorAll(".mini")) {
		const inner = m.querySelector(":scope > .mini-in");
		if (inner) inner.style.transform = `scale(${m.clientWidth / W})`;
	}
};
new ResizeObserver(fit).observe(document.getElementById("cam-stage"));

const mini = (html) => `<div class="mini"><div class="mini-in">${wallHTML}${html}</div></div>`;
const card = (en, zh, html) => `<div class="mini-card"><p class="label">${L(en, zh)}</p>${mini(html)}</div>`;
const renderMinis = () => {
	const base = { ...c, x: 500, y: 200, size: 280, shape: "round", mirror: false, blur: 0, issue: "", hover: false, focus: false, card: false, drag: null, guides: [], phase: "idle", off: false };
	const camCard = (en, zh, patch, extra = "") => {
		const st = { ...base, ...patch };
		return card(en, zh, `${camOverlay(st)}${extra}`);
	};
	document.getElementById("cam-minis").innerHTML = [
		camCard("Hover, round", "悬停，圆形", { hover: true }),
		camCard("Hover, square", "悬停，方形", { hover: true, shape: "square" }),
		camCard("Hover, wide", "悬停，宽屏", { hover: true, shape: "wide", x: 400 }),
		camCard("Smallest: blur moves off the bar", "最小：模糊按钮收起", { hover: true, size: 150, x: 560, y: 260 }),
		camCard("At the top edge, the bar flips below", "贴近顶端时条翻到下方", { hover: true, y: 16, x: 500 }),
		camCard("Light blur, mirrored", "轻度模糊，镜像", { blur: 1, mirror: true, hover: true }),
		camCard("Starting", "启动中", { issue: "loading" }),
		camCard("No access", "没有权限", { issue: "access" }),
		camCard("In use by another app", "被其他应用占用", { issue: "busy", shape: "square" }),
		camCard("Disconnected during a recording", "录制中断开", { issue: "lost", phase: "studio", sec: 84 }),
		camCard("Instant recording: no controls on hover", "即时录制：悬停不显示控件", { hover: true, phase: "instant", sec: 42 }),
		card("Parked inside a picked window", "停在选中的窗口里", `<div class="sc" style="left:0;top:0;width:96px;height:720px"></div><div class="sc" style="left:716px;top:0;width:564px;height:720px"></div><div class="sc" style="left:96px;top:0;width:620px;height:56px"></div><div class="sc" style="left:96px;top:496px;width:620px;height:224px"></div><div class="frame" style="left:96px;top:56px;width:620px;height:440px"></div>${bubble({ ...base, size: 150 }, { x: 550, y: 330, w: 150, h: 150 })}`),
		card("Camera only: large in the picker", "仅摄像头：在选择器里放大", `<div class="sc full"></div>${bubble({ ...base, shape: "wide", size: 400 }, { x: 284, y: 104, w: 712, h: 400 })}<div class="tb ui" style="left:50%;top:520px;transform:translateX(-50%)"><button class="tb-txt">${icon("video", "sm")}Logitech C920${icon("chevron-down", "sm")}</button><span class="rb-sep"></span><button class="tb-start">${FE.recordGlyph()}${L("Start recording", "开始录制")}</button></div>`),
	].join("");
	const tBase = { ...t, x: 310, y: 0, view: "read", playing: false, sets: false, pos: 120, phase: "idle", script: null, hover: false, ended: false, closed: false, mode: "fixed" };
	const tpCard = (en, zh, patch) => {
		const st = { ...tBase, ...patch };
		const [w] = tpSizes[st.v][st.view];
		st.x = Math.round(W / 2 - w / 2);
		st.y = st.v === "B" ? 0 : 28;
		return `<div class="mini-card" data-tp='${JSON.stringify({ pos: st.pos })}'><p class="label">${L(en, zh)}</p>${mini(`${lens()}${tpWindow(st)}`)}</div>`;
	};
	const grid = document.getElementById("tp-minis");
	grid.innerHTML = [
		tpCard("Ready", "就绪", { pos: 0 }),
		tpCard("Reading, not hovered", "正在读，没有悬停", { playing: true, hover: false }),
		tpCard("Reading, hovered", "正在读，悬停", { playing: true, hover: true }),
		tpCard("Paused with the recording", "随录制暂停", { phase: "paused", hover: true }),
		tpCard("Reached the end", "读到结尾", { ended: true, pos: 9999, hover: true }),
		tpCard("When I speak, waiting", "说话时滚动，等你开口", { playing: true, hover: true, mode: "voice", speaking: false }),
		tpCard("Editing", "编辑", { view: "edit", phase: "recording" }),
		tpCard("Empty", "没有文稿", { view: "edit", script: "" }),
		tpCard("Options", "选项", { view: "edit", sets: true }),
		tpCard("Remote desktop: can't hide", "远程桌面：无法隐藏", { view: "edit", phase: "streamed" }),
	].join("");
	for (const el of grid.querySelectorAll("[data-tp]")) place(el, JSON.parse(el.dataset.tp));
	const rec = { ...base, x: 934, y: 356, size: 230 };
	document.getElementById("cap-minis").innerHTML = [
		card("Studio: the screen track", "Studio：屏幕轨道", `<div class="rec-note">${L("No bubble, no teleprompter", "没有气泡，没有提词器")}</div>`),
		card("Studio: the camera track", "Studio：摄像头轨道", `<div class="wall" style="background:#000"></div>${bubble({ ...base, shape: "wide", size: 720 }, { x: 0, y: 0, w: 1280, h: 720 })}<div class="rec-note">${L("Not mirrored, placed in the editor", "不镜像，位置在编辑器里定")}</div>`),
		card("Instant: one picture", "即时：只有一个画面", `${bubble({ ...rec, mirror: true }, { x: 934, y: 356, w: 230, h: 230 })}<div class="rec-note">${L("The bubble is part of the picture, mirrored if you mirrored it", "气泡就在画面里，开了镜像就是镜像的")}</div>`),
	].join("");
	fit();
};

const table = (id, head, rows) => {
	document.getElementById(id).innerHTML = `<table class="inv"><thead><tr>${head.map(([en, zh]) => `<th>${L(en, zh)}</th>`).join("")}</tr></thead><tbody>${rows.map((row) => `<tr>${row.map((cell, i) => `<td class="${i ? "dim" : ""}">${Array.isArray(cell) ? L(...cell) : cell}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
};

const copy = () => {
	const text = {
		"t-cam": ["Camera bubble", "摄像头气泡"],
		"l-cam": ["The stage is a display at 1280 × 720. Move the pointer over the bubble, drag it toward an edge, drag a corner, or click it and use the keyboard. The bottom bar switches between the three structures.", "舞台是一台 1280 × 720 的显示器。把指针移到气泡上，拖向屏幕边缘，拖动四角，或者点击后用键盘。底部的条用来切换三种结构。"],
		"t-cs": ["Camera states", "摄像头状态"],
		"l-cs": ["These follow the structure picked in the bottom bar.", "这些状态跟随底部条选中的结构。"],
		"t-tp": ["Teleprompter", "提词器"],
		"l-tp": ["Press play to scroll. Drag the window, or switch to Edit and change the script. In Chinese the speed counts characters.", "按播放开始滚动。可以拖动窗口，也可以切到编辑改文稿。中文按字数算速度。"],
		"t-ts": ["Teleprompter states", "提词器状态"],
		"l-ts": ["These follow the structure picked in the bottom bar.", "这些状态跟随底部条选中的结构。"],
		"t-cap": ["What ends up in the recording", "录出来的是什么"],
		"l-cap": ["Studio and Instant treat the bubble differently, and the design has to tell the truth about both.", "Studio 和即时对气泡的处理不同，设计要如实反映这两种情况。"],
		"t-rules": ["Rules", "规则"],
		"l-rules": ["Why it looks and behaves this way.", "为什么这样设计。"],
		"t-keys": ["Keyboard", "键盘"],
		"l-keys": ["Both windows work without a mouse once they have focus. Focus reaches them with Tab from the main window or through the two new global shortcuts.", "两个窗口拿到焦点后都能不用鼠标操作。从主窗口按 Tab，或者用新增的两个全局快捷键，都能把焦点带过去。"],
		"t-inv": ["Where everything went", "每个功能的去向"],
		"l-inv": ["Compared with our windows and upstream f7d351611. Everything stays except the opacity slider, which has never worked on Windows.", "对照了我们的窗口和上游 f7d351611。除了在 Windows 上一直没有作用的透明度滑块，其他都保留。"],
		"t-add": ["Added from other products", "从其他产品加入"],
		"l-add": ["Open-source and popular tools already have these. Each has its real cost beside it.", "这些开源和热门工具已经有了。每一项都写明了实际代价。"],
		"t-ref": ["What this is built on", "这份设计的依据"],
		"l-ref": ["Our own decisions first, then primary sources.", "先看我们自己的决定，再看一手资料。"],
	};
	for (const [id, pair] of Object.entries(text)) document.getElementById(id).innerHTML = L(...pair);
	table("cap-table", [["", ""], ["Studio", "Studio"], ["Instant", "即时"]], [
		[["Bubble on screen", "屏幕上的气泡"], ["Not in the screen track. The camera is its own track", "不进屏幕轨道。摄像头单独一条轨道"], ["In the picture, exactly as it looks", "就在画面里，看到什么录到什么"]],
		[["Controls on hover", "悬停时的控件"], ["Shown: nobody records them", "显示：不会被录进去"], ["Hidden while recording: they'd be in the video. Drag still moves the bubble", "录制时隐藏：否则会录进视频。仍然可以拖动气泡"]],
		[["Moving it during the take", "录制中移动"], ["No effect on the video: placement is taken once at the start, into one of six slots", "不影响视频：位置在开始时取一次，落到六个位置之一"], ["Moves in the video", "视频里也跟着动"]],
		[["Mirror", "镜像"], ["Preview only", "只影响预览"], ["Recorded, because the picture is what's captured", "会录进去，因为录的就是这个画面"]],
		[["Teleprompter", "提词器"], ["Never recorded, by window exclusion and by Windows display affinity", "不会录进去，同时靠窗口排除和 Windows 显示亲和性"], ["The same", "相同"]],
		[["Remote desktop and streamed sessions", "远程桌面和串流环境"], ["Hiding is skipped, so the window may be recorded. The teleprompter says so in amber. Tracked in Hide FrameEver windows over remote desktop", "不做隐藏，窗口可能被录进去。提词器用琥珀色提示。后续在“远程桌面下隐藏 FrameEver 窗口”那张票处理"], ["The same", "相同"]],
	]);
	table("rules", [["Rule", "规则"], ["Why", "原因"]], [
		[["The bubble has no border and no shadow. The picture is its edge", "气泡没有边框和阴影，画面本身就是边界"], ["Flat, and previews stay true neutral. Any ring would also be recorded in Instant", "纯平设计，预览保持中性。任何描边在即时录制里也会被录进去"]],
		[["Both windows follow the app theme; the camera window is no longer forced dark", "两个窗口都跟随应用主题；摄像头窗口不再强制深色"], ["The bar is the same object as the recording bar", "这个条和录制控制条是同一种东西"]],
		[["The bar stays 36 px at every bubble size. Today it scales from 70% to 100%", "条在任何气泡尺寸下都是 36 px。现在是 70% 到 100% 缩放"], ["Scaling breaks the 14/16/20 px icon sizes and goes soft at 100%", "缩放不符合 14/16/20 px 图标尺寸，100% 时会发虚"]],
		[["When the bubble is too narrow, blur leaves first, then mirror", "气泡太窄时，先收起模糊，再收起镜像"], ["Upstream's order. They stay on the keyboard and in Recording options", "上游的顺序。这两项仍可用键盘和录制选项操作"]],
		[["Loading is a still camera icon. The drifting clouds and sweep go", "加载状态是静止的摄像头图标，飘动的云和扫光去掉"], ["They use gradients and run continuously beside a recording preview, which the system rules out", "它们用了渐变，还在录制预览旁持续运动，这两点设计系统都不允许"]],
		[["Camera messages fit a 150 px circle: a title and one short line", "摄像头提示要放得进 150 px 的圆：一行标题加一句短话"], ["Today's sentences are up to 110 characters at 10 px", "现在的句子最长 110 个字符，字号 10 px"]],
		[["Square and wide corners are 20 px, was 24", "方形和宽屏的圆角是 20 px，原来是 24"], ["It's the radius-xl token, the same as the recording bar", "这是 radius-xl token，和录制控制条相同"]],
		[["The gradient fade is replaced by a flat reading band. Text that has passed turns muted", "渐变淡出换成纯色阅读带。读过的文字变灰"], ["No gradients, and it needs no mask, which GPUI lacks", "不用渐变，也不需要 GPUI 没有的遮罩"]],
		[["Editing and reading are separate views", "编辑和阅读分成两个视图"], ["Reading needs per-line colour and Space for play, which a text field can't give", "阅读需要逐行变色，还要用 Space 播放，文本框做不到"]],
		[["Speed in Chinese counts characters and shows 字/分. Default 260, English stays 150 wpm", "中文按字数算速度，显示“字/分”。默认 260，英文保持 150 词/分"], ["Today's count splits on spaces, so a Chinese script counts as one word and, going by the code, finishes in a second", "现在按空格分词，一段中文只算一个词，从代码看一秒就滚完"]],
		[["The status line says what's true now, in amber when it can't hide", "状态行只写当前的真实情况，无法隐藏时用琥珀色"], ["Today it always says hidden, even over remote desktop where hiding is skipped", "现在一直写“已隐藏”，远程桌面下其实没有隐藏"]],
		[["Klein blue doesn't appear in either window", "两个窗口都不用克莱因蓝"], ["Neither has a primary action in the sense the blue list means; the cue chevrons were blue and become neutral", "两个窗口都没有蓝色白名单意义上的主操作；原本蓝色的提示箭头改成中性色"]],
	]);
	table("keys", [["Action", "操作"], ["Key", "按键"], ["Source", "来源"]], [
		[["Focus the camera bubble, anywhere", "全局聚焦摄像头气泡"], keys("Ctrl", "Shift", "C"), ["New; checked for conflicts during implementation", "新增；实现时检查冲突"]],
		[["Play or pause the teleprompter, anywhere", "全局播放或暂停提词器"], keys("Ctrl", "Shift", "T"), ["New; only registered while the window is open", "新增；只在窗口打开时注册"]],
		[["Move the bubble", "移动气泡"], `${keys("←")} ${keys("↑")} ${keys("↓")} ${keys("→")}`, ["New; Shift for 10", "新增；Shift 每次 10"]],
		[["Resize the bubble", "调整气泡大小"], `${keys("+")} ${keys("-")}`, ["New", "新增"]],
		[["Shape, mirror, blur", "形状、镜像、模糊"], `${keys("S")} ${keys("M")} ${keys("B")}`, ["New", "新增"]],
		[["Turn the camera off", "关闭摄像头"], keys("Del"), ["New", "新增"]],
		[["Snap off while dragging", "拖动时关闭吸附"], keys("Ctrl"), ["Same as the area picker", "和区域选择一致"]],
		[["Play or pause", "播放或暂停"], keys("Space"), ["New in reading view", "阅读视图新增"]],
		[["Speed", "速度"], `${keys("↑")} ${keys("↓")}`, ["New", "新增"]],
		[["Text size", "字号"], `${keys("+")} ${keys("-")}`, ["New", "新增"]],
		[["Jump three lines", "跳三行"], `${keys("PgUp")} ${keys("PgDn")}`, ["New, as in QPrompt's markers", "新增，类似 QPrompt 的标记跳转"]],
		[["Back to the top", "回到开头"], keys("Home"), ["New", "新增"]],
		[["Edit, or back to reading", "编辑，或回到阅读"], `${keys("E")} ${keys("Esc")}`, ["New", "新增"]],
		[["Read from the editor", "从编辑开始读"], keys("Ctrl", "Enter"), ["New", "新增"]],
	]);
	table("inv", [["Capability", "功能"], ["Today", "现状"], ["In this design", "这份设计"]], [
		[["Round, square, wide", "圆形、方形、宽屏"], ["Ours and upstream", "我们和上游都有"], ["Kept", "保留"]],
		[["Small and large presets, free resize 150–600", "大小两档预设，150–600 自由调整"], ["Ours and upstream", "我们和上游都有"], ["Kept, with corner brackets built like selection handles", "保留，四角标记和选区手柄同样构造"]],
		[["Mirror preview", "镜像预览"], ["Ours and upstream", "我们和上游都有"], ["Kept; Instant's recorded result is now stated", "保留；说明了即时录制里的结果"]],
		[["Background blur: off, light, heavy", "背景模糊：关、轻、重"], ["Ours and upstream; Remove is macOS only", "我们和上游都有；移除背景只有 macOS"], ["Kept, with dots for the level instead of a 7 px label. Remove stays for the macOS work", "保留，用圆点表示强度，代替 7 px 文字。移除背景留到 macOS 版"]],
		[["Bar fits a small bubble", "小气泡也放得下条"], ["Upstream only", "仅上游"], ["Port, without the scaling", "移植，去掉缩放"]],
		[["Reopen stays on screen", "重新打开时不跑到屏幕外"], ["Upstream only, f0f744b2c", "仅上游，f0f744b2c"], ["Port", "移植"]],
		[["Position per display", "每台显示器记住位置"], ["Ours and upstream", "我们和上游都有"], ["Kept", "保留"]],
		[["Parked in the picker", "停在选择器里"], ["Ours and upstream", "我们和上游都有"], ["Kept; the bubble keeps its shape with no chrome", "保留；气泡保持形状，没有控件"]],
		[["Camera only target", "仅摄像头目标"], ["Ours and upstream", "我们和上游都有"], ["Kept, with the area picker's bar under the picture", "保留，画面下方用区域选择的控制条"]],
		[["Camera disconnected message", "摄像头断开提示"], ["Tauri had it; GPUI doesn't", "Tauri 有，GPUI 没有"], ["Port, on the bubble and in the recording bar", "移植，气泡和录制控制条上都有"]],
		[["Upstream's off-thread frame conversion", "上游的后台线程帧转换"], ["Upstream only, macOS path", "仅上游，macOS 路径"], ["For the macOS work: no change on Windows", "留到 macOS 版：Windows 不受影响"]],
		[["Script editing and saving", "文稿编辑和保存"], ["Ours and upstream", "我们和上游都有"], ["Kept, in its own view", "保留，放到单独的视图"]],
		[["Speed 60–350", "速度 60–350"], ["Ours and upstream: a slider", "我们和上游：滑块"], ["Kept as a stepper, 10 at a time; Chinese goes to 420", "改成步进，每次 10；中文到 420"]],
		[["Text size 22–52", "字号 22–52"], ["Ours and upstream", "我们和上游都有"], ["Kept", "保留"]],
		[["Line spacing", "行距"], ["Saved, but nothing sets it", "保存了，但没有地方能改"], ["In options", "放进选项"]],
		[["Cue markers", "提示标记"], ["Ours and upstream", "我们和上游都有"], ["Kept as the reading line", "保留，变成阅读线"]],
		[["Mirror text", "镜像文字"], ["Ours and upstream: the switch does nothing, GPUI has no text flip", "我们和上游：开关没有作用，GPUI 不能翻转文字"], ["Kept; needs a GPUI change, sized in implementation", "保留；需要改 GPUI，实现时评估"]],
		[["Window opacity 45–100", "窗口透明度 45–100"], ["Ours and upstream: does nothing on Windows", "我们和上游：Windows 上没有作用"], ["Taken out until it works: a solid window is hidden from capture all the same", "在能用之前先去掉：不透明的窗口同样不会被录到"]],
		[["Pauses with the recording", "随录制暂停"], ["Ours and upstream", "我们和上游都有"], ["Kept", "保留"]],
		[["Resumes where it stopped", "从暂停处继续"], ["Ours and upstream", "我们和上游都有"], ["Kept", "保留"]],
		[["Hidden from recordings", "录制时隐藏"], ["Ours and upstream", "我们和上游都有"], ["Kept, and the status line tells the truth", "保留，状态行写真实情况"]],
		[["Opened from the compact window", "从紧凑窗口打开"], ["A header button", "标题栏按钮"], ["Kept, plus the command palette", "保留，命令面板也能打开"]],
		[["Liquid glass material", "液态玻璃材质"], ["Upstream, macOS", "上游，macOS"], ["Dropped: no glass", "去掉：不用玻璃"]],
	]);
	table("add", [["Feature", "功能"], ["Who has it", "谁有"], ["Cost", "代价"], ["Recommendation", "建议"]], [
		[["Snap to corners and edges", "吸附到角和边"], "Loom", ["Small", "小"], ["Add, with Ctrl to turn it off", "加入，Ctrl 关闭"]],
		[["Start Studio with the bubble's shape and size", "Studio 沿用气泡的形状和大小"], ["A gap we found: only position and blur carry over today", "我们发现的缺口：现在只带过去位置和模糊"], ["Small: written once at the start, no format change", "小：开始时写一次，不改格式"], ["Add", "加入"]],
		[["Portrait and squircle shapes", "竖屏和超椭圆形状"], "Loom", ["Medium: portrait needs a new shape in the project and the editor", "中：竖屏要在项目和编辑器里新增形状"], ["Your call; left out here", "由你决定；这里先不加"]],
		[["Scroll while you speak", "说话时才滚动"], ["Textream, itsbirdo/teleprompter", "Textream、itsbirdo/teleprompter"], ["Small: the microphone level is already live", "小：麦克风音量已经有了"], ["Add", "加入"]],
		[["Follow each word you say", "逐字跟读"], "Textream", ["Large: live Whisper on a CPU is unproven while recording", "大：录制同时跑实时 Whisper，CPU 上没验证过"], ["Leave out of 1.0", "1.0 不加"]],
		[["Start with the recording", "随录制开始"], ["Our idea; itsbirdo has a countdown", "我们的想法；itsbirdo 有倒计时"], ["Small: waits for the countdown", "小：等倒计时结束"], ["Add", "加入"]],
		[["Hide from screen sharing", "屏幕共享时隐藏"], "Textream", ["Small: the same Windows call, kept on while the window is open", "小：同一个 Windows 调用，窗口打开期间保持"], ["Add, on by default", "加入，默认开启"]],
		[["Move the reading line", "移动阅读线"], "QPrompt", ["Small", "小"], ["Add, by dragging it", "加入，拖动调整"]],
		[["Full screen on another display", "在另一台显示器全屏"], ["Textream, QPrompt", "Textream、QPrompt"], ["Medium", "中"], ["Add", "加入"]],
		[["Remember teleprompter position and size", "记住提词器位置和大小"], ["Ours has it for the camera", "我们的摄像头已有"], ["Small", "小"], ["Add", "加入"]],
		[["Time left and script length", "剩余时间和文稿长度"], ["Common; our idea here", "常见做法"], ["Small", "小"], ["Add", "加入"]],
		[["Read on a phone over the local network", "局域网内用手机看"], "Textream", ["It would need a local server, against ADR-0002", "需要本地服务，违背 ADR-0002"], ["Leave out", "不加"]],
		[["Rich text and importing documents", "富文本和导入文档"], "QPrompt", ["Medium, and the script format would change", "中，还要改文稿格式"], ["Leave out: paste plain text", "不加：粘贴纯文本就够了"]],
	]);
	table("refs", [["Source", "来源"], ["What it decided here", "决定了什么"]], [
		['<a href="https://github.com/AniBullet/FrameEver/issues/57">Recording controls</a>', ["Bar size, button size and construction; red, amber and green; Show camera preview in the options", "条和按钮的尺寸和构造；红、琥珀、绿的用法；选项里的摄像头预览开关"]],
		['<a href="https://github.com/AniBullet/FrameEver/issues/58">Area picker</a>', ["Selection handles, snap guides, Ctrl to turn off snapping, the bubble in the picker", "选区手柄、吸附线、Ctrl 关吸附、选择器里的气泡"]],
		['<a href="https://github.com/AniBullet/FrameEver/issues/25">What can GPUI animate</a>', ["No mask, no text flip, each frame redraws the window section", "没有遮罩、不能翻转文字、每帧重绘窗口区域"]],
		['<a href="https://github.com/AniBullet/FrameEver/issues/43">Hide FrameEver windows over remote desktop</a>', ["Why hiding is skipped on streamed sessions", "为什么串流环境下不做隐藏"]],
		[["Upstream Cap f7d351611", "上游 Cap f7d351611"], ["Toolbar fit order, on-screen reopen, every current capability", "工具条收起顺序、重新打开不跑出屏幕、当前全部功能"]],
		['<a href="https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity">SetWindowDisplayAffinity</a>', ["WDA_EXCLUDEFROMCAPTURE needs Windows 10 2004; older systems show an empty window instead", "WDA_EXCLUDEFROMCAPTURE 需要 Windows 10 2004；更早的系统显示为空窗口"]],
		['<a href="https://support.atlassian.com/loom/docs/get-started-with-the-loom-desktop-app/">Loom</a>', ["Snapping to corners and edges, sizes and shapes", "吸附到角和边、尺寸和形状"]],
		['<a href="https://github.com/f/textream">Textream</a>', ["Scroll modes, hide from sharing, full screen, a window near the lens", "滚动模式、共享时隐藏、全屏、贴近镜头的窗口"]],
		['<a href="https://qprompt.app/">QPrompt</a>', ["Adjustable reading region near the camera, jump keys, multiple screens", "贴近镜头且可调的阅读区域、跳转快捷键、多屏"]],
	]);
};

const renderAll = () => {
	renderTools();
	renderBar();
	copy();
	renderMinis();
	paintCam();
	paintTp();
};

document.getElementById("proto-bar").addEventListener("click", (e) => {
	const b = e.target.closest("[data-p]");
	if (!b) return;
	cycle(b.dataset.p, Number(b.dataset.d));
});
const cycle = (name, d) => {
	const list = name === "cam" ? Object.keys(camVariants) : Object.keys(tpVariants);
	const st = name === "cam" ? c : t;
	st.v = list[(list.indexOf(st.v) + d + list.length) % list.length];
	if (name === "tp") { defaultPlace(); t.pos = 0; t.playing = false; t.ended = false; t.view = "read"; t.sets = false; }
	c.card = false;
	renderAll();
};
document.addEventListener("keydown", (e) => {
	if (e.target.closest?.(".desk, textarea, input")) return;
	if (e.key === "ArrowLeft") cycle("cam", -1);
	if (e.key === "ArrowRight") cycle("cam", 1);
});

document.addEventListener("click", (e) => {
	const b = e.target.closest("[data-seg] button");
	if (!b) return;
	const name = b.closest("[data-seg]").dataset.seg;
	const v = b.dataset.v;
	if (name === "cam-phase") { c.phase = v; c.sec = v === "idle" ? 0 : 12; }
	if (name === "cam-issue") c.issue = v;
	if (name === "tp-view") { t.view = v; t.playing = false; t.sets = false; }
	if (name === "tp-phase") {
		t.phase = v;
		t.sec = v === "idle" ? 0 : 12;
		if (v === "recording" && t.startWith && t.view === "read") { t.playing = true; t.ended = false; }
		if (v === "idle") t.playing = false;
	}
	renderTools();
	paintCam();
	paintTp();
});

defaultPlace();
shell({ here: "camera-teleprompter", depth: 1, sections: [["camera", "Camera", "摄像头"], ["camera-states", "Camera states", "摄像头状态"], ["teleprompter", "Teleprompter", "提词器"], ["teleprompter-states", "Teleprompter states", "提词器状态"], ["capture", "In the recording", "录出来的内容"], ["shared", "Rules", "规则"], ["keyboard", "Keyboard", "键盘"], ["inventory", "Everything", "去向"], ["additions", "Additions", "新增"], ["references", "References", "参考"]] });
renderAll();
onChange(renderAll);
