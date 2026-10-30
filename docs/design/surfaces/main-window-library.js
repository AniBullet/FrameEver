const { L, icon, pick, shell, onChange, tokens } = FE;

const KIND = {
	studio: { icon: "clapperboard", name: ["Studio", "工作室"], tab: ["Studio", "工作室"] },
	instant: { icon: "zap", name: ["Instant", "即时"], tab: ["Instant", "即时"] },
	screenshot: { icon: "image", name: ["Screenshot", "截图"], tab: ["Screenshots", "截图"] },
};
const GROUP = { today: ["Today", "今天"], yesterday: ["Yesterday", "昨天"], week: ["This week", "本周"], oct: ["October 2026", "2026 年 10 月"] };
const DAY = { today: ["Today", "今天"], yesterday: ["Yesterday", "昨天"], week: ["Monday", "周一"], oct: ["Oct 12", "10 月 12 日"] };

const ITEMS = [
	{ id: 1, name: "Product demo", kind: "studio", dur: "3:42", size: "182 MB", mb: 182, res: "2560 × 1440", fps: 60, group: "today", time: "10:12", art: 0, tracks: ["Camera, microphone", "摄像头、麦克风"], exports: [{ file: "Product demo.mp4", size: "64 MB", time: ["Today 10:40", "今天 10:40"] }, { file: "Product demo.gif", size: "21 MB", time: ["Today 10:44", "今天 10:44"], gone: true }] },
	{ id: 2, name: "Bug repro", kind: "instant", dur: "0:48", size: "38 MB", mb: 38, res: "1920 × 1080", fps: 30, group: "today", time: "09:30", art: 1, tracks: ["Microphone", "麦克风"], compressed: { size: "9.6 MB", saved: "75%" }, exports: [] },
	{ id: 3, name: "Settings page", kind: "screenshot", size: "1.2 MB", mb: 1.2, res: "1440 × 900", group: "today", time: "08:51", art: 2, exports: [] },
	{ id: 4, name: "Onboarding walk", kind: "studio", dur: "7:05", size: "410 MB", mb: 410, res: "2560 × 1440", fps: 60, group: "yesterday", time: "16:20", art: 3, tracks: ["Camera, microphone", "摄像头、麦克风"], exports: [] },
	{ id: 5, name: "Quick reply", kind: "instant", dur: "0:21", size: "16 MB", mb: 16, res: "1920 × 1080", fps: 30, group: "yesterday", time: "11:08", art: 4, tracks: ["Microphone", "麦克风"], exports: [{ file: "Quick reply.mp4", size: "16 MB", time: ["Yesterday 11:09", "昨天 11:09"] }] },
	{ id: 6, name: "Timeline glitch", kind: "screenshot", size: "860 KB", mb: 0.8, res: "1280 × 720", group: "yesterday", time: "10:02", art: 1, exports: [] },
	{ id: 7, name: "Release notes", kind: "studio", dur: "2:10", size: "96 MB", mb: 96, res: "1920 × 1080", fps: 60, group: "week", time: "14:33", art: 2, tracks: ["Microphone", "麦克风"], exports: [{ file: "Release notes.mp4", size: "31 MB", time: ["Monday 15:00", "周一 15:00"] }] },
	{ id: 8, name: "Teleprompter take", kind: "studio", dur: "4:33", size: "208 MB", mb: 208, res: "1920 × 1080", fps: 30, group: "week", time: "09:47", art: 4, tracks: ["Camera, microphone", "摄像头、麦克风"], exports: [] },
	{ id: 9, name: "Support clip", kind: "instant", dur: "1:02", size: "52 MB", mb: 52, res: "1920 × 1080", fps: 30, group: "oct", time: "13:05", art: 3, tracks: ["Microphone", "麦克风"], compressed: { size: "12 MB", saved: "77%" }, compressedOnly: true, exports: [] },
	{ id: 10, name: "Error dialog", kind: "screenshot", size: "540 KB", mb: 0.5, res: "960 × 600", group: "oct", time: "12:40", art: 0, exports: [] },
];

const art = (index) => {
	const scenes = [
		'<rect width="320" height="180" fill="#2B3A55"/><rect x="28" y="24" width="264" height="132" rx="6" fill="#F2F1EC"/><rect x="28" y="24" width="60" height="132" fill="#D9D6CB"/><rect x="100" y="40" width="120" height="10" rx="2" fill="#2B3A55"/><rect x="100" y="60" width="176" height="6" rx="2" fill="#A9A79C"/><rect x="100" y="74" width="150" height="6" rx="2" fill="#A9A79C"/><rect x="100" y="96" width="80" height="44" rx="4" fill="#E2725B"/><rect x="188" y="96" width="88" height="44" rx="4" fill="#3E7CB1"/>',
		'<rect width="320" height="180" fill="#1E1F24"/><rect x="0" y="0" width="320" height="22" fill="#2C2E36"/><circle cx="14" cy="11" r="4" fill="#E2725B"/><circle cx="28" cy="11" r="4" fill="#E0B04C"/><circle cx="42" cy="11" r="4" fill="#6FB98F"/><rect x="18" y="38" width="130" height="6" rx="2" fill="#6FB98F"/><rect x="34" y="54" width="180" height="6" rx="2" fill="#8FA7C9"/><rect x="34" y="70" width="110" height="6" rx="2" fill="#D9C27E"/><rect x="18" y="86" width="90" height="6" rx="2" fill="#6FB98F"/><rect x="34" y="102" width="210" height="6" rx="2" fill="#8FA7C9"/><rect x="34" y="118" width="150" height="6" rx="2" fill="#C98FA7"/>',
		'<rect width="320" height="180" fill="#E9E6DF"/><rect x="20" y="20" width="84" height="140" rx="6" fill="#FFFFFF"/><rect x="116" y="20" width="184" height="64" rx="6" fill="#FFFFFF"/><rect x="116" y="96" width="88" height="64" rx="6" fill="#4B7F6B"/><rect x="212" y="96" width="88" height="64" rx="6" fill="#D88C4A"/><rect x="32" y="34" width="60" height="8" rx="2" fill="#B7B3A8"/><rect x="32" y="52" width="48" height="8" rx="2" fill="#D3CFC4"/><rect x="32" y="70" width="56" height="8" rx="2" fill="#D3CFC4"/><rect x="130" y="36" width="120" height="10" rx="2" fill="#4B4A45"/><rect x="130" y="56" width="150" height="6" rx="2" fill="#C9C5BA"/>',
		'<rect width="320" height="180" fill="#3F5E5A"/><circle cx="238" cy="58" r="30" fill="#E8C468"/><path d="M0 180V120l70-44 62 40 56-30 132 70v24z" fill="#264D47"/><path d="M0 180v-30l96-30 88 26 136-16v50z" fill="#1A3531"/>',
		'<rect width="320" height="180" fill="#5B4B8A"/><rect x="42" y="34" width="236" height="112" rx="8" fill="#17171C"/><rect x="58" y="50" width="204" height="64" rx="4" fill="#2C2B3A"/><rect x="58" y="124" width="204" height="6" rx="3" fill="#3C3B4C"/><rect x="58" y="124" width="84" height="6" rx="3" fill="#F2F1EC"/><path d="M150 68l26 14-26 14z" fill="#F2F1EC"/>',
	];
	return `<svg class="art" viewBox="0 0 320 180" preserveAspectRatio="xMidYMid slice" aria-hidden="true">${scenes[index]}</svg>`;
};
const captionGlyph = {
	minimize: '<svg viewBox="0 0 10 10"><path d="M0 5.5h10"/></svg>',
	maximize: '<svg viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" rx="1"/></svg>',
	close: '<svg viewBox="0 0 10 10"><path d="m0.5 0.5 9 9m0-9-9 9"/></svg>',
};
const kbd = (...keys) => keys.map((key) => `<span class="kbd">${key}</span>`).join("");
const statusFilters = [
	["exported", "Exported", "已导出", (i) => i.exports.length > 0],
	["unexported", "Not exported", "未导出", (i) => i.kind !== "screenshot" && i.exports.length === 0],
	["compressed", "Has a compressed copy", "有压缩副本", (i) => Boolean(i.compressed)],
	["compressedOnly", "Compressed copy only", "只剩压缩副本", (i) => Boolean(i.compressedOnly)],
];
const sorts = [
	["date", "Date", "日期"],
	["name", "Name", "名称"],
	["size", "Size", "大小"],
	["duration", "Duration", "时长"],
];
const seconds = (item) => {
	if (!item.dur) return 0;
	const [m, s] = item.dur.split(":").map(Number);
	return m * 60 + s;
};
const defaults = () => ({
	expanded: false, rail: false, mode: "studio", target: "display", view: "grid", type: "all", query: "", status: [], sort: "date", tile: 150,
	selected: [], anchor: null, preview: false, palette: null, dialog: null, toast: null, menu: null, context: null, recording: false,
	items: ITEMS, unseen: 0, playing: false, anim: null,
});

