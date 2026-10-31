const { L, icon, pick, shell, onChange } = FE;

Object.assign(window.FRAMEEVER_ICONS, {
	redo: '<path d="M21 7v6h-6"/><path d="M3 17a9 9 0 0 1 9-9 9 9 0 0 1 6 2.3l3 2.7"/>',
	crop: '<path d="M6 2v14a2 2 0 0 0 2 2h14"/><path d="M18 22V8a2 2 0 0 0-2-2H2"/>',
	proportions: '<rect width="20" height="16" x="2" y="4" rx="2"/><path d="M12 9v11"/><path d="M2 9h13a2 2 0 0 1 2 2v9"/>',
	"zoom-in": '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/><path d="M11 8v6M8 11h6"/>',
	"zoom-out": '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/><path d="M8 11h6"/>',
	"mouse-pointer-2": '<path d="m4 4 7.07 17 2.51-7.39L21 11.07z"/>',
	layers: '<path d="m12 2 10 5-10 5L2 7z"/><path d="m2 12 10 5 10-5"/><path d="m2 17 10 5 10-5"/>',
	"eye-off": '<path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49"/><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242"/><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143"/><path d="m2 2 20 20"/>',
	"square-round-corner": '<path d="M21 11a8 8 0 0 0-8-8"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/>',
	line: '<path d="M21 3 3 21"/>',
	highlighter: '<path d="m9 11-6 6v3h9l3-3"/><path d="m22 12-4.6 4.6a2 2 0 0 1-2.8 0l-5.2-5.2a2 2 0 0 1 0-2.8L14 4"/>',
	flashlight: '<path d="M18 6c0 2-2 2-2 4v10a2 2 0 0 1-2 2h-4a2 2 0 0 1-2-2V10c0-2-2-2-2-4V2h12z"/><line x1="6" x2="18" y1="6" y2="6"/><line x1="12" x2="12" y1="12" y2="12"/>',
	"message-square": '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>',
	"image-plus": '<path d="M16 5h6M19 2v6M21 11.5V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7.5"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/><circle cx="9" cy="9" r="2"/>',
	"between-vertical-start": '<rect width="7" height="13" x="3" y="8" rx="1"/><path d="m15 2-3 3-3-3"/><rect width="7" height="13" x="14" y="8" rx="1"/>',
	hash: '<line x1="4" x2="20" y1="9" y2="9"/><line x1="4" x2="20" y1="15" y2="15"/><line x1="10" x2="8" y1="3" y2="21"/><line x1="16" x2="14" y1="3" y2="21"/>',
});

const glyphs = {
	minimize: '<svg viewBox="0 0 10 10"><path d="M0 5.5h10"/></svg>',
	maximize: '<svg viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" rx="1"/></svg>',
	close: '<svg viewBox="0 0 10 10"><path d="m0.5 0.5 9 9m0-9-9 9"/></svg>',
};

const TOOLS = [
	{ id: "select", g: 0, icon: "mouse-pointer-2", en: "Select", zh: "选择", k: "V" },
	{ id: "draw", g: 1, icon: "pencil", en: "Draw", zh: "画笔", k: "D" },
	{ id: "marker", g: 1, icon: "highlighter", en: "Highlighter", zh: "荧光笔", k: "G" },
	{ id: "line", g: 1, icon: "line", en: "Line", zh: "直线", k: "I" },
	{ id: "arrow", g: 1, icon: "arrow-up-right", en: "Arrow", zh: "箭头", k: "A" },
	{ id: "rect", g: 1, icon: "square", en: "Rectangle", zh: "矩形", k: "R" },
	{ id: "circle", g: 1, icon: "circle", en: "Circle", zh: "椭圆", k: "C" },
	{ id: "text", g: 2, icon: "type", en: "Text", zh: "文字", k: "T" },
	{ id: "bubble", g: 2, icon: "message-square", en: "Speech bubble", zh: "对话气泡", k: "U" },
	{ id: "counter", g: 2, icon: "hash", en: "Step number", zh: "序号", k: "N" },
	{ id: "mask", g: 3, icon: "eye-off", en: "Mask", zh: "遮罩", k: "M" },
	{ id: "spot", g: 3, icon: "flashlight", en: "Spotlight", zh: "聚光灯", k: "F" },
	{ id: "magnify", g: 3, icon: "zoom-in", en: "Magnifier", zh: "放大镜", k: "Z" },
	{ id: "image", g: 4, icon: "image-plus", en: "Image", zh: "图片", k: "K" },
	{ id: "cut", g: 9, icon: "between-vertical-start", en: "Cut out", zh: "切割", k: "X" },
];
const toolOf = (id) => TOOLS.find((t) => t.id === id);

const POPS = { b: "background", p: "screen", h: "screen", e: "screen" };
const COLORS = ["#F05656", "#FFFFFF", "#000000", "#F5A524", "#2CB67D", "#4785FF", "#1F3FD8", "#C05BD6"];
const OUT_W = 1440;
const OUT_H = 900;

const seed = () => [
	{ id: 6, t: "marker", x: 216, y: 36, w: 196, h: 34, stroke: "#F5A524", op: 1 },
	{ id: 1, t: "draw", x: 646, y: 92, w: 120, h: 44, stroke: "#F05656", width: 4, op: 1, pts: [[0, 0.6], [0.2, 0.2], [0.4, 0.8], [0.6, 0.3], [0.8, 0.9], [1, 0.4]] },
	{ id: 2, t: "mask", x: 112, y: 408, w: 250, h: 110, mask: "pixelate", level: 7 },
	{ id: 3, t: "rect", x: 540, y: 150, w: 410, h: 170, stroke: "#F05656", width: 4, fill: "transparent", op: 1 },
	{ id: 4, t: "arrow", x: 420, y: 300, w: 108, h: -66, stroke: "#F05656", width: 4, op: 1 },
	{ id: 5, t: "text", x: 300, y: 290, h: 34, stroke: "#F05656", op: 1, en: "Slower since Tuesday", zh: "周二开始变慢" },
];
const S = {
	tab: "background",
	tool: "select",
	sel: 3,
	menu: null,
	dialog: null,
	display: null,
	zoom: 100,
	aspect: 0,
	v: {},
	shut: {},
	anns: seed(),
	past: [],
	future: [],
	next: 7,
	gesture: null,
};
const selected = () => S.anns.find((a) => a.id === S.sel) ?? null;
const remember = () => {
	S.past.push(JSON.stringify(S.anns));
	S.past = S.past.slice(-50);
	S.future = [];
};
const percent = (value, min, max) => ((Number(value) - Number(min)) / (Number(max) - Number(min))) * 100;

const sl = (k, en, zh, min, max, val, unit = "", step = 1) => ({ t: "sl", k, en, zh, min, max, val, unit, step });
const tg = (k, en, zh, val = false) => ({ t: "tg", k, en, zh, val });
const se = (k, en, zh, opts, val = 0) => ({ t: "se", k, en, zh, opts, val });
const sg = (k, opts, val = 0) => ({ t: "sg", k, opts, val });
const sw = (en, zh, color) => ({ t: "sw", en, zh, color });
const nt = (en, zh) => ({ t: "nt", en, zh });
const bt = (en, zh) => ({ t: "bt", en, zh });
const gr = (en, zh, rows, opt = {}) => ({ en, zh, rows, ...opt });
const O = (...pairs) => pairs;

const val = (c) => {
	if (c.k.startsWith("a.")) return selected()?.[c.k.slice(2)] ?? c.val;
	return S.v[c.k] ?? c.val;
};
const shut = (g) => S.shut[g.en + g.zh] ?? Boolean(g.shut);

