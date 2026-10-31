const { L, icon, pick, shell, onChange } = FE;

const VARIANTS = {
	A: ["One step at a time", "一步一屏"],
	B: ["Sources beside contents", "来源和内容并排"],
	C: ["Every item in a list", "逐项列表"],
};

const DEST = "C:\\Users\\you\\Videos\\FrameEver";
const SOURCES = {
	two: { name: "Cap-aniBullet 2.x", path: "D:\\Cap Recordings", rec: 42, shot: 18, recGb: 36.9, shotGb: 1.3, settings: true, rules: 2, across: true },
	cap: { name: "Cap", path: "C:\\Users\\you\\AppData\\Roaming\\so.cap.desktop", rec: 6, shot: 3, recGb: 4.0, shotGb: 0.1, settings: true, rules: 0, across: false },
	folder: { name: "D:\\Old recordings", path: "D:\\Old recordings", rec: 5, shot: 0, recGb: 2.4, shotGb: 0, settings: false, rules: 0, across: true },
	blank: { name: "D:\\Downloads", path: "D:\\Downloads", rec: 0, shot: 0, recGb: 0, shotGb: 0, settings: false, rules: 0, across: true },
};
const NAMES = ["Product demo", "Bug repro", "Onboarding walkthrough", "Team sync", "Launch recap", "Sales call", "Tutorial", "Design review", "Weekly update", "Support case"];
const LEFT = [
	{ name: "Team sync", kind: "in-use", en: "In use by another program. Close it and try again.", zh: "正被其他程序占用。关闭该程序后再试一次。" },
	{ name: "Launch recap", kind: "failed", en: "Couldn't move this recording. It's still where it was.", zh: "无法移动这个录制。它还在原来的位置。", raw: "The copy of D:\\Cap Recordings\\recordings\\Launch recap.cap does not match the original" },
];

const fresh = () => ({ step: "find", src: null, rec: true, shot: true, settings: true, done: 0, total: 0, stopping: false, stopped: false, detail: false, finished: [], tab: "all", q: "", off: new Set(), popup: false, tried: false });
const s = { variant: "A", found: "two", block: "none", outcome: "all", ...fresh() };
const q = new URLSearchParams(location.search).get("variant");
if (VARIANTS[q]) s.variant = q;

const foundList = () => ({ two: ["two", "cap"], one: ["two"], none: [] })[s.found];
const gb = (n) => `${n.toFixed(1)} GB`;
const countLine = (src) => {
	const rec = L(`${src.rec} recordings`, `${src.rec} 个录制`);
	const shot = L(`${src.shot} screenshots`, `${src.shot} 张截图`);
	return `${rec} · ${shot} · ${gb(src.recGb + src.shotGb)}`;
};
const items = (key) => {
	const src = SOURCES[key];
	const made = [];
	for (let i = 0; i < src.rec; i++) made.push({ id: `r${i}`, kind: "rec", name: i < NAMES.length ? NAMES[i] : `${NAMES[i % NAMES.length]} ${Math.floor(i / NAMES.length) + 1}`, date: `2026-${String(9 - (i % 9)).padStart(2, "0")}-${String(1 + ((i * 7) % 27)).padStart(2, "0")}`, size: 0.3 + ((i * 37) % 19) / 10 });
	for (let i = 0; i < src.shot; i++) made.push({ id: `s${i}`, kind: "shot", name: `Screenshot ${i + 1}`, date: `2026-${String(9 - (i % 9)).padStart(2, "0")}-${String(2 + ((i * 5) % 26)).padStart(2, "0")}`, size: 0.07 });
	const recordings = made.filter((item) => item.kind === "rec");
	const sum = recordings.reduce((total, item) => total + item.size, 0);
	for (const item of recordings) item.size *= src.recGb / sum;
	return made;
};
const selected = () => {
	const src = SOURCES[s.src];
	if (s.variant === "C") {
		const chosen = items(s.src).filter((item) => !s.off.has(item.id));
		return { n: chosen.length, size: chosen.reduce((sum, item) => sum + item.size, 0), settings: src.settings && s.settings };
	}
	return { n: (s.rec ? src.rec : 0) + (s.shot ? src.shot : 0), size: (s.rec ? src.recGb : 0) + (s.shot ? src.shotGb : 0), settings: src.settings && s.settings };
};
const lacksSpace = () => s.block === "space" && SOURCES[s.src].across;