class MainWindow {
	constructor(host, initial = {}, timing = {}) {
		this.host = host;
		this.timing = timing;
		this.s = { ...defaults(), ...initial };
		host.addEventListener("click", (e) => this.click(e));
		host.addEventListener("dblclick", (e) => this.dblclick(e));
		host.addEventListener("contextmenu", (e) => this.contextMenu(e));
		host.addEventListener("keydown", (e) => this.key(e));
		host.addEventListener("input", (e) => this.input(e));
		host.addEventListener("mousemove", (e) => this.scrub(e));
		this.render();
	}

	set(patch) {
		Object.assign(this.s, patch);
		this.render();
	}

	visible() {
		const { items, type, query, status, sort } = this.s;
		const needle = query.trim().toLowerCase();
		const filters = statusFilters.filter(([id]) => status.includes(id));
		const list = items.filter((item) => {
			if (type !== "all" && item.kind !== type) return false;
			if (needle && !`${item.name} ${item.exports.map((x) => x.file).join(" ")}`.toLowerCase().includes(needle)) return false;
			return filters.length === 0 || filters.some(([, , , test]) => test(item));
		});
		if (sort === "name") return [...list].sort((a, b) => a.name.localeCompare(b.name));
		if (sort === "size") return [...list].sort((a, b) => b.mb - a.mb);
		if (sort === "duration") return [...list].sort((a, b) => seconds(b) - seconds(a));
		return list;
	}

	selectedItems() {
		return this.s.items.filter((item) => this.s.selected.includes(item.id));
	}

	titlebar() {
		const { expanded } = this.s;
		return `<div class="titlebar"><span class="mark"></span><span>FrameEver</span><span class="spacer"></span>
			<button class="caption" title="${pick("Minimize", "最小化")}">${captionGlyph.minimize}</button>
			${expanded ? `<button class="caption" title="${pick("Maximize", "最大化")}">${captionGlyph.maximize}</button>` : ""}
			<button class="caption close" data-a="close" title="${pick("Hide to tray", "隐藏到托盘")}">${captionGlyph.close}</button></div>`;
	}

	recording() {
		const { mode, target, rail, expanded, items, unseen } = this.s;
		const shot = mode === "screenshot";
		const startLabel = shot ? L("Take screenshot", "截图") : L("Start recording", "开始录制");
		if (rail && expanded) {
			return `<div class="rec rail"><div class="rec-main"><div class="mode-rail">${Object.entries(KIND).map(([id, k]) => `<button class="${mode === id ? "on" : ""}" data-a="mode" data-v="${id}" title="${pick(...k.name)}">${icon(k.icon)}</button>`).join("")}</div><span style="flex:1"></span><button class="btn primary icon record-round" data-a="start" title="${pick("Start recording", "开始录制")}">${icon("circle", "solid")}</button></div>
				<div class="rec-foot"><button class="btn ghost icon" data-a="rail" title="${pick("Show recording column", "展开录制列")}  Ctrl+B">${icon("panel-left")}</button><button class="btn ghost icon" data-a="settings" title="${pick("Settings", "设置")}">${icon("settings")}</button></div></div>`;
		}
		const targets = [["display", "monitor", "Display", "显示器"], ["window", "app-window", "Window", "窗口"], ["area", "box-select", "Area", "区域"]];
		const foot = expanded
			? `<button class="btn ghost" data-a="collapse">${icon("chevron-left")}${L("Compact", "收回")}</button><span class="grow"></span><button class="btn ghost icon" data-a="rail" title="${pick("Hide recording column", "折叠录制列")}  Ctrl+B">${icon("panel-left")}</button><button class="btn ghost icon" data-a="settings" title="${pick("Settings", "设置")}">${icon("settings")}</button>`
			: `<button class="btn ghost icon" data-a="settings" title="${pick("Settings", "设置")}">${icon("settings")}</button><span class="grow tnum">${L(`${items.length} items`, `${items.length} 项`)}</span><button class="btn ghost" data-a="expand">${L("Library", "资源库")}${unseen ? `<span class="badge klein">${unseen}</span>` : ""}${icon("chevron-right")}</button>`;
		return `<div class="rec"><div class="rec-main">
			<div class="seg block">${Object.entries(KIND).map(([id, k]) => `<button class="${mode === id ? "on" : ""}" data-a="mode" data-v="${id}">${icon(k.icon, "sm")}${L(...k.name)}</button>`).join("")}</div>
			<div class="targets">${targets.map(([id, ic, en, zh]) => `<button class="${target === id ? "on" : ""}" data-a="target" data-v="${id}">${icon(ic, "lg")}${L(en, zh)}</button>`).join("")}</div>
			<div class="device">${icon("video")}<span class="grow">${L("Camera", "摄像头")}</span><em>${L("No camera", "无摄像头")}</em>${icon("chevron-down", "sm")}</div>
			<div class="device">${icon("mic")}<span class="grow">${L("Microphone", "麦克风")}</span><em>${L("Default", "默认")}</em>${icon("chevron-down", "sm")}</div>
			<div class="device">${icon("volume-2")}<span class="grow">${L("System audio", "系统声音")}</span><button class="switch on" data-a="toggle"></button></div>
			<span style="flex:1"></span>
			<button class="btn primary block tall" data-a="start">${icon("circle", "solid")}${startLabel}</button>
		</div><div class="rec-foot">${foot}</div></div>`;
	}

	toolbar() {
		const { type, query, status, sort, view, menu, items } = this.s;
		const count = (kind) => (kind === "all" ? items.length : items.filter((i) => i.kind === kind).length);
		const tabs = [["all", "All", "全部"], ...Object.entries(KIND).map(([id, k]) => [id, ...k.tab])];
		const statusMenu = menu === "status" ? `<div class="menu">${statusFilters.map(([id, en, zh, test]) => `<button data-a="status" data-v="${id}"><span class="check ${status.includes(id) ? "on" : ""}">${icon("check", "sm")}</span><span class="grow">${L(en, zh)}</span><span class="tnum">${items.filter(test).length}</span></button>`).join("")}</div>` : "";
		const sortMenu = menu === "sort" ? `<div class="menu">${sorts.map(([id, en, zh]) => `<button data-a="sort" data-v="${id}"><span class="grow">${L(en, zh)}</span>${sort === id ? icon("check", "sm") : ""}</button>`).join("")}</div>` : "";
		return `<div class="toolbar">
			<div class="seg">${tabs.map(([id, en, zh]) => `<button class="${type === id ? "on" : ""}" data-a="type" data-v="${id}">${L(en, zh)} <em class="tnum">${count(id)}</em></button>`).join("")}</div>
			<div class="pop-anchor"><button class="btn ghost" data-a="menu" data-v="status">${icon("list-filter")}${L("Status", "状态")}${status.length ? `<span class="badge klein">${status.length}</span>` : ""}</button>${statusMenu}</div>
			<span class="spacer"></span>
			<label class="input">${icon("search")}<input data-keep="search" value="${query}" placeholder="${pick("Search", "搜索")}" data-ph-en="Search" data-ph-zh="搜索"></label>
			<div class="pop-anchor end"><button class="btn ghost icon" data-a="menu" data-v="sort" title="${pick("Sort", "排序")}">${icon("arrow-down-wide-narrow")}</button>${sortMenu}</div>
			<div class="seg"><button class="${view === "grid" ? "on" : ""}" data-a="view" data-v="grid" title="${pick("Grid", "网格")}  Ctrl+1">${icon("layout-grid", "sm")}</button><button class="${view === "list" ? "on" : ""}" data-a="view" data-v="list" title="${pick("List", "列表")}  Ctrl+2">${icon("list", "sm")}</button></div>
		</div>`;
	}

