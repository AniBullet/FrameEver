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
	padding: '<rect x="5" y="5" width="14" height="14" rx="2"/><path d="M12 8v.01M8 12v.01M16 12v.01M12 16v.01"/>',
	shadow: '<circle cx="12" cy="12" r="9"/><path d="M13 12h5M13 15h4M13 18h1M13 9h4M13 6h1"/>',
	hash: '<line x1="4" x2="20" y1="9" y2="9"/><line x1="4" x2="20" y1="15" y2="15"/><line x1="10" x2="8" y1="3" y2="21"/><line x1="16" x2="14" y1="3" y2="21"/>',
	highlighter: '<path d="m9 11-6 6v3h9l3-3"/><path d="m22 12-4.6 4.6a2 2 0 0 1-2.8 0l-5.2-5.2a2 2 0 0 1 0-2.8L14 4"/>',
	flashlight: '<path d="M18 6c0 2-2 2-2 4v10a2 2 0 0 1-2 2h-4a2 2 0 0 1-2-2V10c0-2-2-2-2-4V2h12z"/><line x1="6" x2="18" y1="6" y2="6"/><line x1="12" x2="12" y1="12" y2="12"/>',
});

const glyphs = {
	minimize: '<svg viewBox="0 0 10 10"><path d="M0 5.5h10"/></svg>',
	maximize: '<svg viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" rx="1"/></svg>',
	close: '<svg viewBox="0 0 10 10"><path d="m0.5 0.5 9 9m0-9-9 9"/></svg>',
};

const VARIANTS = {
	A: { en: "Inspector", zh: "检查器" },
	B: { en: "Floating dock", zh: "浮动工具坞" },
	C: { en: "Today's layout", zh: "现有结构" },
};
const TOOLS = [
	{ id: "select", icon: "mouse-pointer-2", en: "Select", zh: "选择", k: "V" },
	{ id: "draw", icon: "pencil", en: "Draw", zh: "画笔", k: "D" },
	{ id: "arrow", icon: "arrow-up-right", en: "Arrow", zh: "箭头", k: "A" },
	{ id: "rect", icon: "square", en: "Rectangle", zh: "矩形", k: "R" },
	{ id: "mask", icon: "eye-off", en: "Mask", zh: "遮罩", k: "M" },
	{ id: "circle", icon: "circle", en: "Circle", zh: "椭圆", k: "C" },
	{ id: "text", icon: "type", en: "Text", zh: "文字", k: "T" },
];
const PROPOSED = [
	{ id: "counter", icon: "hash", en: "Step number", zh: "序号", k: "N" },
	{ id: "marker", icon: "highlighter", en: "Highlighter", zh: "荧光笔", k: "G" },
	{ id: "spot", icon: "flashlight", en: "Spotlight", zh: "聚光灯", k: "F" },
];
const allTools = () => (S.proposed ? [...TOOLS, ...PROPOSED] : TOOLS);
const toolOf = (id) => [...TOOLS, ...PROPOSED].find((t) => t.id === id);

const POPS = {
	background: { icon: "image", en: "Background", zh: "背景", k: "B" },
	padding: { icon: "padding", en: "Padding", zh: "内边距", k: "P" },
	rounding: { icon: "square-round-corner", en: "Corners", zh: "圆角", k: "" },
	shadow: { icon: "shadow", en: "Shadow", zh: "阴影", k: "H" },
	border: { icon: "square", en: "Border", zh: "描边", k: "E" },
};
const COLORS = ["#F05656", "#FFFFFF", "#000000", "#F5A524", "#2CB67D", "#4785FF", "#1F3FD8", "#C05BD6"];
const OUT_W = 1440;
const OUT_H = 900;