const row = (c) => {
	if (c.when !== undefined) return (S.v[c.when] ?? false) ? c.rows.map(row).join("") : "";
	switch (c.t) {
		case "sl": {
			const v = val(c);
			return `<div class="f sl"><label for="r-${c.k}" title="${pick(c.en, c.zh)}">${L(c.en, c.zh)}</label><div class="ctl"><input type="range" id="r-${c.k}" style="--p:${percent(v, c.min, c.max)}%" data-range="${c.k}" data-unit="${c.unit}" min="${c.min}" max="${c.max}" step="${c.step}" value="${v}"><output>${v}${c.unit}</output></div></div>`;
		}
		case "tg":
			return `<div class="f"><span>${L(c.en, c.zh)}</span><div class="ctl"><button class="switch ${val(c) ? "on" : ""}" data-act="tg" data-k="${c.k}" aria-label="${pick(c.en, c.zh)}"></button></div></div>`;
		case "se": {
			const o = c.opts[val(c)];
			return `<div class="f"><span>${L(c.en, c.zh)}</span><div class="ctl"><button class="pick" data-act="cycle" data-k="${c.k}" data-n="${c.opts.length}"><span>${L(o[0], o[1])}</span>${icon("chevron-down")}</button></div></div>`;
		}
		case "sg":
			return `<div class="seg-row">${c.opts.map((o, i) => `<button class="${val(c) === i ? "on" : ""}" data-act="set" data-k="${c.k}" data-v="${i}">${L(o[0], o[1])}</button>`).join("")}</div>`;
		case "sw":
			return `<div class="f"><span>${L(c.en, c.zh)}</span><div class="ctl"><span class="swatch"><i style="background:${c.color}"></i>${c.color}</span></div></div>`;
		case "nt":
			return `<p class="note">${L(c.en, c.zh)}</p>`;
		case "bt":
			return `<button class="line-btn" data-act="noop">${L(c.en, c.zh)}</button>`;
		case "colors":
			return `<div class="f"><span>${L(c.en, c.zh)}</span><div class="ctl"><div class="colors">${colorDots(c.k, c.clear)}</div></div></div>`;
		case "swatches": {
			const hues = ["#FFFFFF", "#1F3FD8", "#1E1F24", "#4B7F6B", "#E2725B", "#E0B04C", "#5B4B8A", "#3E7CB1", "#C98FA7", "#2B3A55", "#F2F1EC", "#264D47"];
			return `<div class="tiles four">${Array.from({ length: c.n }, (_, i) => {
				const a = hues[i % hues.length];
				const b = hues[(i + 4) % hues.length];
				const empty = c.clear && i === c.n - 1;
				return `<button class="tile ${i === (S.v.sw ?? 1) ? "on" : ""}" data-act="swatch" data-a="${empty ? "clear" : a}" data-b="${c.grad ? b : a}" data-i="${i}" style="min-height:34px;padding:3px"><i class="${empty ? "none" : ""}" style="background:${empty ? "repeating-conic-gradient(#d0d0d0 0 25%,#fff 0 50%) 0 0/8px 8px" : c.grad ? `linear-gradient(135deg,${a},${b})` : a}"></i></button>`;
			}).join("")}</div>`;
		}
		case "bgbody": {
			const keys = ["wallpaper", "image", "color", "gradient"];
			return body(bgBodies[keys[S.v.bgKind ?? 3]]);
		}
		default:
			return "";
	}
};
const body = (rows) => rows.map((r) => (r.rows && r.en ? group(r) : row(r))).join("");
const group = (g) =>
	`<div class="grp ${shut(g) ? "shut" : ""}"><button data-act="fold" data-g="${g.en + g.zh}" data-d="${g.shut ? 1 : 0}">${L(g.en, g.zh)}${icon("chevron-down")}</button>${
		shut(g) ? "" : `<div class="grp-body">${body(g.rows)}${g.sub ? group(g.sub) : ""}</div>`
	}</div>`;
const colorDots = (k, clear) => {
	const field = k.slice(2);
	const now = selected()?.[field];
	const dots = COLORS.map((c) => `<button class="${now === c ? "on" : ""}" style="background:${c}" data-act="anncolor" data-k="${field}" data-v="${c}" aria-label="${c}"></button>`).join("");
	const none = clear ? `<button class="none ${now === "transparent" ? "on" : ""}" data-act="anncolor" data-k="${field}" data-v="transparent" aria-label="${pick("Transparent", "透明")}"></button>` : "";
	return dots + none;
};

const bgBodies = {
	wallpaper: [sg("wtheme", O(["macOS", "macOS"], ["Dark", "深色"], ["Blue", "蓝色"], ["Cities", "城市"], ["Purple", "紫色"], ["Orange", "橙色"])), { t: "swatches", n: 12, grad: true }],
	image: [bt("Choose or drop an image", "选择或拖入图片"), nt("JPG, PNG, GIF, WebP, BMP", "JPG、PNG、GIF、WebP、BMP")],
	color: [sw("Color", "颜色", "#FFFFFF"), { t: "swatches", n: 12, clear: true }],
	gradient: [{ t: "swatches", n: 12, grad: true }, sw("From", "起始色", "#4785FF"), sw("To", "结束色", "#FF4766")],
};
const corners = O(["Squircle", "超椭圆"], ["Rounded", "圆角"]);
const bgGroups = () => [
	gr("Source", "来源", [sg("bgKind", O(["Wallpaper", "壁纸"], ["Image", "图片"], ["Color", "纯色"], ["Gradient", "渐变"]), 3), { t: "bgbody" }]),
	gr("Blur", "模糊", [sl("bgBlur", "Background blur", "背景模糊", 0, 100, 0, "%")]),
];
const padRows = [sl("padding", "Padding", "内边距", 0, 100, 40, " px")];
const roundRows = [sl("rounding", "Rounded corners", "圆角", 0, 100, 16, " px"), se("corner", "Corner style", "圆角样式", corners)];
const shadowGroup = (open) => gr("Shadow", "阴影", [sl("shadow", "Shadow", "阴影", 0, 100, 50, "%")], { shut: !open, sub: gr("Advanced shadow settings", "高级阴影设置", [sl("shS", "Size", "大小", 0, 100, 14), sl("shO", "Opacity", "不透明度", 0, 100, 68), sl("shB", "Blur", "模糊", 0, 100, 4)], { shut: true }) });
const borderGroup = (open) => gr("Border", "描边", [tg("border", "Show border", "显示描边"), { when: "border", rows: [sl("bW", "Width", "宽度", 1, 20, 2), sw("Color", "颜色", "#FFFFFF"), sl("bO", "Opacity", "不透明度", 0, 100, 50, "%")] }], { shut: !open });
const screenGroups = () => [gr("Layout", "布局", [...padRows, ...roundRows]), shadowGroup(true), borderGroup(true)];

const annRows = (a) => {
	const rows = [];
	if (a.t === "mask") {
		rows.push(sg("a.maskI", O(["Blur", "模糊"], ["Pixelate", "像素化"]), a.mask === "blur" ? 0 : 1), sl("a.level", "Intensity", "强度", 1, 32, a.level));
		return rows;
	}
	if (a.t === "spot") return [sl("a.dim", "Darkness", "暗度", 0, 100, a.dim, "%")];
	if (a.t === "cut") return [nt("The band closes up when you let go. Annotations move with the image, and undo brings it back.", "松手后这段会合拢，标注跟着图片移动，撤销可以恢复。")];
	if (a.t === "image") return [sl("a.opPct", "Opacity", "不透明度", 0, 100, Math.round((a.op ?? 1) * 100), "%"), sl("a.radius", "Rounded corners", "圆角", 0, 40, a.radius, " px"), bt("Replace image", "替换图片")];
	if (a.t === "magnify") return [sl("a.zoom", "Zoom", "放大", 1.5, 4, a.zoom, "×", 0.5), { t: "colors", k: "a.stroke", en: "Border", zh: "边框" }, sl("a.width", "Width", "宽度", 0, 12, a.width, " px")];
	rows.push({ t: "colors", k: "a.stroke", en: a.t === "text" || a.t === "counter" ? "Color" : a.t === "bubble" ? "Fill" : "Stroke", zh: a.t === "bubble" ? "填充" : "颜色" });
	if (["draw", "line", "arrow", "rect", "circle"].includes(a.t)) rows.push(sl("a.width", "Width", "宽度", 1, 40, a.width, " px"));
	if (a.t === "rect" || a.t === "circle") rows.push({ t: "colors", k: "a.fill", en: "Fill", zh: "填充", clear: true });
	rows.push(sl("a.opPct", "Opacity", "不透明度", 0, 100, Math.round((a.op ?? 1) * 100), "%"));
	if (a.t === "text" || a.t === "counter") rows.push(sl("a.h", "Size", "大小", 12, 120, a.h, " px"));
	if (a.t === "bubble") rows.push(sl("a.size", "Text size", "文字大小", 12, 60, a.size, " px"), nt("Drag the tail to point at something.", "拖动尾巴指向目标。"));
	return rows;
};
const kindName = (a) => {
	const t = toolOf(a.t);
	return L(t.en, t.zh);
};
const layerLabel = (a) => (a.t === "text" || a.t === "bubble" ? pick(a.en ?? a.zh, a.zh ?? a.en) : pick(toolOf(a.t).en, toolOf(a.t).zh));

const layerRows = () => {
	if (!S.anns.length) return `<div class="layer-empty"><b>${L("No layers yet", "还没有图层")}</b>${L("Use the tools to add annotations.", "用工具添加标注。")}</div>`;
	return `<div class="layer-list">${[...S.anns]
		.reverse()
		.map((a) => `<button class="layer ${S.sel === a.id ? "on" : ""}" data-act="pick" data-v="${a.id}">${icon(toolOf(a.t).icon)}<span>${layerLabel(a)}</span><span class="ib" data-act="del" data-v="${a.id}" style="flex:none;min-width:24px;height:24px" title="${pick("Delete", "删除")}">${icon("x")}</span></button>`)
		.join("")}</div><p class="layer-hint">${L("Drag to reorder. Top is in front.", "拖动排序，上面的在前。")}</p>`;
};

const SCENE = `
<rect width="1000" height="625" fill="#F7F7F5"/>
<rect width="190" height="625" fill="#ECEBE6"/>
<rect x="24" y="26" width="110" height="14" rx="3" fill="#2B3A55"/>
<rect x="24" y="72" width="130" height="10" rx="3" fill="#A9A79C"/><rect x="24" y="100" width="104" height="10" rx="3" fill="#A9A79C"/><rect x="24" y="128" width="118" height="10" rx="3" fill="#A9A79C"/>
<text x="220" y="62" font-family="Inter,sans-serif" font-size="26" font-weight="600" fill="#20242E">Monthly report</text>
<rect x="220" y="150" width="150" height="110" rx="8" fill="#FFFFFF"/><text x="236" y="186" font-family="Inter,sans-serif" font-size="13" fill="#6B6B66">Exports</text><text x="236" y="226" font-family="Inter,sans-serif" font-size="30" font-weight="600" fill="#20242E">1,284</text>
<rect x="386" y="150" width="130" height="110" rx="8" fill="#FFFFFF"/><text x="400" y="186" font-family="Inter,sans-serif" font-size="13" fill="#6B6B66">Errors</text><text x="400" y="226" font-family="Inter,sans-serif" font-size="30" font-weight="600" fill="#20242E">12</text>
<rect x="556" y="160" width="380" height="150" rx="8" fill="#FFFFFF"/><polyline fill="none" stroke="#3E7CB1" stroke-width="4" points="572,282 620,262 668,270 716,234 764,240 812,196 860,206 908,176"/>
<rect x="112" y="408" width="250" height="110" rx="6" fill="#FFFFFF"/>
<text x="124" y="442" font-family="Inter,monospace" font-size="16" fill="#20242E">demo@example.com</text><text x="124" y="472" font-family="Inter,monospace" font-size="16" fill="#20242E">token 0000-demo</text><text x="124" y="502" font-family="Inter,monospace" font-size="16" fill="#20242E">account 000000</text>
<rect x="400" y="408" width="548" height="24" rx="4" fill="#E9E8E3"/><rect x="400" y="446" width="500" height="24" rx="4" fill="#E9E8E3"/><rect x="400" y="484" width="530" height="24" rx="4" fill="#E9E8E3"/>`;
const sceneSvg = () => `<svg viewBox="0 0 1000 625" aria-hidden="true">${SCENE}</svg>`;

const pathOf = (a) => {
	const pts = a.pts.map(([px, py]) => [a.x + px * a.w, a.y + py * a.h]);
	return `M${pts.map((p) => p.map((n) => n.toFixed(1)).join(" ")).join(" L")}`;
};
const arrowHead = (a) => {
	const ex = a.x + a.w;
	const ey = a.y + a.h;
	const angle = Math.atan2(a.h, a.w);
	const size = Math.max(16, a.width * 4);
	const p = (d) => `${(ex - size * Math.cos(angle + d)).toFixed(1)},${(ey - size * Math.sin(angle + d)).toFixed(1)}`;
	return `${ex},${ey} ${p(0.45)} ${p(-0.45)}`;
};
const box = (a) => ({ x: Math.min(a.x, a.x + a.w), y: Math.min(a.y, a.y + a.h), w: Math.abs(a.w), h: Math.abs(a.h) });
const mosaic = (a, b) => {
	const size = Math.max(6, a.level * 2.4);
	const tones = ["#FFFFFF", "#ECECE8", "#C9C9C4", "#9A9A95", "#6F6F6B"];
	const cells = [];
	for (let i = 0; i * size < b.w; i += 1) {
		for (let j = 0; j * size < b.h; j += 1) {
			const seed = Math.abs(Math.sin(i * 12.9898 + j * 78.233) * 43758.5453) % 1;
			cells.push(`<rect x="${b.x + i * size}" y="${b.y + j * size}" width="${size + 0.5}" height="${size + 0.5}" fill="${tones[Math.floor(seed * tones.length)]}"/>`);
		}
	}
	return cells.join("");
};
const annSvg = (a) => {
	const hit = `data-ann="${a.id}"`;
	const op = `opacity="${a.op ?? 1}"`;
	const b = box(a);
	switch (a.t) {
		case "rect":
			return `<rect ${hit} ${op} x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}" fill="${a.fill === "transparent" ? "transparent" : a.fill}" stroke="${a.stroke}" stroke-width="${a.width}" rx="4"/>`;
		case "circle":
			return `<ellipse ${hit} ${op} cx="${b.x + b.w / 2}" cy="${b.y + b.h / 2}" rx="${b.w / 2}" ry="${b.h / 2}" fill="${a.fill === "transparent" ? "transparent" : a.fill}" stroke="${a.stroke}" stroke-width="${a.width}"/>`;
		case "spot":
			return `<path pointer-events="none" fill-rule="evenodd" fill="rgb(0 0 0 / ${(a.dim ?? 55) / 100})" d="M-400 -400H1400V1025H-400Z M${b.x} ${b.y}h${b.w}v${b.h}h${-b.w}Z"/><rect ${hit} x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}" fill="transparent"/>`;
		case "magnify": {
			const cx = b.x + b.w / 2;
			const cy = b.y + b.h / 2;
			return `<g ${hit}><rect x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}" fill="#fff"/><g clip-path="url(#clip-${a.id})"><g transform="translate(${cx} ${cy}) scale(${a.zoom}) translate(${-cx} ${-cy})">${SCENE}</g></g><rect x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}" fill="none" stroke="${a.stroke}" stroke-width="${a.width}"/></g>`;
		}
		case "bubble": {
			const tx = b.x + b.w * 0.25;
			const by = b.y + b.h;
			return `<g ${hit} ${op}><rect x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}" rx="10" fill="${a.stroke}"/><polygon points="${tx},${by} ${tx + 26},${by} ${tx - 8},${by + 34}" fill="${a.stroke}"/><text x="${b.x + b.w / 2}" y="${b.y + b.h / 2 + a.size * 0.36}" text-anchor="middle" font-family="Inter,'HarmonyOS Sans SC',sans-serif" font-size="${a.size}" font-weight="500" fill="#fff">${layerLabel(a)}</text></g>`;
		}
		case "image":
			return `<g ${hit} ${op}><rect x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}" rx="${a.radius}" fill="#20242E"/><text x="${b.x + b.w / 2}" y="${b.y + b.h / 2 + 7}" text-anchor="middle" font-family="Inter,sans-serif" font-size="20" font-weight="600" fill="#fff">Logo</text></g>`;
		case "cut": {
			const wide = b.w >= b.h;
			const area = wide ? `x="${b.x}" y="0" width="${b.w}" height="625"` : `x="0" y="${b.y}" width="1000" height="${b.h}"`;
			return `<rect ${hit} ${area} fill="url(#fx-cut)" stroke="var(--ring)" stroke-width="1.5" stroke-dasharray="6 4"/>`;
		}
		case "marker":
			return `<rect ${hit} opacity="${0.4 * (a.op ?? 1)}" x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}" fill="${a.stroke}"/>`;
		case "line":
			return `<g ${hit} ${op}><line x1="${a.x}" y1="${a.y}" x2="${a.x + a.w}" y2="${a.y + a.h}" stroke="transparent" stroke-width="20"/><line x1="${a.x}" y1="${a.y}" x2="${a.x + a.w}" y2="${a.y + a.h}" stroke="${a.stroke}" stroke-width="${a.width}" stroke-linecap="round"/></g>`;
		case "arrow":
			return `<g ${hit} ${op}><line x1="${a.x}" y1="${a.y}" x2="${a.x + a.w}" y2="${a.y + a.h}" stroke="transparent" stroke-width="20"/><line x1="${a.x}" y1="${a.y}" x2="${a.x + a.w}" y2="${a.y + a.h}" stroke="${a.stroke}" stroke-width="${a.width}" stroke-linecap="round"/><polygon points="${arrowHead(a)}" fill="${a.stroke}"/></g>`;
		case "draw":
			return `<path ${hit} ${op} d="${pathOf(a)}" fill="none" stroke="${a.stroke}" stroke-width="${a.width}" stroke-linecap="round" stroke-linejoin="round"/>`;
		case "text":
			return `<text ${hit} ${op} x="${a.x}" y="${a.y + a.h}" font-family="Inter,'HarmonyOS Sans SC',sans-serif" font-size="${a.h}" font-weight="500" fill="${a.stroke}">${layerLabel(a)}</text>`;
		case "mask":
			return `<g ${hit}><rect x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}" fill="#F2F1EC"/><g clip-path="url(#clip-${a.id})">${a.mask === "blur" ? `<g filter="url(#fx-blur-${a.id})">${SCENE}</g>` : mosaic(a, b)}</g></g>`;
		case "counter":
			return `<g ${hit} ${op}><circle cx="${a.x}" cy="${a.y}" r="${a.h * 0.7}" fill="${a.stroke}"/><text x="${a.x}" y="${a.y + a.h * 0.32}" text-anchor="middle" font-family="Inter,sans-serif" font-size="${a.h * 0.9}" font-weight="600" fill="#fff">${a.n}</text></g>`;
		default:
			return "";
	}
};
const defsSvg = () =>
	`<defs><pattern id="fx-cut" width="12" height="12" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="12" height="12" fill="rgb(0 0 0 / 0.25)"/><rect width="4" height="12" fill="rgb(255 255 255 / 0.6)"/></pattern>${S.anns
		.filter((a) => a.t === "mask" || a.t === "magnify")
		.map((a) => {
			const b = box(a);
			return `<clipPath id="clip-${a.id}"><rect x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}"/></clipPath>
			<filter id="fx-blur-${a.id}" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="${(a.level ?? 7) * 0.9}"/></filter>`;
		})
		.join("")}</defs>`;