const title = () => `<div class="cap-bar"><span class="logo"></span>FrameEver Converter<span class="grow"></span><span class="caption">${icon("minus")}</span><span class="caption" ${s.step === "moving" ? `title="${pick("Stops after this item", "做完当前一项后停止")}"` : ""}>${icon("x")}</span></div>`;
const check = (key, on) => `<button class="check ${on ? "on" : ""}" data-toggle="${key}" role="checkbox" aria-checked="${on}">${icon("check", "sm")}</button>`;
const warnings = () => {
	if (lacksSpace()) return `<div class="warn">${icon("triangle-alert")}<span class="grow">${L(`Not enough space on C:. This needs ${gb(selected().size)} and 12.1 GB is free. Change the folder or convert less.`, `C: 盘空间不够。需要 ${gb(selected().size)}，只剩 12.1 GB。换个文件夹，或者少转换一些。`)}</span></div>`;
	if (s.block === "running") return `<div class="warn">${icon("triangle-alert")}<span class="grow">${L("Cap is running. Close it first, or the projects it has open stay behind.", "Cap 正在运行。先关闭它，否则它打开着的项目会留在原处。")}</span><button class="btn ghost" data-a="recheck">${L("Check again", "再次检查")}</button></div>`;
	return "";
};
const review = () => {
	const src = SOURCES[s.src];
	const rows = [
		["rec", s.rec, "video", L("Recordings", "录制"), L(".cap projects and loose MP4 exports", ".cap 项目和零散的 MP4 导出"), L(`${src.rec} · ${gb(src.recGb)}`, `${src.rec} 个 · ${gb(src.recGb)}`), src.rec],
		["shot", s.shot, "image", L("Screenshots", "截图"), L("PNG and JPEG, kept with their dates", "PNG 和 JPEG，保留原来的日期"), L(`${src.shot} · ${gb(src.shotGb)}`, `${src.shot} 张 · ${gb(src.shotGb)}`), src.shot],
	].filter((row) => row[6] > 0);
	const settings = src.settings ? `<div class="row">${check("settings", s.settings)}${icon("settings")}<span class="grow"><b>${L("Settings and automations", "设置和自动化")}</b><small>${L("Only what FrameEver doesn't have yet", "只补上 FrameEver 还没有的")}</small></span><span class="num">${src.rules ? L(`${src.rules} rules`, `${src.rules} 条规则`) : ""}</span></div>` : "";
	return `<div class="group">${rows.map(([key, on, ic, name, sub, num]) => `<div class="row ${on ? "" : "off"}">${check(key, on)}${icon(ic)}<span class="grow"><b>${name}</b><small>${sub}</small></span><span class="num">${num}</span></div>`).join("")}${settings}</div>`;
};
const destination = () => {
	const src = SOURCES[s.src];
	return `<div class="group"><div class="row"><span class="grow"><b>${L("Destination", "转换到")}</b><div class="path">${DEST}</div></span><button class="btn" data-a="change">${L("Change…", "更改…")}</button></div></div>
	<p class="note">${src.across ? L("Files are moved to another drive: copied, checked, then the originals are deleted. Afterwards only FrameEver opens them.", "文件会移到另一个磁盘：先复制、再校验，然后删除原文件。之后只有 FrameEver 能打开它们。") : L("Files are moved, not copied. Afterwards only FrameEver opens them.", "文件会被移动而不是复制，之后只有 FrameEver 能打开它们。")}</p>`;
};
const canConvert = () => {
	const pickedSomething = selected().n > 0 || selected().settings;
	return pickedSomething && !lacksSpace();
};
const convertButton = () => {
	const n = selected().n;
	return `<button class="btn primary" data-a="convert" ${canConvert() ? "" : "disabled"}>${n ? L(`Convert ${n} items`, `转换 ${n} 项`) : L("Convert", "转换")}</button>`;
};
const percent = () => (s.total ? Math.round((s.done / s.total) * 100) : 100);
const currentName = () => {
	const list = items(s.src);
	return list[Math.min(s.done, list.length - 1)]?.name ?? "";
};
const moving = () => {
	const src = SOURCES[s.src];
	const left = selected().size * (1 - s.done / Math.max(1, s.total));
	return `<div class="move"><div class="big-pct">${percent()}%</div><div class="progress"><i style="width:${percent()}%"></i></div><div class="line"><span>${s.stopping ? L("Finishing this item…", "正在完成当前一项…") : L(`Moving ${Math.min(s.done + 1, s.total)} of ${s.total}: ${currentName()}`, `正在移动第 ${Math.min(s.done + 1, s.total)} 项，共 ${s.total} 项：${currentName()}`)}</span><span>${src.across ? L(`${gb(left)} left`, `还剩 ${gb(left)}`) : ""}</span></div></div>`;
};
const movingNote = () => `<p class="note">${L("Items not moved yet stay where they are if you stop.", "停止后，还没移动的内容留在原处。")}</p>`;
const result = () => {
	if (s.stopped) return { kind: "stopped", converted: s.done, left: [], settingsFail: false };
	const settingsFail = s.outcome === "settings" && selected().settings;
	const left = s.outcome === "partial" && !s.tried ? LEFT : [];
	return { kind: left.length ? "partial" : "all", converted: s.total - left.length, left, settingsFail };
};
const doodle = () => `<svg width="72" height="62" viewBox="0 0 120 104" aria-hidden="true"><path pathLength="1" d="M 60 12 C 84 12 102 30 102 52 C 102 75 84 92 60 92 C 35 92 18 74 18 52 C 18 31 34 14 56 12.5 C 62 12 68 13 73 15" fill="none" stroke="var(--muted-foreground)" stroke-width="3" stroke-linecap="round"/><path pathLength="1" d="M 38 54 L 54 70 L 84 34" fill="none" stroke="var(--success-text)" stroke-width="5.5" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const leftRows = (r) => `<div class="group">${r.left.map((item) => `<div class="row"><span style="color:var(--amber-text)">${icon("triangle-alert")}</span><span class="grow"><b>${item.name}</b><small style="white-space:normal">${L(item.en, item.zh)}</small>${item.raw ? `<button class="linkish" data-a="detail">${L("Details", "详情")}</button>${s.detail ? `<div class="detail">${item.raw}</div>` : ""}` : ""}</span></div>`).join("")}</div>`;
const settingsBanner = (r) => (r.settingsFail ? `<div class="warn">${icon("triangle-alert")}<span class="grow">${L("Couldn't carry over your settings. Your recordings aren't affected.", "无法带上你的设置。录制不受影响。")} <button class="linkish" data-a="detail">${L("Details", "详情")}</button>${s.detail ? `<div class="detail">Access is denied. (os error 5)</div>` : ""}</span></div>` : "");
const resultHead = (r) => {
	if (r.kind === "stopped") return [L("Conversion stopped", "转换已停止"), L(`${r.converted} of ${s.total} items are now in FrameEver. The rest are where they were.`, `${s.total} 项中的 ${r.converted} 项已进入 FrameEver，其余的还在原处。`)];
	if (r.kind === "partial") return [L("Some items were left behind", "有些内容没有转换"), L(`${r.converted} of ${s.total} items are now in FrameEver.`, `${s.total} 项中的 ${r.converted} 项已进入 FrameEver。`)];
	return [L("Conversion finished", "转换完成"), L(`${s.total} items are now in FrameEver.`, `${s.total} 项内容已进入 FrameEver。`)];
};
const remaining = () => foundList().filter((key) => key !== s.src && !s.finished.includes(key));
const resultButtons = (r) => {
	const open = `<button class="btn ghost" data-a="open">${icon("folder-open")}${L("Show folder", "显示文件夹")}</button>`;
	const another = remaining().length ? `<button class="btn" data-a="another">${L("Convert another", "转换另一处")}</button>` : "";
	const close = `<button class="btn" data-a="reset">${L("Close", "关闭")}</button>`;
	if (r.kind === "stopped") return `${open}<span class="grow"></span>${close}<button class="btn primary" data-a="rest">${L("Convert the rest", "转换剩下的")}</button>`;
	if (r.kind === "partial") return `${open}<span class="grow"></span>${close}<button class="btn primary" data-a="retry">${L("Try again", "再试一次")}</button>`;
	return `${open}<span class="grow"></span>${another}${close}<button class="btn primary" data-a="launch">${L("Open FrameEver", "打开 FrameEver")}</button>`;
};
const noneState = () => `<div class="empty">${icon("folder", "lg")}<span>${L("No Cap or Cap-aniBullet 2.x data on this PC. Choose the folder that holds your recordings.", "这台电脑上没有 Cap 或 Cap-aniBullet 2.x 的数据。选择存放录制的文件夹。")}</span><button class="btn primary" data-a="folder">${L("Choose a folder…", "选择文件夹…")}</button></div>`;
const nothingHere = () => `<div class="empty">${icon("folder", "lg")}<span>${L("There's nothing to convert in this folder.", "这个文件夹里没有可转换的内容。")}</span><button class="btn" data-a="folder">${L("Choose another…", "换一个文件夹…")}</button></div>`;

const stepper = () => {
	const names = [["find", "Choose", "选择"], ["review", "Review", "确认"], ["moving", "Convert", "转换"], ["done", "Done", "完成"]];
	const at = names.findIndex(([key]) => key === s.step);
	return `<div class="steps">${names.map(([, en, zh], i) => (i === at ? `<b>${L(en, zh)}</b>` : i < at ? `<i>${L(en, zh)}</i>` : L(en, zh))).join(`<span class="sep">›</span>`)}</div>`;
};
const variantA = () => {
	let body;
	let foot;
	if (s.step === "find") {
		const list = foundList();
		body = list.length
			? `<div class="head"><h3>${L("Bring your recordings into FrameEver", "把录制带进 FrameEver")}</h3><p>${L("Found on this PC. Choose one to see what comes over.", "在这台电脑上找到了这些。选一项，看看会带上什么。")}</p></div><div class="group">${list.map((key) => `<button class="row" data-src="${key}">${icon("folder")}<span class="grow"><b>${SOURCES[key].name}</b><small>${countLine(SOURCES[key])}</small></span>${s.finished.includes(key) ? `<span class="num">${L("Converted", "已转换")}</span>` : ""}${icon("chevron-right", "sm")}</button>`).join("")}</div>`
			: noneState();
		foot = list.length ? `<button class="btn ghost" data-a="folder">${L("Choose a folder…", "选择文件夹…")}</button><span class="grow"></span><button class="btn" data-a="reset">${L("Close", "关闭")}</button>` : `<span class="grow"></span><button class="btn" data-a="reset">${L("Close", "关闭")}</button>`;
	} else if (s.step === "review") {
		const src = SOURCES[s.src];
		const empty = !src.rec && !src.shot && !src.settings;
		body = empty ? nothingHere() : `<div class="head"><h3>${L("Choose what comes over", "选择要带上的内容")}</h3><p>${L(`From ${src.name}`, `来自 ${src.name}`)}</p></div>${warnings()}${review()}${destination()}`;
		foot = `<button class="btn ghost" data-a="back">${icon("chevron-left", "sm")}${L("Back", "返回")}</button><span class="grow"></span>${empty ? "" : convertButton()}`;
	} else if (s.step === "moving") {
		body = `<div class="head"><h3>${L("Converting…", "正在转换…")}</h3><p>${L("Keep this window open.", "不要关闭这个窗口。")}</p></div>${moving()}${movingNote()}`;
		foot = `<span class="grow"></span><button class="btn" data-a="stop" ${s.stopping ? "disabled" : ""}>${L("Stop", "停止")}</button>`;
	} else {
		const r = result();
		const [head, sub] = resultHead(r);
		body = `${r.kind === "all" && !r.settingsFail ? `<div class="done-mark">${doodle()}</div>` : ""}<div class="head" style="${r.kind === "all" && !r.settingsFail ? "text-align:center" : ""}"><h3>${head}</h3><p>${sub}</p></div>${settingsBanner(r)}${r.left.length ? leftRows(r) : ""}`;
		foot = resultButtons(r);
	}
	return `<div class="win a">${title()}${stepper()}<div class="page"><div class="scroll-body">${body}</div><div class="foot">${foot}</div></div></div>`;
};

const variantB = () => {
	const list = foundList();
	const picked = s.src && !list.includes(s.src) ? [s.src] : [];
	const locked = s.step === "moving";
	const side = `<aside class="src-side"><h4>${L("Found on this PC", "在这台电脑上找到")}</h4>${[...list, ...picked].map((key) => `<button class="src-item ${key === s.src ? "on" : ""}" data-src="${key}" ${locked ? "disabled" : ""}>${icon("folder")}<span class="grow"><b>${SOURCES[key].name}</b><small>${L(`${SOURCES[key].rec + SOURCES[key].shot} items`, `${SOURCES[key].rec + SOURCES[key].shot} 项`)} · ${gb(SOURCES[key].recGb + SOURCES[key].shotGb)}</small></span>${s.finished.includes(key) ? `<span class="ok">${icon("circle-check")}</span>` : ""}</button>`).join("")}<span class="spacer"></span><button class="src-item" data-a="folder" ${locked ? "disabled" : ""}>${icon("plus")}<span class="grow">${L("Choose a folder…", "选择文件夹…")}</span></button></aside>`;
	let body;
	let foot = "";
	if (!s.src) {
		body = noneState();
	} else if (s.step === "review") {
		const src = SOURCES[s.src];
		const empty = !src.rec && !src.shot && !src.settings;
		body = empty ? nothingHere() : `<div class="head"><h3>${src.name}</h3><div class="path">${src.path}</div></div>${warnings()}${review()}${destination()}`;
		foot = empty ? "" : `<span class="grow"></span>${convertButton()}`;
	} else if (s.step === "moving") {
		body = `<div class="head"><h3>${L("Converting…", "正在转换…")}</h3><p>${L("Keep this window open.", "不要关闭这个窗口。")}</p></div>${moving()}${movingNote()}`;
		foot = `<span class="grow"></span><button class="btn" data-a="stop" ${s.stopping ? "disabled" : ""}>${L("Stop", "停止")}</button>`;
	} else {
		const r = result();
		const [head, sub] = resultHead(r);
		const clean = r.kind === "all" && !r.settingsFail;
		body = `${clean ? `<div class="done-mark">${doodle()}</div>` : ""}<div class="head" style="${clean ? "text-align:center" : ""}"><h3>${head}</h3><p>${sub}</p></div>${settingsBanner(r)}${r.left.length ? leftRows(r) : ""}`;
		foot = resultButtons(r);
	}
	return `<div class="win b">${title()}<div class="split">${side}<div class="page"><div class="scroll-body">${body}</div>${foot ? `<div class="foot">${foot}</div>` : ""}</div></div></div>`;
};

const variantC = () => {
	if (!s.src) return `<div class="win c">${title()}${noneState()}</div>`;
	const src = SOURCES[s.src];
	if (!src.rec && !src.shot && !src.settings) return `<div class="win c">${title()}${nothingHere()}</div>`;
	const all = items(s.src);
	const leftNames = s.step === "done" ? result().left : [];
	const statusOf = (item, index) => {
		if (s.step === "moving" || s.step === "done") {
			if (s.off.has(item.id)) return ["", ""];
			const order = all.filter((entry) => !s.off.has(entry.id)).indexOf(item);
			const miss = leftNames.find((entry) => entry.name === item.name);
			if (miss) return ["bad", L("Left behind", "没有转换")];
			if (order < s.done) return ["ok", icon("check", "sm")];
			if (s.step === "moving" && order === s.done) return ["", L("Moving…", "正在移动…")];
			if (s.step === "done") return ["", L("Not moved", "未移动")];
			return ["", L("Waiting", "等待中")];
		}
		return ["", ""];
	};
	const needle = s.q.trim().toLowerCase();
	const visible = all.filter((item) => (s.tab === "all" || item.kind === s.tab || (s.tab === "left" && leftNames.some((entry) => entry.name === item.name))) && (!needle || item.name.toLowerCase().includes(needle)));
	const chosen = all.filter((item) => !s.off.has(item.id)).length;
	const running = s.step === "moving";
	const fixed = running || s.step === "done";
	const tabs = [["all", "All", "全部", all.length], ["rec", "Recordings", "录制", src.rec], ["shot", "Screenshots", "截图", src.shot]];
	if (leftNames.length) tabs.push(["left", "Left behind", "没有转换", leftNames.length]);
	const srcMenu = s.popup ? `<div class="menu">${foundList().map((key) => `<button data-src="${key}"><span class="grow">${SOURCES[key].name}</span>${key === s.src ? icon("check", "sm") : ""}</button>`).join("")}<hr><button data-a="folder">${L("Choose a folder…", "选择文件夹…")}</button></div>` : "";
	const toolbar = `<div class="list-tools"><div class="pop-wrap"><button class="select" data-a="popup" ${running ? "disabled" : ""}>${icon("folder", "sm")}${src.name}${icon("chevron-down", "sm")}</button>${srcMenu}</div><div class="seg">${tabs.filter((tab) => tab[3] > 0).map(([key, en, zh, n]) => `<button class="${s.tab === key ? "on" : ""}" data-tab="${key}">${L(en, zh)} <em>${n}</em></button>`).join("")}</div><label class="input">${icon("search", "sm")}<input id="cv-search" value="${s.q}" data-ph-en="Search" data-ph-zh="搜索" placeholder="${pick("Search", "搜索")}"></label></div>`;
	const settingsRow = src.settings && s.tab === "all" && !needle ? `<div class="item">${fixed ? `<span style="width:16px"></span>` : check("settings", s.settings)}${icon("settings", "sm")}<span class="grow">${L("Settings and automations", "设置和自动化")}</span><em>${L("Only what FrameEver doesn't have yet", "只补上 FrameEver 还没有的")}</em><span class="state ${fixed && s.step === "done" && s.settings && !result().settingsFail ? "ok" : ""}">${s.step === "done" && s.settings ? (result().settingsFail ? L("Failed", "失败") : icon("check", "sm")) : ""}</span></div>` : "";
	const rows = visible.map((item) => {
		const [cls, text] = statusOf(item);
		const miss = leftNames.find((entry) => entry.name === item.name);
		return `<div class="item ${miss ? "tall" : ""}">${fixed ? `<span style="width:16px"></span>` : `<button class="check ${s.off.has(item.id) ? "" : "on"}" data-item="${item.id}">${icon("check", "sm")}</button>`}${icon(item.kind === "rec" ? "video" : "image", "sm")}<span class="grow">${item.name}</span><em>${item.date}</em><em style="width:58px;text-align:right">${item.size < 1 ? `${Math.round(item.size * 1000)} MB` : gb(item.size)}</em><span class="state ${cls}">${text}</span>${miss ? `<span class="why">${L(miss.en, miss.zh)}</span>` : ""}</div>`;
	}).join("");
	const head = `<div class="item head">${fixed ? `<span style="width:16px"></span>` : `<button class="check ${chosen === all.length ? "on" : ""}" data-a="all">${icon("check", "sm")}</button>`}<span style="width:14px"></span><span class="grow">${L("Name", "名称")}</span><em>${L("Date", "日期")}</em><em style="width:58px;text-align:right">${L("Size", "大小")}</em><span class="state"></span></div>`;
	let foot;
	if (s.step === "review") foot = `<span class="grow"><div style="font-weight:500">${L(`${selected().n} of ${all.length} selected · ${gb(selected().size)}`, `已选 ${selected().n} / ${all.length} 项 · ${gb(selected().size)}`)}</div><div class="path">${L("To", "转换到")} ${DEST}</div></span><button class="btn" data-a="change">${L("Change…", "更改…")}</button>${convertButton()}`;
	else if (running) foot = `<span class="grow"><div class="move"><div class="line"><span>${s.stopping ? L("Finishing this item…", "正在完成当前一项…") : L(`Converting… ${percent()}%`, `正在转换… ${percent()}%`)}</span><span>${s.done} / ${s.total}</span></div><div class="progress"><i style="width:${percent()}%"></i></div></div></span><button class="btn" data-a="stop" ${s.stopping ? "disabled" : ""}>${L("Stop", "停止")}</button>`;
	else {
		const r = result();
		const [title2, sub] = resultHead(r);
		foot = `<span class="grow"><div style="font-weight:500">${title2}</div><div class="hint" style="color:var(--muted-foreground);font-size:var(--text-small)">${sub}</div></span>${resultButtons(r).replace('<span class="grow"></span>', "")}`;
	}
	const banner = s.step === "review" ? warnings() : s.step === "done" ? settingsBanner(result()) : "";
	return `<div class="win c">${title()}${toolbar}${banner ? `<div style="padding:8px 16px 0">${banner}</div>` : ""}<div class="items">${head}${settingsRow}${rows}</div><div class="foot" style="border-top:1px solid var(--border);padding:10px 16px">${foot}</div></div>`;
};

const COMPARE = [
	[["Window", "窗口"], ["560 × 560, fixed, as today", "560 × 560，固定大小，同现在"], ["760 × 520, fixed", "760 × 520，固定大小"], ["880 × 600, resizable", "880 × 600，可调整大小"]],
	[["Choosing", "选择"], ["By kind: recordings, screenshots, settings", "按类别：录制、截图、设置"], ["By kind", "按类别"], ["Item by item, plus settings", "逐项，加设置"]],
	[["Two sources found", "找到两处数据"], ["One after the other, through the list", "回到列表，一处接一处"], ["Both stay in view, done ones get a tick", "两处一直都在，做完的打勾"], ["Dropdown; only one list shows", "下拉切换，一次只看一处"]],
	[["Progress", "进度"], ["Big percentage, current item", "大号百分比和当前项"], ["Same, sources stay beside it", "同左，来源留在旁边"], ["Each row changes state, bar in the footer", "每行变状态，进度条在底部"]],
	[["Items left behind", "没转换的内容"], ["List with reasons on the result page", "结果页列出原因"], ["Same", "同左"], ["A Left behind tab on the same list", "同一列表里的“没有转换”页签"]],
	[["New work in the core", "核心库要改的"], ["Filter by kind, progress callback", "按类别过滤、进度回调"], ["Same as A", "同 A"], ["Item filter, each item's size and date read up front, per-item status", "按项过滤、提前读每项的大小和日期、逐项状态"]],
	[["Fits a tool run once", "合不合只用一次的工具"], ["Closest to it", "最合适"], ["Good if people have two sources", "有两处数据时更好"], ["Most control, most to read", "控制最细，要看的也最多"]],
];
const ADDED = [
	["Choose by kind", "按类别选择", "Today everything in a source converts. OBS and Thunderbird both let you tick what comes over.", "现在一处数据只能整体转换。OBS 和 Thunderbird 都能勾选要带的内容。", "convert() takes a selection", "convert() 增加选择参数"],
	["Real progress and Stop", "真实进度和停止", "Today the window says only Converting… and can't be closed. Each item is a separate move, so stopping between items is safe; the caption close button stops too.", "现在窗口只写“正在转换…”，也关不掉。每项是单独移动的，在两项之间停止是安全的；窗口关闭按钮也等于停止。", "Progress callback and a stop flag", "进度回调和停止标记"],
	["Sizes and free space", "大小和剩余空间", "A move to another drive copies first, so it can run out of space partway. Warn before starting and offer Change.", "移到另一个磁盘要先复制，可能中途空间不足。开始前提醒，并提供更改位置。", "Sum sizes in the preview, read free space", "预览时累计大小、读取剩余空间"],
	["Change the destination", "更改目标位置", "Today it's fixed. The core already has choose_storage_folder.", "现在是固定的。核心库已有 choose_storage_folder。", "Save the choice to settings", "把选择写入设置"],
	["Warn when Cap is running", "Cap 运行时提醒", "Open projects fail as in use. Saying so first avoids a left-behind list.", "打开着的项目会因占用而失败。先提醒，就不会出现一串遗留项。", "Look for the old app's process", "检测旧程序进程"],
	["Try again on left-behind items", "对遗留内容再试一次", "The core already converts only what's left when run again; today you have to go back through the wizard.", "核心库再次运行时本来就只转换剩下的；现在要从头走一遍向导。", "None", "无"],
	["Open FrameEver", "打开 FrameEver", "The natural next step. Falls back to Show folder when FrameEver isn't installed.", "最自然的下一步。没装 FrameEver 时退回“显示文件夹”。", "Find the installed app", "查找已安装的应用"],
	["Skip Choose when one source is found", "只找到一处时跳过选择", "Goes straight to Review. Back still reaches Choose a folder.", "直接进入确认，返回后仍可选择文件夹。", "None", "无"],
];
const FIXES = [
	["Could not convert: {error}", "无法转换：{error}", "Couldn't move this recording. It's still where it was. [Details]", "无法移动这个录制。它还在原来的位置。[详情]"],
	["Settings were not carried over: {error}", "设置没有带过来：{error}", "Couldn't carry over your settings. Your recordings aren't affected. [Details]", "无法带上你的设置。录制不受影响。[详情]"],
	["Converting… (no number)", "正在转换…（没有数字）", "Converting… with a percentage", "正在转换… 加百分比"],
	["Recordings / 录制文件", "录制文件", "Recordings / 录制", "录制"],
];
const REFS = [
	["Upstream f7d351611", "上游 f7d351611", "No import or migration screen exists, so there's nothing to port.", "没有导入或迁移界面，无需移植。"],
	["OBS Studio scene collection import", "OBS Studio 场景集合导入", "Searches known locations of other apps, then lists what it finds with a checkbox per row.", "搜索其他应用的已知位置，把找到的内容列出来，每行一个复选框。"],
	["Thunderbird import tool", "Thunderbird 导入工具", "Source, details, what to import, then a summary with Back before starting. Steps aren't clickable.", "来源、细节、要导入的内容，开始前有摘要和返回。步骤不能点击。"],
	["Fluent 2 progress controls", "Fluent 2 进度控件", "A determinate bar when the end is known, with a line of text saying what's going on.", "终点已知时用确定进度条，配一行文字说明正在做什么。"],
	["Not found", "没找到", "I looked for Obsidian Importer's result screen and found no primary source, so it isn't used.", "查了 Obsidian Importer 的结果页，没找到一手资料，所以没有采用。"],
];

const seg = (key, options) => `<div class="seg">${options.map(([value, en, zh]) => `<button class="${s[key] === value ? "on" : ""}" data-s="${key}:${value}">${L(en, zh)}</button>`).join("")}</div>`;

const stageHtml = () => ({ A: variantA, B: variantB, C: variantC })[s.variant]();

const render = () => {
	const scroller = document.querySelector(".items, .scroll-body");
	const top = scroller?.scrollTop ?? 0;
	document.getElementById("root").innerHTML = `
		<h2>${L("FrameEver Converter window", "FrameEver Converter 窗口")}</h2>
		<p class="lead">${L("Three structures, drawn with the app's tokens. The window works: click through it. The controls set up what the machine has and how the run ends. Use the bar at the bottom, or ← →, to switch.", "三种结构，都用应用的 token 绘制。窗口可以操作，直接点下去。上面的选项决定这台电脑上有什么、转换怎么结束。用底部的条或 ← → 切换方案。")}</p>
		<div class="tools">
			<label>${L("Found", "找到")} ${seg("found", [["two", "Two", "两处"], ["one", "One", "一处"], ["none", "None", "没有"]])}</label>
			<label>${L("Before starting", "开始前")} ${seg("block", [["none", "Clear", "正常"], ["running", "Cap running", "Cap 在运行"], ["space", "Low space", "空间不够"]])}</label>
			<label>${L("Ending", "结果")} ${seg("outcome", [["all", "All moved", "全部完成"], ["partial", "Left behind", "有遗留"], ["settings", "Settings failed", "设置失败"]])}</label>
			<button class="btn ghost" data-a="reset">${icon("rotate-ccw", "sm")}${L("Start over", "重新开始")}</button>
		</div>
		<div class="proto-stage" id="stage">${stageHtml()}</div>
		<h2>${L("Comparison", "对比")}</h2>
		<div class="wrap"><table><tr><th></th><th>A ${L(...VARIANTS.A)}</th><th>B ${L(...VARIANTS.B)}</th><th>C ${L(...VARIANTS.C)}</th></tr>${COMPARE.map(([k, a, b, c]) => `<tr><th>${L(...k)}</th><td>${L(...a)}</td><td>${L(...b)}</td><td>${L(...c)}</td></tr>`).join("")}</table></div>
		<h2>${L("Added in every variant", "每个方案都加的")}</h2>
		<p class="lead">${L("The wizard today does none of these. Each costs a small change outside the window.", "现在的向导都没有这些。每项都要在窗口之外做一点小改动。")}</p>
		<div class="wrap"><table><tr><th>${L("What", "内容")}</th><th>${L("Why", "原因")}</th><th>${L("Code it needs", "要改的代码")}</th></tr>${ADDED.map(([a, b, c, d, e, f]) => `<tr><th>${L(a, b)}</th><td>${L(c, d)}</td><td>${L(e, f)}</td></tr>`).join("")}</table></div>
		<h2>${L("Copy fixed against the style guide", "按文案规范改的")}</h2>
		<div class="wrap"><table><tr><th>${L("Today", "现在")}</th><th>${L("New", "改后")}</th></tr>${FIXES.map(([a, b, c, d]) => `<tr><td>${L(a, b)}</td><td>${L(c, d)}</td></tr>`).join("")}</table></div>
		<h2>${L("Settled, the same in every variant", "已定，各方案相同")}</h2>
		<div class="wrap"><table>${[
			[["Tokens and parts", "Token 和组件"], ["Same tokens as the app, GPUI Kit button, checkbox, progress, tab, input, select. theme.rs drops its private palette.", "和应用同一套 token，用 GPUI Kit 的按钮、复选框、进度条、页签、输入框、下拉框。theme.rs 的私有色板去掉。"]],
			[["Blue", "蓝色"], ["One Klein button per screen, on-state checkboxes, the progress bar. Nothing else.", "每屏一个 Klein 按钮，勾选的复选框，进度条。其他都不用蓝。"]],
			[["Theme and language", "主题和语言"], ["Follows Windows dark or light. Language from FrameEver's setting, else Windows.", "跟随 Windows 的深浅色。语言取 FrameEver 的设置，没有就跟随 Windows。"]],
			[["Chinese font", "中文字体"], ["Stays Microsoft YaHei UI: HarmonyOS Sans is 20 MB and this is a small download. It's the one visible difference from the app.", "继续用 Microsoft YaHei UI：HarmonyOS Sans 有 20 MB，而这是个小下载。这是和应用唯一看得出的差别。"]],
			[["Hand-drawn", "手绘"], ["The ALL_SET tick when everything moved, drawn once. Never with a warning. This adds a tenth place to the list of nine.", "全部完成时画一次 ALL_SET 的勾，有警告时不画。这是九处手绘之外新增的第十处。"]],
			[["Keyboard", "键盘"], ["Tab through everything, Space ticks, Enter runs the blue button, Esc goes back. During a run, Esc does nothing.", "Tab 走遍所有控件，空格勾选，Enter 触发蓝色按钮，Esc 返回。转换中 Esc 不起作用。"]],
		].map(([k, v]) => `<tr><th>${L(...k)}</th><td>${L(...v)}</td></tr>`).join("")}</table></div>
		<h2>${L("References", "参考")}</h2>
		<div class="wrap"><table>${REFS.map(([a, b, c, d]) => `<tr><th>${L(a, b)}</th><td>${L(c, d)}</td></tr>`).join("")}</table></div>`;
	const next = document.querySelector(".items, .scroll-body");
	if (next) next.scrollTop = top;
	document.getElementById("switcher").innerHTML = `<button data-go="-1" aria-label="Previous">←</button><span>${s.variant} · ${pick(...VARIANTS[s.variant])}</span><button data-go="1" aria-label="Next">→</button>`;
};

const boot = () => {
	Object.assign(s, fresh());
	const list = foundList();
	if (s.variant === "A") {
		if (list.length === 1) {
			s.src = list[0];
			s.step = "review";
		}
	} else if (list.length) {
		s.src = list[0];
		s.step = "review";
	}
};
let timer = null;
const halt = () => {
	clearInterval(timer);
	timer = null;
};
const run = (from = 0) => {
	s.step = "moving";
	s.done = from;
	s.stopping = false;
	s.stopped = false;
	timer = setInterval(() => {
		s.done = Math.min(s.total, s.done + Math.max(1, Math.round(s.total / 45)));
		if (s.stopping) {
			halt();
			s.stopped = true;
			s.step = "done";
		} else if (s.done >= s.total) {
			halt();
			s.step = "done";
			if (!s.finished.includes(s.src) && result().kind === "all") s.finished.push(s.src);
		}
		render();
	}, 120);
};
const actions = {
	reset: () => { halt(); boot(); },
	back: () => { s.step = "find"; s.src = null; },
	recheck: () => { s.block = "none"; },
	change: () => { if (s.block === "space") s.block = "none"; },
	folder: () => { s.src = s.src === "folder" ? "blank" : "folder"; s.step = "review"; s.popup = false; s.off = new Set(); },
	convert: () => { s.total = selected().n; s.tried = false; run(); },
	stop: () => { s.stopping = true; },
	rest: () => { run(s.done); },
	retry: () => { s.tried = true; s.step = "done"; if (!s.finished.includes(s.src)) s.finished.push(s.src); },
	another: () => {
		const next = remaining()[0];
		if (s.variant === "A") { s.step = "find"; s.src = null; } else { s.src = next; s.step = "review"; }
		s.off = new Set();
	},
	open: () => {},
	launch: () => {},
	detail: () => { s.detail = !s.detail; },
	popup: () => { s.popup = !s.popup; },
	all: () => {
		const list = items(s.src);
		s.off = s.off.size ? new Set() : new Set(list.map((item) => item.id));
	},
};
document.addEventListener("click", (event) => {
	const go = event.target.closest("[data-go]");
	if (go) return cycle(Number(go.dataset.go));
	const set = event.target.closest("[data-s]");
	if (set) {
		const [key, value] = set.dataset.s.split(":");
		s[key] = value;
		if (key === "found") { halt(); boot(); }
		return render();
	}
	const stage = event.target.closest("#stage");
	const button = event.target.closest("[data-a],[data-src],[data-toggle],[data-tab],[data-item]");
	if (!button) return;
	const data = button.dataset;
	if (data.a) actions[data.a]?.();
	else if (!stage) return;
	else if (data.src) {
		s.src = data.src;
		s.step = "review";
		s.popup = false;
		s.off = new Set();
		s.tab = "all";
		s.rec = true;
		s.shot = true;
		s.settings = true;
		s.detail = false;
	} else if (data.toggle) s[data.toggle] = !s[data.toggle];
	else if (data.tab) s.tab = data.tab;
	else if (data.item) {
		if (s.off.has(data.item)) s.off.delete(data.item);
		else s.off.add(data.item);
	}
	render();
});
document.addEventListener("input", (event) => {
	if (event.target.id !== "cv-search") return;
	const caret = event.target.selectionStart;
	s.q = event.target.value;
	render();
	const input = document.getElementById("cv-search");
	input.focus();
	input.setSelectionRange(caret, caret);
});
const cycle = (by) => {
	const list = Object.keys(VARIANTS);
	s.variant = list[(list.indexOf(s.variant) + by + list.length) % list.length];
	halt();
	boot();
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

boot();
shell({ here: "converter", depth: 1 });
onChange(render);
render();