	filterRow() {
		const { status } = this.s;
		if (status.length === 0) return "";
		return `<div class="filters">${statusFilters.filter(([id]) => status.includes(id)).map(([id, en, zh]) => `<button class="tag on" data-a="status" data-v="${id}">${L(en, zh)}${icon("x", "sm")}</button>`).join("")}<button class="clear" data-a="clear-filters">${L("Clear filters", "清除筛选")}</button></div>`;
	}

	tile(item) {
		const on = this.s.selected.includes(item.id);
		const k = KIND[item.kind];
		return `<div class="tile ${on ? "on" : ""} ${item.fresh ? "fresh" : ""}" data-a="item" data-id="${item.id}">
			<span class="thumb">${art(item.art)}<span class="corner tl">${icon(k.icon, "sm")}</span>${item.exports.length ? `<span class="corner tr" title="${pick("Exported", "已导出")}">${icon("check", "sm")}</span>` : ""}${item.dur ? `<span class="corner br tnum">${item.dur}</span>` : ""}${item.kind === "screenshot" ? "" : '<span class="scrub"></span>'}</span>
			<span class="name">${item.name}</span>
			<span class="meta tnum">${L(`${k.name[0]} · ${item.size}`, `${k.name[1]} · ${item.size}`)}</span>
		</div>`;
	}

	row(item) {
		const k = KIND[item.kind];
		return `<div class="table-row ${this.s.selected.includes(item.id) ? "on" : ""}" data-a="item" data-id="${item.id}"><span class="name">${icon(k.icon)}<span class="text">${item.name}</span>${item.exports.length ? icon("check", "sm") : ""}</span><span class="dim">${L(...k.name)}</span><span class="dim tnum">${item.dur ?? "—"}</span><span class="dim tnum">${item.size}</span><span class="dim tnum">${L(...DAY[item.group])}</span></div>`;
	}

	items() {
		const { view, sort, items, query, status, type } = this.s;
		const list = this.visible();
		if (items.length === 0) return `<div class="empty"><span class="empty-mark"></span><div>${L("Recordings you make appear here.", "你录制的内容会显示在这里。")}</div><button class="btn primary" data-a="start">${L("Start recording", "开始录制")}</button></div>`;
		if (list.length === 0 && (query || status.length || type !== "all")) return `<div class="empty"><div>${L("No matching items", "没有匹配的条目")}</div><button class="btn" data-a="clear-filters">${L("Clear filters", "清除筛选")}</button></div>`;
		if (view === "list") {
			return `<div class="table"><div class="table-head"><span>${L("Name", "名称")}</span><span>${L("Type", "类型")}</span><span>${L("Duration", "时长")}</span><span>${L("Size", "大小")}</span><span>${L("Date", "日期")}</span></div>${list.map((item) => this.row(item)).join("")}</div>`;
		}
		if (sort !== "date") return `<div class="group-title"></div><div class="grid">${list.map((item) => this.tile(item)).join("")}</div>`;
		return Object.keys(GROUP)
			.filter((g) => list.some((item) => item.group === g))
			.map((g) => `<div class="group-title">${L(...GROUP[g])}</div><div class="grid">${list.filter((item) => item.group === g).map((item) => this.tile(item)).join("")}</div>`)
			.join("");
	}

	statusBar() {
		const { tile, view, items } = this.s;
		const list = this.visible();
		const total = list.reduce((sum, item) => sum + item.mb, 0);
		const size = total >= 1000 ? `${(total / 1000).toFixed(1)} GB` : `${Math.round(total)} MB`;
		const pct = ((tile - 120) / 160) * 100;
		const slider = view === "grid" && items.length ? `<div class="slider" data-a="tile" title="Ctrl + ${pick("scroll", "滚轮")}"><i></i><b style="width:${pct}%"></b><u style="left:${pct}%"></u></div>` : "";
		return `<div class="status-bar tnum"><span>${L(`${list.length} items · ${size}`, `${list.length} 项 · ${size}`)}</span><span class="spacer"></span>${slider}</div>`;
	}

	detail() {
		const chosen = this.selectedItems();
		if (chosen.length === 0) return "";
		return chosen.length > 1 ? this.batch(chosen) : this.single(chosen[0]);
	}

	single(item) {
		const k = KIND[item.kind];
		const video = item.kind !== "screenshot";
		const unedited = item.kind === "studio" && item.exports.length === 0;
		const primary = {
			studio: [["open", "Open in editor", "在编辑器中打开"], ["export", "Export", "导出"]],
			screenshot: [["open", "Open in editor", "在编辑器中打开"], ["copy", "Copy image", "复制图片"]],
			instant: [["export", "Export", "导出"], ["compress", "Compress", "压缩"]],
		}[item.kind];
		const info = [
			[["Type", "类型"], L(...k.name)],
			[["Created", "时间"], `${L(...DAY[item.group])} ${item.time}`],
			item.dur && [["Duration", "时长"], item.dur],
			[["Resolution", "分辨率"], item.fps ? `${item.res} · ${item.fps} fps` : item.res],
			[["Size", "大小"], item.size],
			item.tracks && [["Tracks", "轨道"], L(...item.tracks)],
		].filter(Boolean);
		const versions = item.compressed ? `<div><div class="subhead">${L("Version", "版本")}</div><div class="seg block"><button class="${item.compressedOnly ? "" : "on"}">${L("Original", "原始")} <em class="tnum">${item.size}</em></button><button class="${item.compressedOnly ? "on" : ""}">${L("Compressed", "压缩")} <em class="tnum">${item.compressed.size}</em></button></div><div class="note" style="margin-top:6px">${L(`Saves ${item.compressed.saved}`, `节省 ${item.compressed.saved}`)}</div></div>` : "";
		const exports = item.exports.length ? `<div><div class="subhead">${L("Exports", "导出")}</div>${item.exports.map((x) => `<div class="export-row ${x.gone ? "gone" : ""}"><span class="grow"><b>${x.file}</b><small class="tnum">${x.gone ? L("Not found", "找不到") : L(`${x.time[0]} · ${x.size}`, `${x.time[1]} · ${x.size}`)}</small></span><span class="row-actions">${x.gone ? `<button class="btn ghost sq" title="${pick("Remove from list", "从列表移除")}">${icon("x", "sm")}</button>` : `<button class="btn ghost sq" title="${pick("Show in folder", "在文件夹中显示")}">${icon("folder-open", "sm")}</button><button class="btn ghost sq" title="${pick("Copy", "复制")}">${icon("copy", "sm")}</button>`}</span></div>`).join("")}</div>` : "";
		return `<aside class="detail"><div class="detail-scroll">
			<div class="player"><span class="thumb" data-a="preview">${art(item.art)}${unedited ? `<span class="corner tl">${L("Unedited", "未编辑")}</span>` : ""}</span>${video ? `<div class="player-bar tnum"><button class="play-btn" data-a="play">${icon(this.s.playing ? "pause" : "play", "sm")}</button><div class="slider"><i></i><b style="width:24%"></b><u style="left:24%"></u></div><span>0:53 / ${item.dur}</span><button class="play-btn">${icon("volume-2", "sm")}</button></div>` : ""}</div>
			<input class="name-edit" value="${item.name}" aria-label="${pick("Name", "名称")}">
			<div class="actions">${primary.map(([, en, zh], i) => `<button class="btn ${i === 0 ? "primary" : ""}">${L(en, zh)}</button>`).join("")}</div>
			<dl class="info tnum">${info.map(([label, val]) => `<dt>${L(...label)}</dt><dd>${val}</dd>`).join("")}</dl>
			${versions}${exports}
		</div><div class="detail-foot"><button class="btn ghost">${L("Show in folder", "在文件夹中显示")}</button><button class="btn ghost">${L("Copy file", "复制文件")}</button><button class="btn ghost del" data-a="delete">${L("Delete…", "删除…")}</button></div></aside>`;
	}