const handlesSvg = () => {
	const a = selected();
	if (a?.t === "cut") return "";
	if (!a || a.t === "text" || a.t === "counter") {
		if (!a) return "";
		const w = a.t === "text" ? layerLabel(a).length * a.h * 0.72 : a.h * 1.4;
		const x = a.t === "text" ? a.x : a.x - a.h * 0.7;
		const y = a.t === "text" ? a.y : a.y - a.h * 0.7;
		return `<rect x="${x - 4}" y="${y - 4}" width="${w + 8}" height="${(a.t === "text" ? a.h * 1.25 : a.h * 1.4) + 8}" fill="none" stroke="var(--ring)" stroke-width="1.5" pointer-events="none"/>`;
	}
	const dot = (x, y) => `<circle cx="${x}" cy="${y}" r="6" fill="#fff" stroke="var(--ring)" stroke-width="2" pointer-events="none"/>`;
	if (a.t === "arrow" || a.t === "line") return dot(a.x, a.y) + dot(a.x + a.w, a.y + a.h);
	const b = box(a);
	const xs = [b.x, b.x + b.w / 2, b.x + b.w];
	const ys = [b.y, b.y + b.h / 2, b.y + b.h];
	const dots = [];
	xs.forEach((x, i) => {
		ys.forEach((y, j) => {
			if (i !== 1 || j !== 1) dots.push(dot(x, y));
		});
	});
	return `<rect x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}" fill="none" stroke="var(--ring)" stroke-width="1.5" pointer-events="none"/>${dots.join("")}`;
};
const annLayer = () => `${defsSvg()}${S.anns.map(annSvg).join("")}${handlesSvg()}`;
const paintAnns = () => {
	const svg = document.getElementById("overlay");
	if (svg) svg.innerHTML = annLayer();
};