const seed = () => [
	{ id: 1, t: "draw", x: 646, y: 92, w: 120, h: 44, stroke: "#F05656", width: 4, op: 1, pts: [[0, 0.6], [0.2, 0.2], [0.4, 0.8], [0.6, 0.3], [0.8, 0.9], [1, 0.4]] },
	{ id: 2, t: "mask", x: 112, y: 408, w: 250, h: 110, mask: "pixelate", level: 7 },
	{ id: 3, t: "rect", x: 540, y: 150, w: 410, h: 170, stroke: "#F05656", width: 4, fill: "transparent", op: 1 },
	{ id: 4, t: "arrow", x: 420, y: 300, w: 108, h: -66, stroke: "#F05656", width: 4, op: 1 },
	{ id: 5, t: "text", x: 300, y: 290, h: 34, stroke: "#F05656", op: 1, en: "Slower since Tuesday", zh: "周二开始变慢" },
];
const S = {
	variant: "A",
	tab: "background",
	tool: "select",
	sel: 3,
	menu: null,
	dialog: null,
	pop: null,
	popX: 0,
	display: null,
	zoom: 100,
	aspect: 0,
	proposed: false,
	layers: true,
	v: {},
	shut: {},
	anns: seed(),
	past: [],
	future: [],
	next: 6,
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
	rows.push({ t: "colors", k: "a.stroke", en: a.t === "text" || a.t === "counter" ? "Color" : "Stroke", zh: "颜色" });
	if (["draw", "arrow", "rect", "circle", "marker"].includes(a.t)) rows.push(sl("a.width", "Width", "宽度", 1, 40, a.width, " px"));
	if (a.t === "rect" || a.t === "circle") rows.push({ t: "colors", k: "a.fill", en: "Fill", zh: "填充", clear: true });
	rows.push(sl("a.opPct", "Opacity", "不透明度", 0, 100, Math.round((a.op ?? 1) * 100), "%"));
	if (a.t === "text" || a.t === "counter") rows.push(sl("a.h", "Size", "大小", 12, 120, a.h, " px"));
	return rows;
};
const kindName = (a) => {
	const t = toolOf(a.t);
	return L(t.en, t.zh);
};
const layerLabel = (a) => (a.t === "text" ? pick(a.en ?? a.zh, a.zh ?? a.en) : pick(toolOf(a.t).en, toolOf(a.t).zh));

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
		case "marker":
			return `<rect ${hit} opacity="0.4" x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}" fill="${a.stroke}"/>`;
		case "spot":
			return `<path ${hit} fill-rule="evenodd" fill="rgb(0 0 0 / ${(a.dim ?? 55) / 100})" d="M-400 -400H1400V1025H-400Z M${b.x} ${b.y}h${b.w}v${b.h}h${-b.w}Z"/>`;
		default:
			return "";
	}
};
const defsSvg = () =>
	`<defs>${S.anns
		.filter((a) => a.t === "mask")
		.map((a) => {
			const b = box(a);
			return `<clipPath id="clip-${a.id}"><rect x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}"/></clipPath>
			<filter id="fx-blur-${a.id}" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="${a.level * 0.9}"/></filter>`;
		})
		.join("")}</defs>`;