	batch(chosen) {
		const total = chosen.reduce((sum, item) => sum + item.mb, 0);
		const kinds = Object.keys(KIND).map((id) => [id, chosen.filter((i) => i.kind === id).length]).filter(([, n]) => n);
		const compressible = chosen.filter((i) => i.kind === "instant" && !i.compressed).length;
		return `<aside class="detail"><div class="detail-scroll">
			<div><div class="batch-title tnum">${L(`${chosen.length} selected`, `已选 ${chosen.length} 项`)}</div><div class="note" style="margin-top:2px">${total >= 1000 ? (total / 1000).toFixed(1) + " GB" : Math.round(total) + " MB"}</div></div>
			<dl class="info tnum">${kinds.map(([id, n]) => `<dt>${L(...KIND[id].name)}</dt><dd>${n}</dd>`).join("")}</dl>
			<div class="batch-actions">
				<button class="btn" ${compressible ? "" : "disabled"}>${icon("download")}${L(`Compress ${compressible}`, `压缩 ${compressible} 项`)}</button>
				<button class="btn">${icon("folder-open")}${L("Show in folder", "在文件夹中显示")}</button>
				<button class="btn">${icon("copy")}${L("Copy files", "复制文件")}</button>
				<button class="btn" data-a="delete">${icon("trash")}${L("Delete…", "删除…")}</button>
			</div>
		</div><div class="detail-foot"><button class="btn ghost" data-a="clear-selection">${L("Clear selection", "取消选择")}</button></div></aside>`;
	}

	library() {
		const { anim } = this.s;
		return `<div class="library ${anim ?? ""}">${this.toolbar()}${this.filterRow()}<div class="lib-body"><div class="lib-main"><div class="items" tabindex="-1" style="--tile:${this.s.tile}px">${this.items()}</div>${this.statusBar()}</div>${this.detail()}</div></div>`;
	}

	preview() {
		const item = this.selectedItems()[0];
		if (!this.s.preview || !item) return "";
		const k = KIND[item.kind];
		const video = item.kind !== "screenshot";
		return `<div class="preview"><div class="preview-top"><span class="grow"><b>${item.name}</b><span class="dim tnum">${L(...k.name)}${item.dur ? ` · ${item.dur}` : ""} · ${item.size}</span></span><button class="btn">${L("Open in editor", "在编辑器中打开")}</button><button class="btn">${L("Export", "导出")}</button><button class="btn">${L("Show in folder", "在文件夹中显示")}</button><button class="btn ghost icon" data-a="close-preview" title="Esc">${icon("x")}</button></div>
			<div class="preview-media"><button class="btn ghost icon" data-a="step" data-v="-1">${icon("chevron-left")}</button><span class="thumb">${art(item.art)}</span><button class="btn ghost icon" data-a="step" data-v="1">${icon("chevron-right")}</button></div>
			<div class="preview-foot tnum">${video ? `<button class="play-btn" data-a="play">${icon(this.s.playing ? "pause" : "play")}</button><span>0:53 / ${item.dur}</span><div class="slider"><i></i><b style="width:24%"></b><u style="left:24%"></u></div><button class="play-btn">${icon("volume-2")}</button>` : `<span style="flex:1"></span>`}<button class="play-btn" title="F">${icon("maximize")}</button></div></div>`;
	}

	commands() {
		const { expanded, recording, items, view } = this.s;
		if (recording) {
			return [
				{ group: ["Recording", "录制"], rows: [
					{ icon: "square", label: ["Stop recording", "停止录制"] },
					{ icon: "pause", label: ["Pause", "暂停"] },
					{ icon: "rotate-ccw", label: ["Restart", "重新开始"] },
				] },
			];
		}
		const commands = [
			{ icon: "circle", label: ["Start recording", "开始录制"] },
			{ icon: "screenshot", label: ["Take screenshot", "截图"] },
			expanded
				? { icon: "chevron-left", label: ["Collapse to compact", "收回到紧凑模式"], keys: ["Ctrl", "L"], act: "collapse" }
				: { icon: "chevron-right", label: ["Show library", "显示资源库"], keys: ["Ctrl", "L"], act: "expand" },
			...(expanded
				? [
						{ icon: "download", label: ["Export…", "导出…"] },
						{ icon: "layout-grid", label: ["Grid view", "网格视图"], keys: ["Ctrl", "1"], act: "grid" },
						{ icon: "list", label: ["List view", "列表视图"], keys: ["Ctrl", "2"], act: "list" },
						{ icon: "panel-left", label: ["Toggle recording column", "切换录制列"], keys: ["Ctrl", "B"], act: "rail" },
						{ icon: "refresh-cw", label: ["Refresh library", "刷新资源库"] },
					]
				: []),
			{ icon: "folder-open", label: ["Open storage folder", "打开存储文件夹"] },
			{ icon: "settings", label: ["Open settings", "打开设置"] },
		];
		const settings = [
			{ icon: "sliders-horizontal", label: ["Recording quality", "录制画质"], sub: ["Recording", "录制"] },
			{ icon: "sliders-horizontal", label: ["Open library after recording", "录完打开资源库"], sub: ["General", "通用"] },
			{ icon: "keyboard", label: ["Shortcuts", "快捷键"], sub: ["Shortcuts", "快捷键"] },
			{ icon: "folder", label: ["Storage folder", "存储文件夹"], sub: ["General", "通用"] },
			{ icon: "folder", label: ["Export folder", "导出文件夹"], sub: ["General", "通用"] },
			{ icon: "type", label: ["Language", "语言"], sub: ["General", "通用"] },
		];
		const projects = items.slice(0, 6).map((item) => ({ icon: KIND[item.kind].icon, label: [item.name, item.name], sub: KIND[item.kind].name, project: item.id }));
		return { commands, settings, projects, view };
	}

	paletteRows() {
		const { palette, recording } = this.s;
		const data = this.commands();
		if (recording) return data;
		const q = palette.q.trim().toLowerCase();
		const match = (row) => !q || row.label[0].toLowerCase().includes(q) || row.label[1].includes(q);
		const scope = palette.scope;
		if (!q && !scope) {
			return [
				{ group: ["Recent commands", "最近的命令"], rows: data.commands.slice(0, 3) },
				{ group: ["Recent projects", "最近的项目"], rows: data.projects.slice(0, 3) },
			];
		}
		const groups = [
			{ key: "command", group: ["Commands", "命令"], rows: data.commands.filter(match) },
			{ key: "project", group: ["Projects", "项目"], rows: data.projects.filter(match) },
			{ key: "setting", group: ["Settings", "设置"], rows: data.settings.filter(match) },
		];
		return groups.filter((g) => (!scope || g.key === scope) && g.rows.length);
	}