const titleActions = () => `<button class="ib save-btn" data-act="save" title="Ctrl+S">${icon("download")}${L("Save", "保存")}</button>
	<button class="export-btn copy-btn" data-act="copy" title="Ctrl+C">${L("Copy", "复制")}</button>`;
const captions = () => `<span class="caption">${glyphs.minimize}</span><span class="caption">${glyphs.maximize}</span><span class="caption">${glyphs.close}</span>`;
const history = () => `<button class="ib" data-act="undo" title="${pick("Undo", "撤销")} Ctrl+Z" ${S.past.length ? "" : "disabled"}>${icon("undo-2")}</button>
	<button class="ib" data-act="redo" title="${pick("Redo", "重做")} Ctrl+Y" ${S.future.length ? "" : "disabled"}>${icon("redo-2")}</button>`;
const nameBlock = () => `<span class="name">Monthly report<em>.ever</em></span>
	<button class="ib" data-act="noop" title="${pick("Show in Explorer", "在资源管理器中显示")}">${icon("folder")}</button>
	<button class="ib" data-act="dialog" data-d="delete" title="${pick("Delete screenshot", "删除截图")}">${icon("trash")}</button>`;
const titlebar = () => `<div class="titlebar">${nameBlock()}<span class="grow"></span>${history()}<span class="sep"></span>${titleActions()}${captions()}</div>`;

const aspects = [["Auto", "自动"], ["Wide 16:9", "宽屏 16:9"], ["Vertical 9:16", "竖屏 9:16"], ["Square 1:1", "方形 1:1"], ["Classic 4:3", "经典 4:3"], ["Tall 3:4", "高屏 3:4"]];
const aspectValue = [null, 16 / 9, 9 / 16, 1, 4 / 3, 3 / 4];
const aspectBtn = () => `<button class="ib ${S.menu === "aspect" ? "on" : ""}" data-act="menu" data-m="aspect" title="${pick("Aspect ratio", "宽高比")}">${icon("proportions")}${L(...aspects[S.aspect])}${icon("chevron-down")}</button>`;
const cropBtn = () => `<button class="ib" data-act="dialog" data-d="crop" title="${pick("Crop image", "裁剪图片")}">${icon("crop")}${L("Crop", "裁剪")}</button>`;
const toolBtn = (t, label = "") => `<button class="ib ${S.tool === t.id ? "on" : ""}" data-act="tool" data-v="${t.id}" title="${pick(t.en, t.zh)} ${t.k}" aria-label="${pick(t.en, t.zh)}">${icon(t.icon)}${label}</button>`;
const cutBtn = () => toolBtn(toolOf("cut"), L("Cut out", "切割"));
const toolBtns = () =>
	`<span class="tools">${[0, 1, 2, 3, 4]
		.map((g) => TOOLS.filter((t) => t.g === g).map((t) => toolBtn(t)).join(""))
		.join('<span class="sep"></span>')}</span>`;
const zoomCtl = () => `<button class="ib" data-act="zoom" data-v="-10" title="${pick("Zoom out", "缩小")} Ctrl −">${icon("zoom-out")}</button>
	<input class="zoomer" type="range" min="10" max="300" step="10" value="${S.zoom}" style="--p:${percent(S.zoom, 10, 300)}%" data-range="__zoom" aria-label="${pick("Zoom", "缩放")}">
	<button class="ib" data-act="zoom" data-v="10" title="${pick("Zoom in", "放大")} Ctrl +">${icon("zoom-in")}</button>
	<span class="pct" id="pct">${S.zoom}%</span>`;