const handlesSvg = () => {
	const a = selected();
	if (!a || a.t === "text" || a.t === "counter") {
		if (!a) return "";
		const w = a.t === "text" ? layerLabel(a).length * a.h * 0.72 : a.h * 1.4;
		const x = a.t === "text" ? a.x : a.x - a.h * 0.7;
		const y = a.t === "text" ? a.y : a.y - a.h * 0.7;
		return `<rect x="${x - 4}" y="${y - 4}" width="${w + 8}" height="${(a.t === "text" ? a.h * 1.25 : a.h * 1.4) + 8}" fill="none" stroke="var(--ring)" stroke-width="1.5" pointer-events="none"/>`;
	}
	const dot = (x, y) => `<circle cx="${x}" cy="${y}" r="6" fill="#fff" stroke="var(--ring)" stroke-width="2" pointer-events="none"/>`;
	if (a.t === "arrow") return dot(a.x, a.y) + dot(a.x + a.w, a.y + a.h);
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

const titleActions = () => `<button class="ib copy-less save-btn" data-act="save" title="Ctrl+S">${icon("download")}${L("Save", "保存")}</button>
	<button class="export-btn copy-btn" data-act="copy" title="Ctrl+C">${L("Copy", "复制")}</button>`;
const captions = () => `<span class="caption">${glyphs.minimize}</span><span class="caption">${glyphs.maximize}</span><span class="caption">${glyphs.close}</span>`;
const history = () => `<button class="ib" data-act="undo" title="${pick("Undo", "撤销")} Ctrl+Z" ${S.past.length ? "" : "disabled"}>${icon("undo")}</button>
	<button class="ib" data-act="redo" title="${pick("Redo", "重做")} Ctrl+Y" ${S.future.length ? "" : "disabled"}>${icon("redo")}</button>`;
const nameBlock = () => `<span class="name">Monthly report<em>.ever</em></span>
	<button class="ib" data-act="noop" title="${pick("Show in Explorer", "在资源管理器中显示")}">${icon("folder")}</button>
	<button class="ib" data-act="dialog" data-d="delete" title="${pick("Delete screenshot", "删除截图")}">${icon("trash")}</button>`;
const titlebar = () => `<div class="titlebar">${nameBlock()}<span class="grow"></span>${history()}<span class="sep"></span>${titleActions()}${captions()}</div>`;

const aspects = [["Auto", "自动"], ["Wide 16:9", "宽屏 16:9"], ["Vertical 9:16", "竖屏 9:16"], ["Square 1:1", "方形 1:1"], ["Classic 4:3", "经典 4:3"], ["Tall 3:4", "高屏 3:4"]];
const aspectValue = [null, 16 / 9, 9 / 16, 1, 4 / 3, 3 / 4];
const aspectBtn = (short) => `<button class="ib ${S.menu === "aspect" ? "on" : ""}" data-act="menu" data-m="aspect" title="${pick("Aspect ratio", "宽高比")}">${icon("proportions")}${short ? "" : L(...aspects[S.aspect])}${icon("chevron-down")}</button>`;
const cropBtn = (short) => `<button class="ib" data-act="dialog" data-d="crop" title="${pick("Crop image", "裁剪图片")}">${icon("crop")}${short ? "" : L("Crop", "裁剪")}</button>`;
const toolBtns = () => `<span class="tools">${allTools()
	.map((t) => `<button class="ib ${S.tool === t.id ? "on" : ""}" data-act="tool" data-v="${t.id}" title="${pick(t.en, t.zh)} ${t.k}" aria-label="${pick(t.en, t.zh)}">${icon(t.icon)}</button>`)
	.join("")}</span>`;
const popBtns = () =>
	Object.entries(POPS)
		.map(([id, p]) => `<button class="ib ${S.pop === id ? "on" : ""}" data-act="pop" data-v="${id}" title="${pick(p.en, p.zh)} ${p.k}" aria-label="${pick(p.en, p.zh)}">${icon(p.icon)}</button>`)
		.join("");
const zoomCtl = () => `<button class="ib" data-act="zoom" data-v="-10" title="${pick("Zoom out", "缩小")} Ctrl −">${icon("zoom-out")}</button>
	<input class="zoomer" type="range" min="10" max="300" step="10" value="${S.zoom}" style="--p:${percent(S.zoom, 10, 300)}%" data-range="__zoom" aria-label="${pick("Zoom", "缩放")}">
	<button class="ib" data-act="zoom" data-v="10" title="${pick("Zoom in", "放大")} Ctrl +">${icon("zoom-in")}</button>
	<span class="pct" id="pct">${S.zoom}%</span>`;
const stage = (extra = "") => `<div class="ss-stage" id="ss-stage"><div class="frame" id="frame"><div class="shot" id="shot"><div class="pic">${sceneSvg()}</div><svg class="overlay ${S.tool === "select" ? "" : "drawing"}" id="overlay" viewBox="0 0 1000 625" aria-label="${pick("Annotations", "标注")}">${annLayer()}</svg></div></div>${extra}</div>`;
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
const layoutA = () => `${titlebar()}<div class="ed-body"><div class="col"><div class="row-top">
	<div class="player">
		<div class="toolbar center">${aspectBtn()}${cropBtn()}<span class="grow"></span>${toolBtns()}<span class="grow"></span><span style="width:166px"></span></div>
		${stage()}
		<div class="status">${zoomCtl()}<span class="grow"></span><span>${L("Output", "输出")} ${outputSize()}</span></div>
	</div>
	${railA()}${panelA()}
</div></div></div>`;

const chipRange = (k, en, zh, min, max, v, unit = "") => `<label class="chip">${L(en, zh)}<input type="range" style="--p:${percent(v, min, max)}%" data-range="${k}" data-unit="${unit}" min="${min}" max="${max}" value="${v}"><output>${v}${unit}</output></label>`;
const chipColor = (field, en, zh) => {
	const c = selected()[field];
	return `<span class="chip">${L(en, zh)}<button class="dot ${c === "transparent" ? "none" : ""}" style="background:${c === "transparent" ? "repeating-conic-gradient(#d0d0d0 0 25%,#fff 0 50%) 0 0/8px 8px" : c}" data-act="menu" data-m="color-${field}" aria-label="${pick(en, zh)}"></button></span>`;
};
const barChips = (a) => {
	if (a.t === "mask") return `<span class="seg-row" style="padding:0">${[["Blur", "模糊", 0], ["Pixelate", "像素化", 1]].map(([en, zh, i]) => `<button class="${(a.mask === "blur" ? 0 : 1) === i ? "on" : ""}" data-act="set" data-k="a.maskI" data-v="${i}">${L(en, zh)}</button>`).join("")}</span>${chipRange("a.level", "Intensity", "强度", 1, 32, a.level)}`;
	if (a.t === "spot") return chipRange("a.dim", "Darkness", "暗度", 0, 100, a.dim, "%");
	const parts = [chipColor("stroke", a.t === "text" || a.t === "counter" ? "Color" : "Stroke", "颜色")];
	if (["draw", "arrow", "rect", "circle", "marker"].includes(a.t)) parts.push(chipRange("a.width", "Width", "宽度", 1, 40, a.width, " px"));
	if (a.t === "rect" || a.t === "circle") parts.push(chipColor("fill", "Fill", "填充"));
	parts.push(chipRange("a.opPct", "Opacity", "不透明度", 0, 100, Math.round((a.op ?? 1) * 100), "%"));
	if (a.t === "text" || a.t === "counter") parts.push(chipRange("a.h", "Size", "大小", 12, 120, a.h, " px"));
	return parts.join("");
};
const barEnd = (a) => `<span class="sep"></span><button class="ib" data-act="del" data-v="${a.id}" title="${pick("Delete", "删除")} Del">${icon("trash")}</button><button class="ib" data-act="deselect">${L("Done", "完成")}</button>`;

const popBody = () => {
	switch (S.pop) {
		case "background":
			return bgGroups().map(group).join("");
		case "padding":
			return `<div class="grp-body" style="padding-top:10px">${padRows.map(row).join("")}</div>`;
		case "rounding":
			return `<div class="grp-body" style="padding-top:10px">${roundRows.map(row).join("")}</div>`;
		case "shadow":
			return group(shadowGroup(true));
		case "border":
			return group(borderGroup(true));
		default:
			return "";
	}
};
const popWidth = { background: 340, padding: 280, rounding: 300, shadow: 300, border: 300 };
const floatPop = (style) => (S.pop ? `<div class="float-panel" style="width:${popWidth[S.pop]}px;${style}">${panelHead(L(POPS[S.pop].en, POPS[S.pop].zh), `<button class="ib" data-act="pop" data-v="${S.pop}">${icon("x")}</button>`)}<div class="panel-body" id="panel-body">${popBody()}</div></div>` : "");
const ctxPlace = (a) => {
	const b = box(a);
	const top = Math.max(2, (b.y / 625) * 100 - 9);
	return `left:50%;top:${top}%;transform:translateX(-50%)`;
};
const layoutB = () => {
	const a = selected();
	const ctx = a ? `<div class="ctx-bar float" style="${ctxPlace(a)}">${barChips(a)}${barEnd(a)}</div>` : "";
	const layers = S.layers ? `<div class="float-panel" style="left:12px;top:12px;width:220px;max-height:300px">${panelHead(L("Layers", "图层"), `<button class="ib" data-act="layers">${icon("x")}</button>`)}<div class="panel-body">${layerRows()}</div></div>` : "";
	const dock = `<div class="dock">${aspectBtn(true)}${cropBtn(true)}<span class="sep"></span>${toolBtns()}<span class="sep"></span>${popBtns()}<span class="sep"></span><button class="ib ${S.layers ? "on" : ""}" data-act="layers" title="${pick("Layers", "图层")} L">${icon("layers")}</button></div>`;
	const hud = `<div class="zoom-hud">${zoomCtl()}</div>`;
	return `${titlebar()}<div class="ed-body"><div class="col"><div class="row-top"><div class="player">${stage(`${layers}${ctx}${floatPop("right:12px;bottom:80px")}${dock}${hud}`)}</div></div></div></div>`;
};

const layoutC = () => {
	const a = selected();
	const bar = a ? `<div class="ctx-bar fixed">${barChips(a)}${barEnd(a)}</div>` : "";
	const head = `<div class="titlebar">${aspectBtn(true)}${cropBtn(true)}<span class="sep"></span><button class="ib ${S.layers ? "on" : ""}" data-act="layers" title="${pick("Layers", "图层")} L">${icon("layers")}</button><span class="sep"></span>${toolBtns()}<span class="sep"></span>${popBtns()}<span class="grow"></span>${history()}<span class="sep"></span>${titleActions()}<button class="ib ${S.menu === "more" ? "on" : ""}" data-act="menu" data-m="more" title="${pick("More actions", "更多操作")}">${icon("more-horizontal")}</button>${captions()}</div>`;
	const side = S.layers ? `<div class="side-layers">${panelHead(L("Layers", "图层"), `<button class="ib" data-act="layers">${icon("x")}</button>`)}<div class="list">${layerRows()}</div></div>` : "";
	return `${head}<div class="ed-body"><div class="col"><div class="row-top c">${side}<div class="player" style="position:relative">${bar}${stage(`<div class="zoom-hud">${zoomCtl()}</div>`)}</div></div></div></div>${floatPop(`left:${S.popX}px;top:48px`)}`;
};

const menuAt = { A: "left:8px;top:84px", B: "left:50%;bottom:68px;margin-left:-300px", C: "left:8px;top:44px" };
const menus = {
	aspect: () => `<div class="pop menu" style="${menuAt[S.variant]};width:200px">${aspects.map((a, i) => `<button data-act="aspect" data-v="${i}"><span class="grow">${L(a[0], a[1])}</span>${S.aspect === i ? icon("check") : ""}</button>`).join("")}</div>`,
	more: () => `<div class="pop menu" style="right:136px;top:44px;width:200px"><button data-act="close">${icon("folder")}${L("Open folder", "打开文件夹")}</button><button class="danger" data-act="dialog" data-d="delete">${icon("trash")}${L("Delete", "删除")}</button></div>`,
};
const colorMenu = (field) => `<div class="pop menu" style="left:50%;top:108px;margin-left:-110px;width:220px;padding:10px"><div class="colors" style="justify-content:flex-start">${colorDots(`a.${field}`, field === "fill")}</div><div class="input" style="height:28px;margin-top:8px"><input value="${selected()?.[field] ?? ""}" aria-label="Hex" readonly></div></div>`;
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
		<div class="seg">${[[false, "Parity tools", "现有工具"], [true, "Plus proposed tools", "加上建议工具"]].map(([v, en, zh]) => `<button class="${S.proposed === v ? "on" : ""}" data-pact="proposed" data-v="${v ? 1 : 0}">${L(en, zh)}</button>`).join("")}</div>
		<span class="hint">${L(`${S.anns.length} layers · tool ${toolOf(S.tool).en} · ${S.sel ? "selected" : "nothing selected"}`, `${S.anns.length} 个图层 · 工具：${toolOf(S.tool).zh} · ${S.sel ? "已选中" : "未选中"}`)}</span>`;
};
const renderSwitch = () => {
	const v = VARIANTS[S.variant];
	document.getElementById("switch").innerHTML = `<button data-pact="cycle" data-v="-1" aria-label="${pick("Previous variant", "上一个变体")}">←</button><b>${S.variant} · ${L(v.en, v.zh)}</b><button data-pact="cycle" data-v="1" aria-label="${pick("Next variant", "下一个变体")}">→</button>`;
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
const layouts = { A: layoutA, B: layoutB, C: layoutC };
const render = () => {
	const prior = document.getElementById("panel-body")?.scrollTop ?? 0;
	const stacked = display() === "stacked";
	const colorKey = S.menu?.startsWith("color-") ? S.menu.slice(6) : null;
	document.getElementById("stage").innerHTML = `<div class="ed-fit" id="ed-fit"><div class="ed ${stacked ? "stacked" : ""}" id="ed">
		${layouts[S.variant]()}
		${colorKey ? colorMenu(colorKey) : S.menu ? menus[S.menu]() : ""}
		${S.dialog ? dialogs[S.dialog]() : ""}
	</div></div>`;
	const panelNode = document.getElementById("panel-body");
	if (panelNode) panelNode.scrollTop = prior;
	renderViewRow();
	renderSwitch();
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
const openPop = (id, el) => {
	if (S.variant === "A") {
		S.tab = id === "background" ? "background" : "screen";
		S.sel = null;
		return;
	}
	S.pop = S.pop === id ? null : id;
	S.menu = null;
	if (el) {
		const ed = document.getElementById("ed");
		const scale = ed.getBoundingClientRect().width / ed.offsetWidth || 1;
		S.popX = Math.min(ed.offsetWidth - popWidth[id] - 8, (el.getBoundingClientRect().left - ed.getBoundingClientRect().left) / scale);
	}
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
			S.pop = null;
			break;
		case "pop":
			openPop(d.v, el);
			break;
		case "layers":
			S.layers = !S.layers;
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
const cycle = (delta) => {
	const keys = Object.keys(VARIANTS);
	S.variant = keys[(keys.indexOf(S.variant) + delta + keys.length) % keys.length];
	S.pop = null;
	S.menu = null;
	S.dialog = null;
	render();
};
const pact = (el) => {
	const d = el.dataset;
	if (d.pact === "display") S.display = d.v;
	if (d.pact === "proposed") {
		S.proposed = d.v === "1";
		if (!S.proposed && PROPOSED.some((t) => t.id === S.tool)) S.tool = "select";
	}
	if (d.pact === "cycle") return cycle(Number(d.v));
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
	if (["rect", "circle", "arrow", "draw"].includes(tool)) Object.assign(a, { stroke: "#F05656", width: 4, fill: "transparent" });
	if (tool === "draw") a.raw = [[x, y]];
	if (tool === "mask") Object.assign(a, { mask: "pixelate", level: 7 });
	if (tool === "marker") Object.assign(a, { stroke: "#F5A524", width: 20 });
	if (tool === "spot") a.dim = 55;
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
		if (a && ["rect", "circle", "mask", "marker", "spot"].includes(a.t) && Math.abs(a.w) < 5 && Math.abs(a.h) < 5) {
			Object.assign(a, { x: a.x - 60, y: a.y - 40, w: 120, h: 80 });
		}
		if (a?.t === "draw" && !a.pts) {
			S.anns = S.anns.filter((n) => n !== a);
			S.tool = "select";
			render();
			return;
		}
		if (a?.t === "arrow" && Math.abs(a.w) < 5 && Math.abs(a.h) < 5) Object.assign(a, { w: 90, h: -60 });
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
	if (k === "arrowleft") return cycle(-1);
	if (k === "arrowright") return cycle(1);
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
		if (S.dialog || S.menu || S.pop) {
			S.dialog = null;
			S.menu = null;
			S.pop = null;
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
	const tool = allTools().find((t) => t.k.toLowerCase() === k);
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
	const pop = Object.entries(POPS).find(([, p]) => p.k.toLowerCase() === k);
	if (pop) {
		const el = document.querySelector(`[data-act="pop"][data-v="${pop[0]}"]`);
		openPop(pop[0], el);
		render();
		return;
	}
	if (k === "l") {
		if (S.variant === "A") {
			S.tab = "layers";
			S.sel = null;
		} else S.layers = !S.layers;
		render();
	}
});
window.addEventListener("resize", () => {
	fit();
	sizeFrame();
});

const table = (id, cls, head, rows) => {
	const headHtml = head ? `<thead><tr>${head.map((c) => `<th>${L(c[0], c[1])}</th>`).join("")}</tr></thead>` : "";
	document.getElementById(id).innerHTML = `<table class="${cls}">${headHtml}<tbody>${rows.map((r) => `<tr>${r.map((c, i) => `<td class="${i ? "" : "dim"}">${Array.isArray(c) ? L(c[0], c[1]) : c}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
};
const renderDocs = () => {
	const heads = {
		"t-live": ["Screenshot editor", "截图编辑器"],
		"l-live": ["Three structures for the same editor. Use the pill at the bottom to switch. Tap an annotation to select it, pick a tool and drag on the screenshot to draw. Keyboard shortcuts work on a PC.", "同一个编辑器的三种结构，用底部的胶囊切换。点标注可以选中，选工具后在截图上拖动可以绘制。电脑上快捷键可用。"],
		"t-compare": ["How they differ", "三者的区别"],
		"l-compare": ["Same capabilities in all three. Only where things live changes.", "三个变体功能完全相同，只是位置不同。"],
		"t-research": ["What was looked at", "看过什么"],
		"l-research": ["Current screenshot and annotation tools, checked on 2026-10-31.", "当前的截图和标注工具，2026-10-31 查看。"],
		"t-inv": ["Where everything went", "每项功能的位置"],
		"l-inv": ["Every capability in screenshot_editor.rs, screenshot_annotations.rs, screenshot_crop.rs and screenshot_export.rs, plus what upstream has now. Places are for variant A.", "screenshot_editor.rs、screenshot_annotations.rs、screenshot_crop.rs 和 screenshot_export.rs 的全部功能，加上上游现状。位置按变体 A 写。"],
	};
	for (const [id, text] of Object.entries(heads)) document.getElementById(id).innerHTML = L(text[0], text[1]);
	table("compare-table", "doc", [["", ""], ["A Inspector", "A 检查器"], ["B Floating dock", "B 浮动工具坞"], ["C Today's layout", "C 现有结构"]], [
		[["Tools", "工具"], ["Toolbar above the preview", "预览上方的工具栏"], ["Dock floating over the canvas", "浮在画布上的工具坞"], ["In the title bar", "在标题栏里"]],
		[["Annotation options", "标注选项"], ["Inspector swaps, as in the editor", "检查器切换，和编辑器一样"], ["Bar next to the selection", "选中对象旁的横条"], ["Bar under the title bar", "标题栏下的横条"]],
		[["Background and style", "背景和样式"], ["Inspector, always visible", "检查器，始终可见"], ["Panel opened from the dock", "从工具坞打开面板"], ["One popover each", "各自一个弹窗"]],
		[["Layers", "图层"], ["Inspector page", "检查器的一页"], ["Floating card", "浮动卡片"], ["Left column", "左侧栏"]],
		[["Canvas size", "画布大小"], ["Smallest, 340 px gone", "最小，少 340 px"], ["Largest", "最大"], ["Large", "较大"]],
		[["Clicks to change padding", "改内边距的点击数"], ["1 or 2", "1 到 2 次"], ["2", "2 次"], ["2", "2 次"]],
		[["Fits the editor", "和编辑器一致"], ["Yes: title bar, rail, 340 inspector", "是：标题栏、图标栏、340 检查器"], ["Title bar only", "只有标题栏"], ["No", "否"]],
	]);
	table("research-table", "doc", [["Product", "产品"], ["What it does", "做法"], ["What we take", "取用"]], [
		["ScreenSnap Pro", ["Annotation tools float above the canvas; a right style panel sets padding, corners, shadow, background; Copy and Save in the top bar", "标注工具浮在画布上方；右侧样式面板管内边距、圆角、阴影、背景；复制和保存在顶栏"], ["Split between markup and output styling, used by A", "标注和成品样式分开，A 采用"]],
		["CleanShot X", ["Counter, spotlight, highlighter, four arrow styles, text styles; background tool with padding, alignment, auto balance, presets", "序号、聚光灯、荧光笔、四种箭头、文字样式；背景工具有内边距、对齐、自动平衡、预设"], ["Candidates: step number, highlighter, spotlight", "候选：序号、荧光笔、聚光灯"]],
		["Shottr", ["Single-key tools, backdrop, spotlight, step counter, text-only blur, ruler", "单键切工具、背景、聚光灯、序号、仅模糊文字、尺子"], ["Single-key tools, already ours", "单键切工具，我们已有"]],
		["PixPin, Snipaste", ["Pinning on screen, OCR, long capture, mosaic", "贴图、OCR、长截图、马赛克"], ["Not in this ticket: they are capture features", "不属于本工单：它们是采集功能"]],
		["Flameshot, ShareX", ["Numbering tool; large tool sets for power users", "序号工具；面向高级用户的大量工具"], ["Numbering again confirmed as expected", "序号再次得到印证"]],
	]);
	table("inv", "doc", [["Capability", "功能"], ["Place", "位置"]], [
		[["Select, draw, arrow, rectangle, mask, circle, text", "选择、画笔、箭头、矩形、遮罩、椭圆、文字"], ["Toolbar; V D A R M C T, and O for circle, S for select", "工具栏；V D A R M C T，O 也是椭圆，S 也是选择"]],
		[["Drag to draw, Shift constrains, move, eight handles, arrow end points, double-click to edit text", "拖动绘制、Shift 约束、移动、八个手柄、箭头端点、双击改文字"], ["Canvas, unchanged", "画布，不变"]],
		[["Stroke color, width, fill with transparent, opacity", "颜色、宽度、含透明的填充、不透明度"], ["Inspector when selected, per annotation type as today", "选中后在检查器，按类型显示，和现在一样"]],
		[["Hex color input", "十六进制颜色输入"], ["Inspector, in the color control", "检查器的颜色控件"]],
		[["Mask: blur or pixelate, intensity, kept inside the image", "遮罩：模糊或像素化、强度、限制在图片内"], ["Inspector when selected", "选中后在检查器"]],
		[["Text color, size, inline editing", "文字颜色、大小、原位编辑"], ["Inspector when selected; editing on the canvas", "选中后在检查器；在画布上编辑"]],
		[["Done, Delete or Backspace, Escape", "完成、Delete 或 Backspace、Esc"], ["Inspector head and keyboard", "检查器顶部和键盘"]],
		[["Copy and paste an annotation", "复制粘贴标注"], ["Ctrl+C with a selection, Ctrl+V", "选中时 Ctrl+C，Ctrl+V"]],
		[["Layers: list, reorder, delete, hover highlight, empty state, Top = front", "图层：列表、排序、删除、悬停高亮、空状态、上面在前"], ["Layers page in the rail, L", "图标栏的图层页，L"]],
		[["Remember layers panel open", "记住图层面板状态"], ["Replaced: the inspector remembers its last page", "替换：检查器记住上次的页面"]],
		[["Background: wallpaper with themes, image with drag and clear, color with hex and transparent, gradient with from, to and presets", "背景：带主题的壁纸、可拖入和清除的图片、带十六进制和透明的纯色、带起止色和预设的渐变"], ["Background page, B", "背景页，B"]],
		[["Background blur", "背景模糊"], ["Background page", "背景页"]],
		[["Animated gradient shown as a still, with its note", "动态渐变按静态显示并提示"], ["Background page", "背景页"]],
		[["Padding, rounding, corner style", "内边距、圆角、圆角样式"], ["Screen page; P opens it", "画面页；P 打开"]],
		[["Shadow and advanced size, opacity, blur", "阴影和高级大小、不透明度、模糊"], ["Screen page; H opens it", "画面页；H 打开"]],
		[["Border: toggle, width, color, opacity", "描边：开关、宽度、颜色、不透明度"], ["Screen page; E opens it", "画面页；E 打开"]],
		[["Aspect ratio: Auto, 16:9, 9:16, 1:1, 4:3, 3:4", "宽高比：自动、16:9、9:16、1:1、4:3、3:4"], ["Preview toolbar", "预览工具栏"]],
		[["Crop dialog: size and position fields, ratios, snap, Full, Reset, Save, arrow nudge, Shift and Alt, Escape", "裁剪对话框：大小和位置输入、比例、吸附、全图、重置、保存、方向键微调、Shift 和 Alt、Esc"], ["Preview toolbar, then the dialog", "预览工具栏，然后是对话框"]],
		[["Zoom 10 to 300%, Ctrl+wheel, pinch, Ctrl + and −, wheel pans", "缩放 10% 到 300%、Ctrl+滚轮、双指、Ctrl + 和 −、滚轮平移"], ["Status bar under the preview", "预览下方的状态栏"]],
		[["Undo, redo, autosave", "撤销、重做、自动保存"], ["Title bar; autosave has no indicator", "标题栏；自动保存不显示状态"]],
		[["Copy, flattened on white unless transparent", "复制，非透明时铺白"], ["Title bar, Ctrl+C", "标题栏，Ctrl+C"]],
		[["Save as PNG, off-frame annotations kept", "存为 PNG，保留画面外的标注"], ["Title bar, Ctrl+S", "标题栏，Ctrl+S"]],
		[["Open folder, delete with confirmation", "打开文件夹、确认后删除"], ["Title bar icons (was a More menu)", "标题栏图标（原在更多菜单）"]],
		[["Loading preview, load errors, toasts", "加载预览、加载错误、提示"], ["Stage and toast", "画布区和提示"]],
		[["After a screenshot: open, copy, or both", "截图后：打开、复制或两者"], ["Settings · Recording, already placed", "设置 · 录制，已定"]],
		[["Open from the library, tray, deep link", "从资源库、托盘、深链接打开"], ["Unchanged; a separate window per screenshot", "不变；每张截图一个窗口"]],
		[["Upstream: share link, sign in to share, upgrade, upload", "上游：分享链接、登录分享、升级、上传"], ["Dropped: cloud and account", "去掉：云和账号功能"]],
		[["Upstream since the fork: no new local capability, only fixes, speed and translations", "上游分叉后：没有新的本地功能，只有修复、提速和翻译"], ["Worth porting by hand: export fixes, listed in the ticket", "值得手动移植：导出修复，列在工单里"]],
		[["Ctrl+K command palette", "Ctrl+K 命令面板"], ["New here, from the command palette decision", "新增，来自命令面板决定"]],
	]);
};
shell({ here: "screenshot-editor", depth: 1, sections: [["live", "Screenshot editor", "截图编辑器"], ["compare", "How they differ", "三者区别"], ["research", "What was looked at", "参考"], ["inventory", "Where everything went", "功能位置"]] });
renderDocs();
onChange(() => {
	renderDocs();
	render();
});
render();