	palette() {
		const { palette, recording } = this.s;
		if (!palette) return "";
		const groups = this.paletteRows();
		const flat = groups.flatMap((g) => g.rows);
		const index = Math.min(palette.index, Math.max(0, flat.length - 1));
		let n = -1;
		const list = groups.map((g) => `<div class="palette-group">${L(...g.group)}</div>${g.rows.map((row) => {
			n += 1;
			return `<button class="palette-item ${n === index ? "on" : ""}" data-a="palette-run" data-v="${n}">${icon(row.icon)}<span class="grow">${L(...row.label)}${row.sub ? `<span class="sub">${L(...row.sub)}</span>` : ""}</span>${row.keys ? kbd(...row.keys) : ""}</button>`;
		}).join("")}`).join("");
		const scopeName = { command: ["Commands", "命令"], project: ["Projects", "项目"], setting: ["Settings", "设置"] }[palette.scope];
		const selected = flat[index];
		const hints = selected?.project
			? `<span>${kbd("Enter")}${L("Open", "打开")}</span><span>${kbd("Shift", "Enter")}${L("Show in library", "在资源库中显示")}</span>`
			: `<span>${kbd("Enter")}${L("Run", "执行")}</span>`;
		const prefixes = palette.q || palette.scope || recording ? "" : `<span style="margin-left:auto">${kbd(">")}${L("Commands", "命令")}</span><span>${kbd("@")}${L("Projects", "项目")}</span><span>${kbd("$")}${L("Settings", "设置")}</span>`;
		return `<div class="scrim" data-a="palette-close"></div><div class="palette ${palette.fresh ? "opening" : ""}"><div class="palette-input">${icon("search")}${scopeName ? `<span class="tag on">${L(...scopeName)}</span>` : ""}<input data-keep="palette" value="${palette.q}" placeholder="${pick("Search commands, projects and settings", "搜索命令、项目和设置")}" data-ph-en="Search commands, projects and settings" data-ph-zh="搜索命令、项目和设置"></div><div class="palette-list">${list || `<div class="palette-empty">${L("No results", "没有结果")}</div>`}</div><div class="palette-foot">${flat.length ? hints : ""}${prefixes}</div></div>`;
	}

	dialog() {
		const { dialog } = this.s;
		if (!dialog) return "";
		const items = this.selectedItems();
		const one = items.length === 1 ? items[0] : null;
		return `<div class="scrim center">${deleteDialog(one, items.length, dialog.choice ?? 0, true)}</div>`;
	}

	toast() {
		const { toast } = this.s;
		if (!toast) return "";
		return `<div class="win-toast toast"><span class="grow">${toast}</span><button class="btn ghost" data-a="undo">${L("Undo", "撤销")}</button></div>`;
	}

	contextOverlay() {
		const { context } = this.s;
		if (!context) return "";
		const style = `left:${context.x}px;top:${context.y}px`;
		if (context.blank) {
			return `<div class="menu context" style="${style}"><button><span class="grow">${L("Sort by", "排序方式")}</span>${icon("chevron-right", "sm")}</button><button><span class="grow">${L("View", "视图")}</span>${icon("chevron-right", "sm")}</button><button>${L("Refresh", "刷新")}</button><hr><button>${L("Open storage folder", "打开存储文件夹")}</button></div>`;
		}
		const many = this.s.selected.length > 1;
		if (many) return `<div class="menu context" style="${style}"><button>${L("Compress", "压缩")}</button><button>${L("Copy files", "复制文件")}</button><button>${L("Show in folder", "在文件夹中显示")}</button><hr><button class="danger" data-a="delete">${L("Delete…", "删除…")}</button></div>`;
		return `<div class="menu context" style="${style}"><button><span class="grow">${L("Open", "打开")}</span>${kbd("Enter")}</button><button data-a="preview"><span class="grow">${L("Preview", "预览")}</span>${kbd("Space")}</button><hr><button>${L("Export…", "导出…")}</button><button>${L("Compress", "压缩")}</button><hr><button><span class="grow">${L("Rename", "重命名")}</span>${kbd("F2")}</button><button><span class="grow">${L("Copy file", "复制文件")}</span>${kbd("Ctrl", "C")}</button><button>${L("Show in folder", "在文件夹中显示")}</button><hr><button class="danger" data-a="delete"><span class="grow">${L("Delete…", "删除…")}</span>${kbd("Del")}</button></div>`;
	}

	render() {
		const { expanded } = this.s;
		const keep = document.activeElement?.dataset?.keep;
		const had = this.host.contains(document.activeElement);
		const scroll = this.host.querySelector(".items")?.scrollTop ?? 0;
		const times = [];
		if (this.timing.expand) times.push(`--expand-time:${this.timing.expand}ms`);
		if (this.timing.collapse) times.push(`--collapse-time:${this.timing.collapse}ms`);
		this.host.innerHTML = `<div class="win ${expanded ? "expanded" : ""}" tabindex="0" style="${times.join(";")}">${this.titlebar()}<div class="win-body">${this.recording()}${expanded ? this.library() : ""}</div>${this.preview()}${this.palette()}${this.dialog()}${this.toast()}${this.contextOverlay()}</div>`;
		this.s.anim = null;
		if (this.s.palette) this.s.palette.fresh = false;
		const items = this.host.querySelector(".items");
		if (items) items.scrollTop = scroll;
		if (keep) {
			const input = this.host.querySelector(`[data-keep="${keep}"]`);
			input?.focus();
			input?.setSelectionRange(input.value.length, input.value.length);
		} else if (had && this.s.palette) {
			this.host.querySelector('[data-keep="palette"]')?.focus();
		} else if (had) {
			this.host.querySelector(".win")?.focus();
		}
	}

	expand() {
		this.set({ expanded: true, unseen: 0, anim: this.timing.calm ? "calm-in" : "in", palette: null });
	}

	collapse() {
		const library = this.host.querySelector(".library");
		this.s.palette = null;
		if (!library) return;
		library.classList.add("out");
		setTimeout(() => this.set({ expanded: false, selected: [], preview: false }), this.timing.collapse ?? tokens.motion.duration.collapse);
	}

	choose(id, event) {
		const list = this.visible().map((item) => item.id);
		const { selected, anchor } = this.s;
		if (event?.shiftKey && anchor !== null) {
			const [a, b] = [list.indexOf(anchor), list.indexOf(id)].sort((x, y) => x - y);
			this.set({ selected: list.slice(a, b + 1), context: null, menu: null });
		} else if (event?.ctrlKey) {
			this.set({ selected: selected.includes(id) ? selected.filter((x) => x !== id) : [...selected, id], anchor: id, context: null, menu: null });
		} else {
			this.set({ selected: [id], anchor: id, context: null, menu: null, playing: false });
		}
	}

	step(by) {
		const list = this.visible().map((item) => item.id);
		if (list.length === 0) return;
		const at = list.indexOf(this.s.selected[this.s.selected.length - 1]);
		const next = list[Math.max(0, Math.min(list.length - 1, at + by))] ?? list[0];
		this.set({ selected: [next], anchor: next });
		this.host.querySelector(".tile.on, .table-row.on")?.scrollIntoView({ block: "nearest" });
	}

	remove() {
		const gone = this.selectedItems();
		const first = gone[0];
		const text = gone.length === 1 ? L(`Deleted “${first.name}”`, `已删除“${first.name}”`) : L(`Deleted ${gone.length} items`, `已删除 ${gone.length} 项`);
		this.backup = this.s.items;
		this.set({ items: this.s.items.filter((item) => !gone.includes(item)), selected: [], dialog: null, toast: text, context: null });
		clearTimeout(this.toastTimer);
		this.toastTimer = setTimeout(() => this.set({ toast: null }), 8000);
	}