const stage = () => `<div class="ss-stage" id="ss-stage"><div class="frame" id="frame"><div class="shot" id="shot"><div class="pic">${sceneSvg()}</div><svg class="overlay ${S.tool === "select" ? "" : "drawing"}" id="overlay" viewBox="0 0 1000 625" aria-label="${pick("Annotations", "标注")}">${annLayer()}</svg></div></div></div>`;
const outputSize = () => {
	const pad = (S.v.padding ?? 40) * 2;
	const ratio = aspectValue[S.aspect];
	const w = OUT_W + pad;
	const h = OUT_H + pad;
	if (!ratio) return `${w} × ${h}`;
	return w / h > ratio ? `${w} × ${Math.round(w / ratio)}` : `${Math.round(h * ratio)} × ${h}`;
};

const panelHead = (titleHtml, closeAct) => `<div class="panel-head"><b>${titleHtml}</b>${closeAct ?? ""}</div>`;
const selHead = (a) => panelHead(kindName(a), `<button class="ib" data-act="del" data-v="${a.id}" title="${pick("Delete", "删除")} Del">${icon("trash")}</button><button class="ib" data-act="deselect">${L("Done", "完成")}</button>`);
const RAIL = [
	["background", "image", "Background", "背景"],
	["screen", "monitor", "Screen", "画面"],
	["layers", "layers", "Layers", "图层"],
];
const railA = () => `<div class="rail left icons">${RAIL.map(([id, ic, en, zh]) => `<button class="${!selected() && S.tab === id ? "on" : ""}" data-act="tab" data-v="${id}" title="${pick(en, zh)}" aria-label="${pick(en, zh)}">${icon(ic)}</button>`).join("")}</div>`;
const panelA = () => {
	const a = selected();
	if (a) return `<div class="panel">${selHead(a)}<div class="panel-body" id="panel-body"><div class="grp-body" style="padding-top:10px">${body(annRows(a))}</div></div></div>`;
	const [, , en, zh] = RAIL.find((r) => r[0] === S.tab);
	const inner = S.tab === "layers" ? layerRows() : (S.tab === "background" ? bgGroups() : screenGroups()).map(group).join("");
	return `<div class="panel">${panelHead(L(en, zh))}<div class="panel-body" id="panel-body">${inner}</div></div>`;
};
const layout = () => `${titlebar()}<div class="ed-body"><div class="col"><div class="row-top">
	<div class="player">
		<div class="toolbar center">${aspectBtn()}${cropBtn()}${cutBtn()}<span class="grow"></span>${toolBtns()}<span class="grow"></span></div>
		${stage()}
		<div class="status">${zoomCtl()}<span class="grow"></span><span>${L("Output", "输出")} ${outputSize()}</span></div>
	</div>
	${railA()}${panelA()}
</div></div></div>`;

const menus = {
	aspect: () => `<div class="pop menu" style="left:8px;top:84px;width:200px">${aspects.map((a, i) => `<button data-act="aspect" data-v="${i}"><span class="grow">${L(a[0], a[1])}</span>${S.aspect === i ? icon("check") : ""}</button>`).join("")}</div>`,
};
const ratios = ["Free", "1:1", "2:1", "3:2", "4:3", "9:16", "16:9", "16:10", "21:9"];
const dialogs = {
	delete: () => `<div class="scrim"><div class="dialog"><h3>${L("Delete screenshot?", "删除截图？")}</h3><p>${L("It goes to the Recycle Bin.", "它会进入回收站。")}</p><div class="actions"><button class="btn" data-act="close">${L("Keep", "保留")}</button><button class="btn danger" data-act="close">${L("Delete screenshot", "删除截图")}</button></div></div></div>`,
	crop: () => `<div class="scrim"><div class="dialog" style="width:560px;max-width:calc(100% - 24px)"><h3>${L("Crop image", "裁剪图片")}</h3><div class="crop-box">${sceneSvg()}<i></i></div>
		<div class="crop-fields">${[["W", "宽", 1123], ["H", "高", 684], ["X", "X", 144], ["Y", "Y", 90]].map(([en, zh, v]) => `<label>${L(en, zh)}<span class="input"><input value="${v}" aria-label="${en}"></span></label>`).join("")}</div>
		<div class="crop-row">${ratios.map((r, i) => `<span class="tag ${i === (S.v.ratio ?? 0) ? "on" : ""}" data-act="set" data-k="ratio" data-v="${i}">${r === "Free" ? L("Free", "自由") : r}</span>`).join("")}</div>
		<div class="crop-row" style="margin-top:8px"><button class="ib ${S.v.snap ? "on" : ""}" data-act="tg" data-k="snap">${L("Snap to ratios", "吸附比例")}</button><button class="ib" data-act="noop">${L("Full", "全图")}</button><button class="ib" data-act="noop">${L("Reset", "重置")}</button><span class="sp"></span></div>
		<div class="actions"><button class="btn" data-act="close">${L("Cancel", "取消")}</button><button class="btn primary" data-act="close">${L("Save", "保存")}</button></div></div></div>`,
};

const display = () => S.display ?? (window.innerWidth < 760 ? "stacked" : "fit");
const renderViewRow = () => {
	document.getElementById("view-row").innerHTML = `
		<div class="seg">${[["fit", "Whole window", "整窗"], ["stacked", "Phone detail", "手机细看"]].map(([v, en, zh]) => `<button class="${display() === v ? "on" : ""}" data-pact="display" data-v="${v}">${L(en, zh)}</button>`).join("")}</div>
		<span class="hint">${L(`${S.anns.length} layers · tool ${toolOf(S.tool).en} · ${S.sel ? "selected" : "nothing selected"}`, `${S.anns.length} 个图层 · 工具：${toolOf(S.tool).zh} · ${S.sel ? "已选中" : "未选中"}`)}</span>`;
};
const sizeFrame = () => {
	const stageEl = document.getElementById("ss-stage");
	const frame = document.getElementById("frame");
	const shot = document.getElementById("shot");
	if (!stageEl || !frame) return;
	const style = getComputedStyle(stageEl);
	const availW = stageEl.clientWidth - Number.parseFloat(style.paddingLeft) * 2;
	const availH = display() === "stacked" ? 210 : stageEl.clientHeight - Number.parseFloat(style.paddingTop) * 2;
	const pad = (S.v.padding ?? 40) * 2.2;
	const ratio = aspectValue[S.aspect] ?? (1000 + pad * 2) / (625 + pad * 2);
	const w = Math.min(availW, availH * ratio);
	const h = w / ratio;
	frame.style.width = `${w}px`;
	frame.style.height = `${h}px`;
	frame.style.transform = `scale(${S.zoom / 100})`;
	const unit = Math.min(w / (1000 + pad * 2), h / (625 + pad * 2));
	const iw = 1000 * unit;
	const ih = 625 * unit;
	shot.style.left = `${(w - iw) / 2}px`;
	shot.style.top = `${(h - ih) / 2}px`;
	shot.style.width = `${iw}px`;
	shot.style.height = `${ih}px`;
	const pic = shot.querySelector(".pic");
	const round = (S.v.rounding ?? 16) * unit * 1.4;
	pic.style.borderRadius = `${round}px`;
	const blur = (S.v.shadow ?? 50) * unit * 0.9;
	shot.style.borderRadius = `${round}px`;
	shot.style.boxShadow = `0 ${blur * 0.3}px ${blur}px rgb(0 0 0 / ${(S.v.shadow ?? 50) / 170})`;
	shot.style.outline = S.v.border ? `${(S.v.bW ?? 2) * unit * 1.4}px solid rgb(255 255 255 / ${(S.v.bO ?? 50) / 100})` : "none";
	const clear = S.v.c1 === "clear";
	frame.classList.toggle("clear", clear);
	const solid = (S.v.bgKind ?? 3) === 2;
	frame.style.background = clear ? "" : solid ? (S.v.c1 ?? "#1F3FD8") : `linear-gradient(135deg,${S.v.c1 ?? "#1F3FD8"},${S.v.c2 ?? "#C98FA7"})`;
};
const fit = () => {
	const wrap = document.getElementById("ed-fit");
	const ed = document.getElementById("ed");
	const stageEl = document.getElementById("stage");
	if (!wrap || !ed) return;
	if (display() === "stacked") return;
	const scale = Math.min(1, stageEl.clientWidth / 1300);
	ed.style.transform = `scale(${scale})`;
	ed.style.transformOrigin = "top left";
	wrap.style.width = `${1275 * scale}px`;
	wrap.style.height = `${800 * scale}px`;
};
const render = () => {
	const prior = document.getElementById("panel-body")?.scrollTop ?? 0;
	const stacked = display() === "stacked";
	document.getElementById("stage").innerHTML = `<div class="ed-fit" id="ed-fit"><div class="ed ${stacked ? "stacked" : ""}" id="ed">
		${layout()}
		${S.menu ? menus[S.menu]() : ""}
		${S.dialog ? dialogs[S.dialog]() : ""}
	</div></div>`;
	const panelNode = document.getElementById("panel-body");
	if (panelNode) panelNode.scrollTop = prior;
	renderViewRow();
	fit();
	sizeFrame();
};