	click(e) {
		const el = e.target.closest("[data-a]");
		const a = el?.dataset.a;
		const v = el?.dataset.v;
		const close = { context: null };
		if (this.s.menu && a !== "menu" && a !== "status" && a !== "sort") close.menu = null;
		switch (a) {
			case "mode": return this.set({ mode: v, ...close });
			case "target": return this.set({ target: v, ...close });
			case "toggle": el.classList.toggle("on"); return;
			case "expand": return this.expand();
			case "collapse": return this.collapse();
			case "rail": return this.set({ rail: !this.s.rail, ...close });
			case "type": return this.set({ type: v, ...close });
			case "view": return this.set({ view: v, ...close });
			case "menu": return this.set({ menu: this.s.menu === v ? null : v, context: null });
			case "status": {
				const { status } = this.s;
				return this.set({ status: status.includes(v) ? status.filter((x) => x !== v) : [...status, v] });
			}
			case "sort": return this.set({ sort: v, menu: null });
			case "clear-filters": return this.set({ status: [], query: "", type: "all", ...close });
			case "clear-selection": return this.set({ selected: [], ...close });
			case "item": return this.choose(Number(el.dataset.id), e);
			case "preview": return this.set({ preview: true, ...close });
			case "close-preview": return this.set({ preview: false });
			case "step": return this.step(Number(v));
			case "play": return this.set({ playing: !this.s.playing });
			case "delete": return this.set({ dialog: { choice: 0 }, ...close });
			case "dialog-choice": return this.set({ dialog: { choice: Number(v) } });
			case "dialog-cancel": return this.set({ dialog: null });
			case "dialog-confirm": return this.remove();
			case "undo": return this.set({ items: this.backup, toast: null });
			case "palette-close": return this.set({ palette: null });
			case "palette-run": return this.run(Number(v), e);
			case "tile": {
				const box = el.getBoundingClientRect();
				const ratio = (e.clientX - box.left) / box.width;
				return this.set({ tile: Math.round(120 + Math.min(1, Math.max(0, ratio)) * 160) });
			}
			default:
				if (e.target.closest(".items") && !e.target.closest("[data-a]")) return this.set({ selected: [], ...close });
				if (close.menu === null || this.s.context) this.set(close);
		}
	}

	dblclick(e) {
		if (e.target.closest("[data-a=item]")) this.set({ preview: true });
	}

	contextMenu(e) {
		const inItems = e.target.closest(".items");
		if (!inItems) return;
		e.preventDefault();
		const win = this.host.querySelector(".win");
		const box = win.getBoundingClientRect();
		const zoom = box.width / win.offsetWidth;
		const x = Math.min((e.clientX - box.left) / zoom, win.offsetWidth - 260);
		const y = Math.min((e.clientY - box.top) / zoom, win.offsetHeight - 300);
		const item = e.target.closest("[data-a=item]");
		if (item) {
			const id = Number(item.dataset.id);
			const selected = this.s.selected.includes(id) ? this.s.selected : [id];
			this.set({ selected, anchor: id, context: { x, y } });
		} else {
			this.set({ context: { x, y, blank: true } });
		}
	}

	run(index, e) {
		const groups = this.paletteRows();
		const row = groups.flatMap((g) => g.rows)[index];
		if (!row) return;
		if (row.project) {
			this.set({ palette: null, expanded: true, selected: [row.project], anchor: row.project, });
			return;
		}
		if (row.act === "expand") return this.expand();
		if (row.act === "collapse") return this.collapse();
		if (row.act === "grid" || row.act === "list") return this.set({ view: row.act, palette: null });
		if (row.act === "rail") return this.set({ rail: !this.s.rail, palette: null });
		this.set({ palette: null });
	}

	input(e) {
		const keep = e.target.dataset.keep;
		if (keep === "search") this.set({ query: e.target.value });
		if (keep === "palette") {
			const typed = e.target.value;
			const scope = { ">": "command", "@": "project", $: "setting" }[typed];
			if (scope && !this.s.palette.scope) this.set({ palette: { q: "", scope, index: 0 } });
			else this.set({ palette: { ...this.s.palette, q: typed, index: 0 } });
		}
	}

	scrub(e) {
		const tile = e.target.closest(".tile");
		const bar = tile?.querySelector(".scrub");
		if (!bar) return;
		const box = tile.querySelector(".thumb").getBoundingClientRect();
		bar.style.right = `${(1 - Math.min(1, Math.max(0, (e.clientX - box.left) / box.width))) * 100}%`;
	}

	key(e) {
		const s = this.s;
		const typing = e.target.tagName === "INPUT";
		const ctrl = e.ctrlKey || e.metaKey;
		if (ctrl && e.key.toLowerCase() === "k") {
			e.preventDefault();
			return this.set({ palette: s.palette ? null : { q: "", scope: null, index: 0, fresh: true }, context: null, menu: null });
		}
		if (s.palette) {
			const count = this.paletteRows().flatMap((g) => g.rows).length;
			if (e.key === "Escape") return this.set({ palette: null });
			if (e.key === "ArrowDown") {
				e.preventDefault();
				return this.set({ palette: { ...s.palette, index: (s.palette.index + 1) % Math.max(1, count) } });
			}
			if (e.key === "ArrowUp") {
				e.preventDefault();
				return this.set({ palette: { ...s.palette, index: (s.palette.index - 1 + count) % Math.max(1, count) } });
			}
			if (e.key === "Enter") return this.run(s.palette.index, e);
			if (e.key === "Backspace" && !s.palette.q && s.palette.scope) return this.set({ palette: { ...s.palette, scope: null } });
			return;
		}
		if (ctrl && e.key.toLowerCase() === "l") {
			e.preventDefault();
			return s.expanded ? this.collapse() : this.expand();
		}
		if (!s.expanded) return;
		if (ctrl && e.key.toLowerCase() === "b") {
			e.preventDefault();
			return this.set({ rail: !s.rail });
		}
		if (ctrl && e.key === "1") {
			e.preventDefault();
			return this.set({ view: "grid" });
		}
		if (ctrl && e.key === "2") {
			e.preventDefault();
			return this.set({ view: "list" });
		}
		if (ctrl && e.key.toLowerCase() === "f") {
			e.preventDefault();
			return this.host.querySelector('[data-keep="search"]')?.focus();
		}
		if (e.key === "Escape") {
			if (s.dialog) return this.set({ dialog: null });
			if (s.preview) return this.set({ preview: false });
			if (s.query) return this.set({ query: "" });
			if (typing) return e.target.blur();
			return this.set({ selected: [], context: null, menu: null });
		}
		if (typing) return;
		if (ctrl && e.key.toLowerCase() === "a") {
			e.preventDefault();
			return this.set({ selected: this.visible().map((item) => item.id) });
		}
		if (e.key === " ") {
			e.preventDefault();
			if (s.selected.length) this.set({ preview: !s.preview });
		}
		if (s.preview && e.key === "ArrowLeft") return this.step(-1);
		if (s.preview && e.key === "ArrowRight") return this.step(1);
		if (s.preview && e.key.toLowerCase() === "k") return this.set({ playing: !s.playing });
		if (s.preview) return;
		if (["ArrowRight", "ArrowDown"].includes(e.key)) {
			e.preventDefault();
			this.step(e.key === "ArrowDown" && s.view === "grid" ? 3 : 1);
		}
		if (["ArrowLeft", "ArrowUp"].includes(e.key)) {
			e.preventDefault();
			this.step(e.key === "ArrowUp" && s.view === "grid" ? -3 : -1);
		}
		if (e.key === "Delete" && s.selected.length) {
			if (e.shiftKey) this.remove();
			else this.set({ dialog: { choice: 0 } });
		}
	}
}