const toast = (en, zh) => {
	const el = document.getElementById("toast");
	el.innerHTML = L(en, zh);
	el.hidden = false;
	clearTimeout(toast.t);
	toast.t = setTimeout(() => {
		el.hidden = true;
	}, 1600);
};
const setField = (key, value) => {
	const a = selected();
	if (!a) return;
	const field = key.slice(2);
	if (field === "opPct") a.op = value / 100;
	else if (field === "maskI") a.mask = value === 0 ? "blur" : "pixelate";
	else a[field] = value;
};
const undo = () => {
	if (!S.past.length) return;
	S.future.push(JSON.stringify(S.anns));
	S.anns = JSON.parse(S.past.pop());
	if (!selected()) S.sel = null;
};
const redo = () => {
	if (!S.future.length) return;
	S.past.push(JSON.stringify(S.anns));
	S.anns = JSON.parse(S.future.pop());
};
const del = (id) => {
	remember();
	S.anns = S.anns.filter((a) => a.id !== id);
	if (S.sel === id) S.sel = null;
};
const act = (el) => {
	const d = el.dataset;
	switch (d.act) {
		case "tab":
			S.tab = d.v;
			S.sel = null;
			break;
		case "tool":
			S.tool = d.v;
			if (d.v !== "select") S.sel = null;
			break;
		case "pick":
			S.sel = Number(d.v);
			S.tool = "select";
			break;
		case "del":
			del(Number(d.v));
			break;
		case "deselect":
			S.sel = null;
			S.menu = null;
			break;
		case "tg":
			S.v[d.k] = !(S.v[d.k] ?? false);
			break;
		case "cycle":
			S.v[d.k] = ((S.v[d.k] ?? 0) + 1) % Number(d.n);
			break;
		case "set":
			if (d.k.startsWith("a.")) {
				remember();
				setField(d.k, Number(d.v));
			} else S.v[d.k] = Number(d.v);
			break;
		case "swatch":
			S.v.c1 = d.a;
			S.v.c2 = d.b;
			S.v.sw = Number(d.i);
			break;
		case "anncolor": {
			remember();
			const a = selected();
			if (a) a[d.k] = d.v;
			S.menu = null;
			break;
		}
		case "fold": {
			const cur = S.shut[d.g] ?? d.d === "1";
			S.shut[d.g] = !cur;
			break;
		}
		case "menu":
			S.menu = S.menu === d.m ? null : d.m;
			break;
		case "aspect":
			S.aspect = Number(d.v);
			S.menu = null;
			break;
		case "close":
			S.menu = null;
			S.dialog = null;
			break;
		case "dialog":
			S.dialog = d.d;
			S.menu = null;
			break;
		case "zoom":
			S.zoom = Math.min(300, Math.max(10, S.zoom + Number(d.v)));
			break;
		case "undo":
			undo();
			break;
		case "redo":
			redo();
			break;
		case "copy":
			toast("Copied to clipboard", "已复制到剪贴板");
			return;
		case "save":
			toast("Saved", "已保存");
			return;
		default:
			return;
	}
	render();
};
const pact = (el) => {
	const d = el.dataset;
	if (d.pact === "display") S.display = d.v;
	render();
};

const point = (event) => {
	const svg = document.getElementById("overlay");
	const m = svg.getScreenCTM().inverse();
	const p = new DOMPoint(event.clientX, event.clientY).matrixTransform(m);
	return [p.x, p.y];
};
const create = (tool, x, y) => {
	const a = { id: S.next++, t: tool, x, y, w: 0, h: 0, op: 1 };
	if (["rect", "circle", "line", "arrow", "draw"].includes(tool)) Object.assign(a, { stroke: "#F05656", width: 4, fill: "transparent" });
	if (tool === "draw") a.raw = [[x, y]];
	if (tool === "mask") Object.assign(a, { mask: "pixelate", level: 7 });
	if (tool === "marker") a.stroke = "#F5A524";
	if (tool === "spot") a.dim = 55;
	if (tool === "magnify") Object.assign(a, { zoom: 2, stroke: "#FFFFFF", width: 3 });
	if (tool === "image") a.radius = 8;
	if (tool === "bubble") Object.assign(a, { stroke: "#F05656", size: 22, en: "Note", zh: "说明" });
	if (tool === "text") Object.assign(a, { h: 34, stroke: "#F05656", en: "Text", zh: "文字" });
	if (tool === "counter") Object.assign(a, { h: 30, stroke: "#F05656", n: S.anns.filter((n) => n.t === "counter").length + 1 });
	return a;
};
document.addEventListener("pointerdown", (event) => {
	const svg = event.target.closest?.("#overlay");
	if (!svg) return;
	event.preventDefault();
	const [x, y] = point(event);
	if (S.tool !== "select") {
		remember();
		const a = create(S.tool, x, y);
		S.anns.push(a);
		S.gesture = { kind: "create", id: a.id };
		paintAnns();
		return;
	}
	const hit = event.target.closest("[data-ann]");
	if (hit) {
		const id = Number(hit.dataset.ann);
		remember();
		S.sel = id;
		const a = selected();
		S.gesture = { kind: "move", id, dx: x - a.x, dy: y - a.y };
		S.menu = null;
		render();
		return;
	}
	S.sel = null;
	S.menu = null;
	render();
});
document.addEventListener("pointermove", (event) => {
	const g = S.gesture;
	if (!g) return;
	const a = S.anns.find((n) => n.id === g.id);
	if (!a) return;
	const [x, y] = point(event);
	if (g.kind === "move") {
		a.x = x - g.dx;
		a.y = y - g.dy;
	} else if (a.t === "draw") {
		a.raw.push([x, y]);
		const xs = a.raw.map((p) => p[0]);
		const ys = a.raw.map((p) => p[1]);
		a.x = Math.min(...xs);
		a.y = Math.min(...ys);
		a.w = Math.max(1, Math.max(...xs) - a.x);
		a.h = Math.max(1, Math.max(...ys) - a.y);
		a.pts = a.raw.map(([px, py]) => [(px - a.x) / a.w, (py - a.y) / a.h]);
	} else if (a.t !== "text" && a.t !== "counter") {
		a.w = x - a.x;
		a.h = y - a.y;
	}
	paintAnns();
});
document.addEventListener("pointerup", () => {
	const g = S.gesture;
	if (!g) return;
	S.gesture = null;
	if (g.kind === "create") {
		const a = S.anns.find((n) => n.id === g.id);
		if (a?.t === "draw") delete a.raw;
		if (a && ["rect", "circle", "mask", "marker", "spot", "magnify", "image", "bubble", "cut"].includes(a.t) && Math.abs(a.w) < 5 && Math.abs(a.h) < 5) {
			Object.assign(a, { x: a.x - 60, y: a.y - 40, w: 120, h: 80 });
		}
		if (a?.t === "draw" && !a.pts) {
			S.anns = S.anns.filter((n) => n !== a);
			S.tool = "select";
			render();
			return;
		}
		if ((a?.t === "arrow" || a?.t === "line") && Math.abs(a.w) < 5 && Math.abs(a.h) < 5) Object.assign(a, { w: 90, h: -60 });
		S.tool = "select";
		S.sel = g.id;
	}
	render();
});

document.addEventListener("click", (event) => {
	const p = event.target.closest("[data-pact]");
	if (p) return pact(p);
	const el = event.target.closest("[data-act]");
	if (el) {
		event.stopPropagation();
		return act(el);
	}
	if (S.menu && !event.target.closest(".pop")) {
		S.menu = null;
		render();
	}
});
document.addEventListener("input", (event) => {
	const el = event.target;
	const key = el.dataset?.range;
	if (!key) return;
	const value = Number(el.value);
	if (key === "__zoom") {
		S.zoom = value;
		el.style.setProperty("--p", `${percent(value, 10, 300)}%`);
		document.getElementById("pct").textContent = `${value}%`;
		sizeFrame();
		return;
	}
	el.style.setProperty("--p", `${percent(el.value, el.min, el.max)}%`);
	el.nextElementSibling.textContent = `${value}${el.dataset.unit}`;
	if (key.startsWith("a.")) {
		setField(key, value);
		paintAnns();
		return;
	}
	S.v[key] = value;
	sizeFrame();
});
document.addEventListener("keydown", (event) => {
	if (event.target.closest?.("input,textarea,[contenteditable]")) return;
	const k = event.key.toLowerCase();
	const ctrl = event.ctrlKey || event.metaKey;
	if (ctrl) {
		if (k === "z") event.shiftKey ? redo() : undo();
		else if (k === "y") redo();
		else if (k === "c") toast("Copied to clipboard", "已复制到剪贴板");
		else if (k === "s") toast("Saved", "已保存");
		else if (k === "-") S.zoom = Math.max(10, S.zoom - 10);
		else if (k === "=" || k === "+") S.zoom = Math.min(300, S.zoom + 10);
		else return;
		event.preventDefault();
		render();
		return;
	}
	if (k === "escape") {
		if (S.dialog || S.menu) {
			S.dialog = null;
			S.menu = null;
		} else {
			S.tool = "select";
			S.sel = null;
		}
		render();
		return;
	}
	if (k === "delete" || k === "backspace") {
		if (S.sel) del(S.sel);
		render();
		return;
	}
	const tool = TOOLS.find((t) => t.k.toLowerCase() === k);
	if (tool) {
		S.tool = tool.id;
		if (tool.id !== "select") S.sel = null;
		render();
		return;
	}
	if (k === "o") {
		S.tool = "circle";
		S.sel = null;
		render();
		return;
	}
	if (k === "s") {
		S.tool = "select";
		S.sel = null;
		render();
		return;
	}
	if (POPS[k]) {
		S.tab = POPS[k];
		S.sel = null;
		render();
		return;
	}
	if (k === "l") {
		S.tab = "layers";
		S.sel = null;
		render();
	}
});
window.addEventListener("resize", () => {
	fit();
	sizeFrame();
});