const deleteDialog = (item, count, choice, live) => {
	const footer = (label) => `<div class="actions"><button class="btn" ${live ? 'data-a="dialog-cancel"' : ""}>${L("Cancel", "取消")}</button><button class="btn danger" ${live ? 'data-a="dialog-confirm"' : ""}>${label}</button></div>`;
	const keeps = `<p class="note">${L("Exports stay where they are.", "导出文件会保留在原位置。")}</p>`;
	if (item?.compressed) {
		const options = [
			["Only the compressed copy", "只删压缩副本", "The original stays.", "原始视频会保留。"],
			["Only the original", "只删原始视频", "The compressed copy stays.", "压缩副本会保留。"],
			["The whole project", "删除整个项目", "Original and compressed copy.", "原始视频和压缩副本。"],
		];
		return `<div class="dialog"><h3>${L(`Delete “${item.name}”?`, `删除“${item.name}”？`)}</h3><p>${L("Files go to the Recycle Bin.", "文件会移到回收站。")}</p>${options.map(([a, b, c, d], i) => `<div class="radio ${i === choice ? "on" : ""}" ${live ? `data-a="dialog-choice" data-v="${i}"` : ""}><i></i><div>${L(a, b)}<small>${L(c, d)}</small></div></div>`).join("")}${keeps}${footer(L("Delete", "删除"))}</div>`;
	}
	const title = item ? L(`Delete “${item.name}”?`, `删除“${item.name}”？`) : L(`Delete ${count} items?`, `删除 ${count} 项？`);
	return `<div class="dialog"><h3>${title}</h3><p>${L("Files go to the Recycle Bin.", "文件会移到回收站。")}</p>${keeps}${footer(L("Delete", "删除"))}</div>`;
};

const titles = {
	live: ["Try it", "试一试", "One window, working. It starts compact and only records. Library expands it in place into the recording column plus the library. Click, Ctrl- and Shift-click tiles; arrows, Enter, Space, Delete, Ctrl+K, Ctrl+L, Ctrl+B, Ctrl+1/2, Ctrl+F and Esc all work once the window has focus.", "一个能用的窗口。启动时是紧凑态，只负责录制；点“资源库”原地展开成录制列加资源库。单击、Ctrl 或 Shift 点击图块；窗口获得焦点后，方向键、Enter、Space、Delete、Ctrl+K、Ctrl+L、Ctrl+B、Ctrl+1/2、Ctrl+F 和 Esc 都可以用。"],
	states: ["Library states", "资源库的各种状态", "Each tab opens the expanded window in one state, and each is still interactive. Compressed copies apply to Instant only; exports show inside their project, never as their own tile.", "每个标签打开展开态的一种状态，都可以继续操作。压缩副本只用于 Instant 录制；导出显示在所属项目里，不单独成卡片。"],
	transition: ["Expanding and collapsing", "展开和收回", "The window frame never animates: it snaps once and only the content moves (Fluent's container transform). The top-left corner stays put, so the compact window becomes the recording column and the library grows right and down. Near a screen edge the whole window is pushed back inside the work area with a 12 px margin.", "窗口外框不做动画，只跳一次，动的是内容（Fluent 的容器变形）。左上角不动，紧凑窗口原地变成录制列，资源库往右下长出；超出工作区时整体推回，四边留 12 px。"],
	palette: ["Command palette", "命令面板", "Ctrl+K in the main window, editor and screenshot editor. It finds commands, settings and projects, filtered by context. Prefixes become a chip in the input; Backspace removes it. No blur behind it, and it never expands the compact window.", "主窗口、编辑器、截图编辑器里按 Ctrl+K 打开。查找命令、设置和项目，按上下文过滤。前缀输入后变成输入框里的标签块，退格删除。背后不模糊，紧凑态不触发展开。"],
	delete: ["Delete and undo", "删除和撤销", "Deletes go to the Recycle Bin and show Undo for 8 seconds. Exports are never touched. Delete opens the dialog; Shift+Delete skips it. Only Instant recordings with a compressed copy get the three-way choice.", "删除都进回收站，Toast 带“撤销”，留 8 秒。导出文件永远保留。Delete 打开弹框，Shift+Delete 跳过弹框。只有带压缩副本的 Instant 录制才有三选一。"],
	keys: ["Keyboard", "键盘", "Every action has a keyboard path. Esc unwinds one level at a time inside the window and never collapses it. Closing hides to the tray in both states.", "每个操作都能用键盘完成。Esc 在窗口内逐层退，从不收回窗口。两种状态点关闭都是藏进托盘。"],
};
for (const [key, [en, zh, leadEn, leadZh]] of Object.entries(titles)) {
	document.getElementById(`t-${key}`).innerHTML = L(en, zh);
	document.getElementById(`l-${key}`).innerHTML = L(leadEn, leadZh);
}

shell({ here: "main-window-library", depth: 1, sections: [
	["live", "Try it", "试一试"], ["states", "Library states", "资源库状态"], ["transition", "Expand and collapse", "展开和收回"], ["palette", "Command palette", "命令面板"], ["delete", "Delete", "删除"], ["keys", "Keyboard", "键盘"],
] });

const live = new MainWindow(document.getElementById("live-stage"), { unseen: 2 });

const tabRow = (id, tabs, on) => {
	const box = document.getElementById(id);
	const draw = (active) => {
		box.innerHTML = `<div class="seg">${tabs.map(([key, en, zh]) => `<button class="${key === active ? "on" : ""}" data-tab="${key}">${L(en, zh)}</button>`).join("")}</div>`;
	};
	draw(tabs[0][0]);
	box.addEventListener("click", (e) => {
		const tab = e.target.closest("[data-tab]");
		if (!tab) return;
		draw(tab.dataset.tab);
		on(tab.dataset.tab);
	});
};

const presets = {
	project: { selected: [1], anchor: 1 },
	instant: { selected: [2], anchor: 2 },
	none: {},
	batch: { selected: [1, 2, 5, 7], anchor: 7 },
	list: { view: "list", selected: [4], anchor: 4 },
	filtered: { status: ["exported"], selected: [1], anchor: 1 },
	none_match: { query: "zzz" },
	empty: { items: [] },
	rail: { rail: true, selected: [1], anchor: 1 },
	preview: { selected: [1], anchor: 1, preview: true },
};
const stateHost = document.getElementById("states-stage");
let stateWindow;
const showState = (key) => {
	stateHost.replaceChildren();
	const host = document.createElement("div");
	stateHost.append(host);
	stateWindow = new MainWindow(host, { expanded: true, ...presets[key] });
};
tabRow("state-tabs", [["project", "Studio selected", "选中工作室项目"], ["instant", "Instant with a copy", "带压缩副本的即时录制"], ["none", "Nothing selected", "无选中"], ["batch", "Several selected", "批量"], ["list", "List", "列表"], ["filtered", "Filtered", "筛选"], ["none_match", "No matches", "无匹配"], ["empty", "Empty", "空状态"], ["rail", "Column folded", "录制列折叠"], ["preview", "Space preview", "空格预览"]], showState);
showState("project");

document.getElementById("transition-steps").innerHTML = [
	["Hide content", "先藏内容", "The library isn't drawn yet.", "资源库先不画。"],
	["Snap the frame", "外框跳一次", "One SetWindowPos to 1200 × 760 or the remembered size, on top of nothing else.", "一次 SetWindowPos 到 1200×760 或记住的大小。"],
	["Library arrives", "资源库进入", `${tokens.motion.duration.expand} ms: fade with Fluent decelerate, slide 24 px with the Klein Field move curve. The recording column doesn't move.`, `${tokens.motion.duration.expand} ms：Fluent 减速淡入，Klein Field 移动曲线滑入 24 px。录制列不动。`],
	["Collapse", "收回", `${tokens.motion.duration.collapse} ms fade with Fluent accelerate, then one snap back. With reduced motion, only the fades remain.`, `${tokens.motion.duration.collapse} ms Fluent 加速淡出，再一次缩回。减少动态效果时只保留淡入淡出。`],
].map(([a, b, c, d], i) => `<div class="card"><b>${i + 1} · ${L(a, b)}</b><span class="body">${L(c, d)}</span></div>`).join("");