const table = (id, cls, rows) => {
	document.getElementById(id).innerHTML = `<table class="${cls}"><tbody>${rows.map((r) => `<tr>${r.map((c, i) => `<td class="${i ? "" : "dim"}">${Array.isArray(c) ? L(c[0], c[1]) : c}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
};
const renderDocs = () => {
	const heads = {
		"t-live": ["Screenshot editor", "截图编辑器"],
		"l-live": ["The chosen layout: the same title bar, rail and 340 px inspector as the editor, with tools above the preview. Tap an annotation to select it, or pick a tool and drag on the screenshot. Keyboard shortcuts work on a PC.", "选定的布局：标题栏、图标栏和 340 px 检查器与编辑器相同，工具在预览上方。点标注可以选中，选工具后在截图上拖动可以绘制。电脑上快捷键可用。"],
		"t-layout": ["Layout", "布局"],
		"l-layout": ["Sizes at the default 1275 × 800 window.", "默认 1275 × 800 窗口下的尺寸。"],
		"t-ann": ["Annotations", "标注"],
		"l-ann": ["The inspector shows only what the selected type uses. A new annotation selects itself and the tool returns to Select.", "检查器只显示所选类型用得到的选项。新标注创建后自动选中，工具回到“选择”。"],
		"t-states": ["Other states", "其他状态"],
		"l-states": ["What the window shows outside normal editing.", "正常编辑之外，窗口显示什么。"],
		"t-inv": ["Where everything went", "每项功能的位置"],
		"l-inv": ["Every capability in screenshot_editor.rs, screenshot_annotations.rs, screenshot_crop.rs and screenshot_export.rs, checked against upstream on 2026-10-31. Implementation tickets check against this list.", "screenshot_editor.rs、screenshot_annotations.rs、screenshot_crop.rs 和 screenshot_export.rs 的全部功能，已在 2026-10-31 与上游核对。实现工单按这张表核对。"],
	};
	for (const [id, text] of Object.entries(heads)) document.getElementById(id).innerHTML = L(text[0], text[1]);
	table("layout-table", "doc", [
		[["Title bar", "标题栏"], ["44 high: name, show in Explorer, delete, undo, redo, Save, Copy, window controls", "44 高：名称、在资源管理器中显示、删除、撤销、重做、保存、复制、窗口按钮"]],
		[["Preview toolbar", "预览工具栏"], ["40 high: aspect ratio, crop and cut out on the left, then 14 tools in five groups: select; drawing; text; cover and focus; image", "40 高：左侧宽高比、裁剪和切割，然后是分五组的 14 个工具：选择；绘制；文字；遮盖和聚焦；图片"]],
		[["Preview", "预览"], ["Flexible on the preview token, frame fitted inside 20 px of padding", "弹性，使用 preview token，画面在 20 px 边距内适应"]],
		[["Status bar", "状态栏"], ["36 high: zoom on the left, output size on the right", "36 高：左侧缩放，右侧输出尺寸"]],
		[["Inspector", "检查器"], ["46 icon rail plus a 340 panel: Background, Screen, Layers", "46 宽图标栏加 340 宽面板：背景、画面、图层"]],
		[["Primary action", "主要操作"], ["Copy is the one Klein blue button, because a screenshot most often goes to the clipboard", "复制是唯一的克莱因蓝按钮，因为截图最常去剪贴板"]],
		[["Window", "窗口"], ["One separate window per screenshot, opened from the library, tray or after a capture", "每张截图一个独立窗口，从资源库、托盘或截图后打开"]],
	]);
	table("ann-table", "doc", [
		[["Arrow, rectangle, circle, draw", "箭头、矩形、椭圆、画笔"], ["Color, width, opacity; rectangle and circle add fill with transparent", "颜色、宽度、不透明度；矩形和椭圆多一个可透明的填充"]],
		[["Line (new)", "直线（新增）"], ["I. An arrow without a head: color, width, opacity", "I。没有箭头的箭头线：颜色、宽度、不透明度"]],
		[["Highlighter (new)", "荧光笔（新增）"], ["G. A translucent box over content: color, opacity", "G。盖在内容上的半透明色块：颜色、不透明度"]],
		[["Text", "文字"], ["Color, opacity, size; double-click edits on the canvas", "颜色、不透明度、大小；双击在画布上编辑"]],
		[["Mask", "遮罩"], ["Blur or pixelate, intensity; stays inside the image", "模糊或像素化、强度；限制在图片内"]],
		[["Step number (new)", "序号（新增）"], ["N. Click to place a filled circle with the next unused number; color, opacity, size", "N。点击放置带下一个可用数字的实心圆；颜色、不透明度、大小"]],
		[["Speech bubble (new)", "对话气泡（新增）"], ["U. Filled box with a tail you drag: fill, opacity, text size; double-click edits the text", "U。带可拖动尾巴的色块：填充、不透明度、文字大小；双击改文字"]],
		[["Spotlight (new)", "聚光灯（新增）"], ["F. Darkens everything outside the box: darkness", "F。压暗选区以外的内容：暗度"]],
		[["Magnifier (new)", "放大镜（新增）"], ["Z. Enlarges what is under the box: zoom, border color, width", "Z。放大选区内的内容：放大倍数、边框颜色、宽度"]],
		[["Image (new)", "图片（新增）"], ["K, drop a file, or Ctrl+V with an image on the clipboard: opacity, rounded corners, replace. The file is copied into the project", "K、拖入文件，或剪贴板有图片时按 Ctrl+V：不透明度、圆角、替换。文件复制进项目"]],
		[["Cut out (new)", "切割（新增）"], ["X, in the toolbar next to Crop. Drag across to remove a vertical or horizontal band; the image closes up, annotations move with it and undo restores it. The original stays in the project", "X，在工具栏的裁剪旁边。横向或纵向拖过一段将其去掉；图片合拢，标注跟着移动，可以撤销。原图保留在项目里"]],
		[["Selection", "选中"], ["Ring-colored outline with eight handles; arrows show two end points; Shift constrains", "ring 颜色描边加八个手柄；箭头显示两个端点；Shift 约束"]],
		[["Shortcuts", "快捷键"], ["V or S select, D draw, G highlighter, I line, A arrow, R rectangle, C or O circle, T text, U speech bubble, N step number, M mask, F spotlight, Z magnifier, K image, X cut out, B background, P padding, H shadow, E border, L layers, Del delete, Esc deselect", "V 或 S 选择，D 画笔，G 荧光笔，I 直线，A 箭头，R 矩形，C 或 O 椭圆，T 文字，U 对话气泡，N 序号，M 遮罩，F 聚光灯，Z 放大镜，K 图片，X 切割，B 背景，P 内边距，H 阴影，E 描边，L 图层，Del 删除，Esc 取消选中"]],
	]);
	const states = [
		[["Loading", "加载中"], ["The preview says Loading preview and the tools stay disabled until the image is ready.", "预览显示“正在加载预览”，图片就绪前工具不可用。"]],
		[["Couldn't open", "打不开"], ["Says what happened and shows Details: no PNG, unreadable image or not a screenshot.", "说明原因并提供详情：没有 PNG、图片无法读取或不是截图。"]],
		[["No layers yet", "还没有图层"], ["The Layers page says to use the tools to add annotations.", "图层页提示用工具添加标注。"]],
		[["Saving", "保存"], ["Autosave, no indicator. Closing flushes first.", "自动保存，不显示状态。关闭前先写入。"]],
		[["Copied or saved", "已复制或已保存"], ["A short toast. Copy and Save disable while an export runs.", "显示简短提示。导出进行时，复制和保存不可用。"]],
		[["Transparent", "透明"], ["The frame shows a checkerboard. Copy keeps alpha only when the background is invisible.", "画面显示棋盘格。只有背景不可见时，复制才保留透明。"]],
	];
	document.getElementById("states-grid").innerHTML = states.map(([h, b]) => `<div class="card"><h3>${L(h[0], h[1])}</h3><p>${L(b[0], b[1])}</p></div>`).join("");
	table("inv", "doc", [
		[["Select, draw, arrow, rectangle, mask, circle, text", "选择、画笔、箭头、矩形、遮罩、椭圆、文字"], ["Preview toolbar, same keys", "预览工具栏，快捷键不变"]],
		[["Drag to draw, Shift constrains, move, eight handles, arrow end points, double-click to edit text", "拖动绘制、Shift 约束、移动、八个手柄、箭头端点、双击改文字"], ["Preview, unchanged", "预览，不变"]],
		[["Stroke color, hex input, width, fill with transparent, opacity", "颜色、十六进制输入、宽度、含透明的填充、不透明度"], ["Inspector when selected (was a bar over the preview)", "选中后在检查器（原在预览上方的横条）"]],
		[["Mask: blur or pixelate, intensity, kept inside the image", "遮罩：模糊或像素化、强度、限制在图片内"], ["Inspector when selected", "选中后在检查器"]],
		[["Text color, size, inline editing", "文字颜色、大小、原位编辑"], ["Inspector when selected; editing on the canvas", "选中后在检查器；在画布上编辑"]],
		[["Done, Delete or Backspace, Escape", "完成、Delete 或 Backspace、Esc"], ["Inspector head and keyboard", "检查器顶部和键盘"]],
		[["Copy and paste an annotation", "复制粘贴标注"], ["Ctrl+C with a selection, Ctrl+V", "选中时 Ctrl+C，Ctrl+V"]],
		[["Layers: list, reorder, delete, hover highlight, empty state, top is in front", "图层：列表、排序、删除、悬停高亮、空状态、上面在前"], ["Layers page, L (was a left panel)", "图层页，L（原为左侧面板）"]],
		[["Remember the layers panel", "记住图层面板状态"], ["Replaced: the inspector remembers its last page", "替换：检查器记住上次的页面"]],
		[["Background: wallpaper with themes, image with drag and clear, color with hex and transparent, gradient with from, to and presets", "背景：带主题的壁纸、可拖入和清除的图片、带十六进制和透明的纯色、带起止色和预设的渐变"], ["Background page, B (was a popover)", "背景页，B（原为弹窗）"]],
		[["Background blur; animated gradient shown as a still with its note", "背景模糊；动态渐变按静态显示并提示"], ["Background page", "背景页"]],
		[["Padding, rounding, corner style", "内边距、圆角、圆角样式"], ["Screen page, P (was two popovers)", "画面页，P（原为两个弹窗）"]],
		[["Shadow with advanced size, opacity, blur", "阴影及高级大小、不透明度、模糊"], ["Screen page, H (was a popover)", "画面页，H（原为弹窗）"]],
		[["Border: toggle, width, color, opacity", "描边：开关、宽度、颜色、不透明度"], ["Screen page, E (was a popover)", "画面页，E（原为弹窗）"]],
		[["Aspect ratio: Auto, 16:9, 9:16, 1:1, 4:3, 3:4", "宽高比：自动、16:9、9:16、1:1、4:3、3:4"], ["Preview toolbar", "预览工具栏"]],
		[["Crop dialog: size and position fields, ratios, snap, Full, Reset, Save, arrow nudge, Shift and Alt, Escape", "裁剪对话框：大小和位置输入、比例、吸附、全图、重置、保存、方向键微调、Shift 和 Alt、Esc"], ["Preview toolbar, then the dialog", "预览工具栏，然后是对话框"]],
		[["Zoom 10 to 300%, Ctrl+wheel, pinch, Ctrl + and −, wheel pans", "缩放 10% 到 300%、Ctrl+滚轮、双指、Ctrl + 和 −、滚轮平移"], ["Status bar (was a floating control)", "状态栏（原为浮动控件）"]],
		[["Undo, redo, autosave", "撤销、重做、自动保存"], ["Title bar; autosave has no indicator", "标题栏；自动保存不显示状态"]],
		[["Copy, flattened on white unless transparent", "复制，非透明时铺白"], ["Title bar, Ctrl+C", "标题栏，Ctrl+C"]],
		[["Save as PNG, annotations past the frame kept", "存为 PNG，保留画面外的标注"], ["Title bar, Ctrl+S", "标题栏，Ctrl+S"]],
		[["Open folder, delete with confirmation", "打开文件夹、确认后删除"], ["Title bar icons (was a More menu)", "标题栏图标（原在更多菜单）"]],
		[["Loading preview, load errors, toasts", "加载预览、加载错误、提示"], ["Preview and toast", "预览和提示"]],
		[["After a screenshot: open, copy, or both", "截图后：打开、复制或两者"], ["Settings · Recording", "设置 · 录制"]],
		[["Open from the library, tray and deep link", "从资源库、托盘和深链接打开"], ["Unchanged; the library lists screenshots as items", "不变；资源库把截图列为项目"]],
		[["Step number, line, highlighter, speech bubble, spotlight, magnifier, image, cut out", "序号、直线、荧光笔、对话气泡、聚光灯、放大镜、图片、切割"], ["New in 1.0, taken from ShareX, Flameshot and Greenshot. Each needs a new annotation type; image and cut out also change what a project stores", "1.0 新增，来自 ShareX、Flameshot 和 Greenshot。各需要一种新的标注类型；图片和切割还会改变项目存储的内容"]],
		[["Ctrl+K command palette", "Ctrl+K 命令面板"], ["New, from the command palette decision", "新增，来自命令面板决定"]],
		[["Upstream share link, sign in to share, upgrade, upload", "上游的分享链接、登录分享、升级、上传"], ["Removed: FrameEver stays local", "去掉：FrameEver 只在本地使用"]],
		[["Upstream export fixes and speed-ups", "上游的导出修复和提速"], ["No new controls; ported by hand in their own ticket", "没有新控件；在单独的工单里手动移植"]],
	]);
};
shell({ here: "screenshot-editor", depth: 1, sections: [["live", "Screenshot editor", "截图编辑器"], ["layout", "Layout", "布局"], ["annotations", "Annotations", "标注"], ["states", "Other states", "其他状态"], ["inventory", "Where everything went", "功能位置"]] });
renderDocs();
onChange(() => {
	renderDocs();
	render();
});
render();