const transitionHost = document.getElementById("transition-stage");
const transition = { speed: 1, calm: false };
const newTransition = () => {
	transitionHost.replaceChildren();
	const host = document.createElement("div");
	transitionHost.append(host);
	transition.window = new MainWindow(host, {}, {
		expand: tokens.motion.duration.expand * transition.speed,
		collapse: tokens.motion.duration.collapse * transition.speed,
		calm: transition.calm,
	});
};
const controls = document.getElementById("transition-controls");
const drawControls = () => {
	controls.innerHTML = `<button class="btn primary" data-t="go">${L("Expand or collapse", "展开或收回")}</button><div class="seg"><button class="${transition.speed === 1 ? "on" : ""}" data-t="speed" data-v="1">1×</button><button class="${transition.speed === 4 ? "on" : ""}" data-t="speed" data-v="4">${L("4× slower", "慢 4 倍")}</button></div><div class="seg"><button class="${transition.calm ? "" : "on"}" data-t="calm" data-v="0">${L("Full motion", "完整动效")}</button><button class="${transition.calm ? "on" : ""}" data-t="calm" data-v="1">${L("Reduced motion", "减少动态效果")}</button></div>`;
};
controls.addEventListener("click", (e) => {
	const el = e.target.closest("[data-t]");
	if (!el) return;
	if (el.dataset.t === "go") return transition.window.s.expanded ? transition.window.collapse() : transition.window.expand();
	if (el.dataset.t === "speed") transition.speed = Number(el.dataset.v);
	if (el.dataset.t === "calm") transition.calm = el.dataset.v === "1";
	drawControls();
	newTransition();
});
drawControls();
newTransition();

const paletteLeft = document.createElement("div");
paletteLeft.className = "stage";
document.getElementById("palette-pair").append(paletteLeft);
const compactPalette = document.createElement("div");
paletteLeft.append(compactPalette);
new MainWindow(compactPalette, { palette: { q: "", scope: null, index: 0 } });
document.getElementById("palette-pair").insertAdjacentHTML("beforeend", `<div class="card"><h3>${L("How it sits", "位置和行为")}</h3><dl class="info" style="grid-template-columns:96px minmax(0,1fr)">${[
	[["Position", "位置"], ["Centred, a fifth of the way down, at most 640 px wide or window width − 24 px, so compact nearly fills the window's width.", "水平居中，离顶部约 1/5 窗口高；宽度取“窗口宽减 24 px”和 640 px 的较小值，所以紧凑态基本撑满窗口宽。"]],
	[["Open and close", "打开和关闭"], ["150 ms decelerate fade and a small drop; 100 ms to close. Ctrl+K again closes it.", "150 ms 减速淡入并轻微下移，关闭 100 ms。再按一次 Ctrl+K 关闭。"]],
	[["Empty input", "空输入"], ["Up to 5 recent commands and 5 recent projects, with the prefixes in the footer.", "最近用过的命令最多 5 条、最近打开的项目最多 5 个，底部提示前缀。"]],
	[["Results", "结果"], ["Commands run. Settings open the settings window at that row with a brief highlight. Projects: Enter opens, Shift+Enter shows in the library.", "命令直接执行；设置项打开设置窗口，跳到那一行并闪一下高亮；项目按 Enter 打开，Shift+Enter 在资源库中显示。"]],
	[["While recording", "录制中"], ["Only stop, pause and restart.", "只列停止、暂停、重新开始。"]],
	[["Matching", "匹配"], ["Fuzzy. The Chinese UI matches Chinese and English names; no pinyin in 1.0.", "模糊匹配。中文界面同时匹配中文名和英文名；1.0 不做拼音。"]],
].map(([a, b]) => `<dt>${L(...a)}</dt><dd>${L(...b)}</dd>`).join("")}</dl></div>`);

const paletteHost = document.getElementById("palette-stage");
const paletteStates = {
	mixed: { palette: { q: "exp", scope: null, index: 0 } },
	commands: { palette: { q: "", scope: "command", index: 0 } },
	projects: { palette: { q: "", scope: "project", index: 1 } },
	settings: { palette: { q: "", scope: "setting", index: 0 } },
	nothing: { palette: { q: "zzz", scope: null, index: 0 } },
	recording: { palette: { q: "", scope: null, index: 0 }, recording: true },
};
const showPalette = (key) => {
	paletteHost.replaceChildren();
	const host = document.createElement("div");
	paletteHost.append(host);
	new MainWindow(host, { expanded: true, selected: [1], anchor: 1, ...paletteStates[key] });
};
tabRow("palette-tabs", [["mixed", "Mixed search", "混搜"], ["commands", "> Commands", "> 命令"], ["projects", "@ Projects", "@ 项目"], ["settings", "$ Settings", "$ 设置"], ["nothing", "No results", "没有结果"], ["recording", "While recording", "录制中"]], showPalette);
showPalette("mixed");

document.getElementById("delete-grid").innerHTML = [
	[L("One item", "一个条目"), `<div class="static-dialog">${deleteDialog(ITEMS[0], 1, 0, false)}</div>`],
	[L("Instant with a compressed copy", "带压缩副本的即时录制"), `<div class="static-dialog">${deleteDialog(ITEMS[1], 1, 0, false)}</div>`],
	[L("Several items, and the toast", "多个条目和提示"), `<div class="static-dialog">${deleteDialog(null, 4, 0, false)}</div><div class="toast" style="margin-top:12px"><span class="grow">${L("Deleted 4 items", "已删除 4 项")}</span><button class="btn ghost">${L("Undo", "撤销")}</button></div>`],
].map(([title, body]) => `<div><div class="label">${title}</div>${body}</div>`).join("");

const keyRows = (rows) => rows.map(([en, zh, ...keys]) => `<div class="key-row"><span>${L(en, zh)}</span><span class="keys">${keys.map((k) => `<span class="kbd">${k}</span>`).join("")}</span></div>`).join("");
document.getElementById("keys-grid").innerHTML = `<div class="card"><h3>${L("Library", "资源库")}</h3>${keyRows([
	["Move, extend selection", "移动、扩选", "←↑→↓", "Shift ←↑→↓"], ["Open", "打开", "Enter"], ["Preview", "预览", "Space"], ["Rename", "重命名", "F2"], ["Delete…", "删除…", "Del"], ["Delete without asking", "直接删除", "Shift Del"], ["Copy file", "复制文件", "Ctrl C"], ["Select all", "全选", "Ctrl A"], ["Search", "搜索", "Ctrl F"], ["Thumbnail size", "缩略图大小", "Ctrl =", "Ctrl -"], ["Grid, list", "网格、列表", "Ctrl 1", "Ctrl 2"], ["Back out one level", "逐层退出", "Esc"],
])}</div><div class="card"><h3>${L("Window", "窗口")}</h3>${keyRows([
	["Command palette", "命令面板", "Ctrl K"], ["Expand, collapse", "展开、收回", "Ctrl L"], ["Fold the recording column", "折叠录制列", "Ctrl B"],
])}<h3 style="margin-top:16px">${L("Preview", "预览")}</h3>${keyRows([
	["Previous, next", "上一个、下一个", "←", "→"], ["Play, pause", "播放、暂停", "K"], ["Back, forward 5 s", "后退、前进 5 秒", "J", "L"], ["Mute", "静音", "M"], ["Whole screen", "整屏", "F"], ["Close", "关闭", "Space", "Esc"],
])}</div>`;

onChange(() => {
	for (const input of document.querySelectorAll("[data-ph-en]")) input.placeholder = pick(input.dataset.phEn, input.dataset.phZh);
	live.render();
});
