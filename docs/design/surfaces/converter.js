const { L, icon, pick, shell, onChange } = FE;

const DEST = "C:\\Users\\you\\Videos\\FrameEver";
const SOURCES = {
	two: {
		name: "Cap-aniBullet 2.x",
		rec: 42,
		shot: 18,
		recGb: 36.9,
		shotGb: 1.3,
		settings: true,
		rules: 2,
		across: true,
	},
	cap: {
		name: "Cap",
		rec: 6,
		shot: 3,
		recGb: 4.0,
		shotGb: 0.1,
		settings: true,
		rules: 0,
		across: false,
	},
	folder: {
		name: "D:\\Old recordings",
		rec: 5,
		shot: 0,
		recGb: 2.4,
		shotGb: 0,
		settings: false,
		rules: 0,
		across: true,
	},
	blank: {
		name: "D:\\Downloads",
		rec: 0,
		shot: 0,
		recGb: 0,
		shotGb: 0,
		settings: false,
		rules: 0,
		across: true,
	},
};
const NAMES = [
	"Product demo",
	"Bug repro",
	"Onboarding walkthrough",
	"Team sync",
	"Launch recap",
	"Sales call",
	"Tutorial",
	"Design review",
	"Weekly update",
	"Support case",
];
const LEFT = [
	{
		name: "Team sync",
		en: "In use by another program. Close it and try again.",
		zh: "正被其他程序占用。关闭该程序后再试一次。",
	},
	{
		name: "Launch recap",
		en: "Couldn't move this recording. It's still where it was.",
		zh: "无法移动这个录制。它还在原来的位置。",
		raw: "The copy of D:\\Cap Recordings\\recordings\\Launch recap.cap does not match the original",
	},
];

const fresh = () => ({
	step: "find",
	src: null,
	rec: true,
	shot: true,
	settings: true,
	done: 0,
	total: 0,
	stopping: false,
	stopped: false,
	detail: false,
	finished: [],
	tried: false,
});
const s = { found: "two", block: "none", outcome: "all", ...fresh() };

const foundList = () =>
	({ two: ["two", "cap"], one: ["two"], none: [] })[s.found];
const gb = (n) => `${n.toFixed(1)} GB`;
const countLine = (src) =>
	`${L(`${src.rec} recordings`, `${src.rec} 个录制`)} · ${L(`${src.shot} screenshots`, `${src.shot} 张截图`)} · ${gb(src.recGb + src.shotGb)}`;
const selected = () => {
	const src = SOURCES[s.src];
	return {
		n: (s.rec ? src.rec : 0) + (s.shot ? src.shot : 0),
		size: (s.rec ? src.recGb : 0) + (s.shot ? src.shotGb : 0),
		settings: src.settings && s.settings,
	};
};
const lacksSpace = () => s.block === "space" && SOURCES[s.src].across;
const canConvert = () =>
	(selected().n > 0 || selected().settings) && !lacksSpace();
const percent = () => (s.total ? Math.round((s.done / s.total) * 100) : 100);
const currentName = () => {
	const index = Math.min(s.done, Math.max(0, s.total - 1));
	return index < NAMES.length
		? NAMES[index]
		: `${NAMES[index % NAMES.length]} ${Math.floor(index / NAMES.length) + 1}`;
};

const title = () =>
	`<div class="cap-bar"><span class="logo"></span>FrameEver Converter<span class="grow"></span><span class="caption">${icon("minus")}</span><span class="caption" ${s.step === "moving" ? `data-a="stop" title="${pick("Stops after this item", "做完当前一项后停止")}"` : ""}>${icon("x")}</span></div>`;
const stepper = () => {
	const names = [
		["find", "Choose", "选择"],
		["review", "Review", "确认"],
		["moving", "Convert", "转换"],
		["done", "Done", "完成"],
	];
	const at = names.findIndex(([key]) => key === s.step);
	return `<div class="steps-line">${names.map(([, en, zh], i) => (i === at ? `<b>${L(en, zh)}</b>` : i < at ? `<i>${L(en, zh)}</i>` : L(en, zh))).join(`<span class="sep">›</span>`)}</div>`;
};
const check = (key, on) =>
	`<button class="check ${on ? "on" : ""}" data-toggle="${key}" role="checkbox" aria-checked="${on}">${icon("check", "sm")}</button>`;
const warnings = () => {
	if (lacksSpace())
		return `<div class="warn">${icon("triangle-alert")}<span class="grow">${L(`Not enough space on C:. This needs ${gb(selected().size)} and 12.1 GB is free. Change the folder or convert less.`, `C: 盘空间不够。需要 ${gb(selected().size)}，只剩 12.1 GB。换个文件夹，或者少转换一些。`)}</span></div>`;
	if (s.block === "running")
		return `<div class="warn">${icon("triangle-alert")}<span class="grow">${L("Cap is running. Close it first, or the projects it has open stay behind.", "Cap 正在运行。先关闭它，否则它打开着的项目会留在原处。")}</span><button class="btn ghost" data-a="recheck">${L("Check again", "再次检查")}</button></div>`;
	return "";
};
const review = () => {
	const src = SOURCES[s.src];
	const rows = [
		[
			"rec",
			s.rec,
			"video",
			L("Recordings", "录制"),
			L(".cap projects and loose MP4 exports", ".cap 项目和零散的 MP4 导出"),
			L(`${src.rec} · ${gb(src.recGb)}`, `${src.rec} 个 · ${gb(src.recGb)}`),
			src.rec,
		],
		[
			"shot",
			s.shot,
			"image",
			L("Screenshots", "截图"),
			L("PNG and JPEG, kept with their dates", "PNG 和 JPEG，保留原来的日期"),
			L(
				`${src.shot} · ${gb(src.shotGb)}`,
				`${src.shot} 张 · ${gb(src.shotGb)}`,
			),
			src.shot,
		],
	].filter((row) => row[6] > 0);
	const settings = src.settings
		? `<div class="row">${check("settings", s.settings)}${icon("settings")}<span class="grow"><b>${L("Settings and automations", "设置和自动化")}</b><small>${L("Only what FrameEver doesn't have yet", "只补上 FrameEver 还没有的")}</small></span><span class="num">${src.rules ? L(`${src.rules} rules`, `${src.rules} 条规则`) : ""}</span></div>`
		: "";
	return `<div class="group">${rows.map(([key, on, ic, name, sub, num]) => `<div class="row ${on ? "" : "off"}">${check(key, on)}${icon(ic)}<span class="grow"><b>${name}</b><small>${sub}</small></span><span class="num">${num}</span></div>`).join("")}${settings}</div>`;
};
const destination =
	() => `<div class="group"><div class="row"><span class="grow"><b>${L("Destination", "转换到")}</b><div class="path">${DEST}</div></span><button class="btn" data-a="change">${L("Change…", "更改…")}</button></div></div>
	<p class="moved-note">${SOURCES[s.src].across ? L("Files are moved to another drive: copied, checked, then the originals are deleted. Afterwards only FrameEver opens them.", "文件会移到另一个磁盘：先复制、再校验，然后删除原文件。之后只有 FrameEver 能打开它们。") : L("Files are moved, not copied. Afterwards only FrameEver opens them.", "文件会被移动而不是复制，之后只有 FrameEver 能打开它们。")}</p>`;
const convertButton = () => {
	const n = selected().n;
	return `<button class="btn primary" data-a="convert" ${canConvert() ? "" : "disabled"}>${n ? L(`Convert ${n} items`, `转换 ${n} 项`) : L("Convert", "转换")}</button>`;
};
const moving = () => {
	const left = selected().size * (1 - s.done / Math.max(1, s.total));
	const at = Math.min(s.done + 1, s.total);
	return `<div class="move"><div class="big-pct">${percent()}%</div><div class="progress"><i style="width:${percent()}%"></i></div><div class="line"><span>${s.stopping ? L("Finishing this item…", "正在完成当前一项…") : L(`Moving ${at} of ${s.total}: ${currentName()}`, `正在移动第 ${at} 项，共 ${s.total} 项：${currentName()}`)}</span><span>${SOURCES[s.src].across ? L(`${gb(left)} left`, `还剩 ${gb(left)}`) : ""}</span></div></div><p class="moved-note">${L("Items not moved yet stay where they are if you stop.", "停止后，还没移动的内容留在原处。")}</p>`;
};
const result = () => {
	if (s.stopped)
		return {
			kind: "stopped",
			converted: s.done,
			left: [],
			settingsFail: false,
		};
	const settingsFail = s.outcome === "settings" && selected().settings;
	const left = s.outcome === "partial" && !s.tried ? LEFT : [];
	return {
		kind: left.length ? "partial" : "all",
		converted: s.total - left.length,
		left,
		settingsFail,
	};
};
const doodle = () =>
	`<div class="done-mark"><svg width="72" height="62" viewBox="0 0 120 104" aria-hidden="true"><path pathLength="1" d="M 60 12 C 84 12 102 30 102 52 C 102 75 84 92 60 92 C 35 92 18 74 18 52 C 18 31 34 14 56 12.5 C 62 12 68 13 73 15" fill="none" stroke="var(--muted-foreground)" stroke-width="3" stroke-linecap="round"/><path pathLength="1" d="M 38 54 L 54 70 L 84 34" fill="none" stroke="var(--success-text)" stroke-width="5.5" stroke-linecap="round" stroke-linejoin="round"/></svg></div>`;
const leftRows = (r) =>
	`<div class="group">${r.left.map((item) => `<div class="row"><span class="amber">${icon("triangle-alert")}</span><span class="grow"><b>${item.name}</b><small>${L(item.en, item.zh)}</small>${item.raw ? `<button class="linkish" data-a="detail">${L("Details", "详情")}</button>${s.detail ? `<div class="detail">${item.raw}</div>` : ""}` : ""}</span></div>`).join("")}</div>`;
const settingsBanner = (r) =>
	r.settingsFail
		? `<div class="warn">${icon("triangle-alert")}<span class="grow">${L("Couldn't carry over your settings. Your recordings aren't affected.", "无法带上你的设置。录制不受影响。")} <button class="linkish" data-a="detail">${L("Details", "详情")}</button>${s.detail ? `<div class="detail">Access is denied. (os error 5)</div>` : ""}</span></div>`
		: "";
const resultHead = (r) => {
	if (r.kind === "stopped")
		return [
			L("Conversion stopped", "转换已停止"),
			L(
				`${r.converted} of ${s.total} items are now in FrameEver. The rest are where they were.`,
				`${s.total} 项中的 ${r.converted} 项已进入 FrameEver，其余的还在原处。`,
			),
		];
	if (r.kind === "partial")
		return [
			L("Some items were left behind", "有些内容没有转换"),
			L(
				`${r.converted} of ${s.total} items are now in FrameEver.`,
				`${s.total} 项中的 ${r.converted} 项已进入 FrameEver。`,
			),
		];
	return [
		L("Conversion finished", "转换完成"),
		L(
			`${s.total} items are now in FrameEver.`,
			`${s.total} 项内容已进入 FrameEver。`,
		),
	];
};
const remaining = () =>
	foundList().filter((key) => key !== s.src && !s.finished.includes(key));
const resultButtons = (r) => {
	const open = `<button class="btn ghost" data-a="noop">${icon("folder-open")}${L("Show folder", "显示文件夹")}</button>`;
	const another = remaining().length
		? `<button class="btn" data-a="another">${L("Convert another", "转换另一处")}</button>`
		: "";
	const close = `<button class="btn" data-a="reset">${L("Close", "关闭")}</button>`;
	if (r.kind === "stopped")
		return `${open}<span class="grow"></span>${close}<button class="btn primary" data-a="rest">${L("Convert the rest", "转换剩下的")}</button>`;
	if (r.kind === "partial")
		return `${open}<span class="grow"></span>${close}<button class="btn primary" data-a="retry">${L("Try again", "再试一次")}</button>`;
	return `${open}<span class="grow"></span>${another}${close}<button class="btn primary" data-a="noop">${L("Open FrameEver", "打开 FrameEver")}</button>`;
};
const noneState = () =>
	`<div class="empty">${icon("folder", "lg")}<span>${L("No Cap or Cap-aniBullet 2.x data on this PC. Choose the folder that holds your recordings.", "这台电脑上没有 Cap 或 Cap-aniBullet 2.x 的数据。选择存放录制的文件夹。")}</span><button class="btn primary" data-a="folder">${L("Choose a folder…", "选择文件夹…")}</button></div>`;
const nothingHere = () =>
	`<div class="empty">${icon("folder", "lg")}<span>${L("There's nothing to convert in this folder.", "这个文件夹里没有可转换的内容。")}</span><button class="btn" data-a="folder">${L("Choose another…", "换一个文件夹…")}</button></div>`;

const page = () => {
	if (s.step === "find") {
		const list = foundList();
		const close = `<span class="grow"></span><button class="btn" data-a="reset">${L("Close", "关闭")}</button>`;
		if (!list.length) return [noneState(), close];
		return [
			`<div class="head"><h3>${L("Bring your recordings into FrameEver", "把录制带进 FrameEver")}</h3><p>${L("Found on this PC. Choose one to see what comes over.", "在这台电脑上找到了这些。选一项，看看会带上什么。")}</p></div><div class="group">${list.map((key) => `<button class="row" data-src="${key}">${icon("folder")}<span class="grow"><b>${SOURCES[key].name}</b><small>${countLine(SOURCES[key])}</small></span>${s.finished.includes(key) ? `<span class="num">${L("Converted", "已转换")}</span>` : ""}${icon("chevron-right", "sm")}</button>`).join("")}</div>`,
			`<button class="btn ghost" data-a="folder">${L("Choose a folder…", "选择文件夹…")}</button>${close}`,
		];
	}
	if (s.step === "review") {
		const src = SOURCES[s.src];
		const empty = !src.rec && !src.shot && !src.settings;
		const back = `<button class="btn ghost" data-a="back">${icon("chevron-left", "sm")}${L("Back", "返回")}</button><span class="grow"></span>`;
		if (empty) return [nothingHere(), back];
		return [
			`<div class="head"><h3>${L("Choose what comes over", "选择要带上的内容")}</h3><p>${L(`From ${src.name}`, `来自 ${src.name}`)}</p></div>${warnings()}${review()}${destination()}`,
			`${back}${convertButton()}`,
		];
	}
	if (s.step === "moving") {
		return [
			`<div class="head"><h3>${L("Converting…", "正在转换…")}</h3><p>${L("Keep this window open.", "不要关闭这个窗口。")}</p></div>${moving()}`,
			`<span class="grow"></span><button class="btn" data-a="stop" ${s.stopping ? "disabled" : ""}>${L("Stop", "停止")}</button>`,
		];
	}
	const r = result();
	const [head, sub] = resultHead(r);
	const clean = r.kind === "all" && !r.settingsFail;
	return [
		`${clean ? doodle() : ""}<div class="head ${clean ? "center" : ""}"><h3>${head}</h3><p>${sub}</p></div>${settingsBanner(r)}${r.left.length ? leftRows(r) : ""}`,
		resultButtons(r),
	];
};

const seg = (key, options) =>
	`<div class="seg">${options.map(([value, en, zh]) => `<button class="${s[key] === value ? "on" : ""}" data-s="${key}:${value}">${L(en, zh)}</button>`).join("")}</div>`;
const render = () => {
	document.getElementById("tools").innerHTML =
		`<label>${L("Found", "找到")} ${seg("found", [
			["two", "Two", "两处"],
			["one", "One", "一处"],
			["none", "None", "没有"],
		])}</label><label>${L("Before starting", "开始前")} ${seg("block", [
			["none", "Clear", "正常"],
			["running", "Cap running", "Cap 在运行"],
			["space", "Low space", "空间不够"],
		])}</label><label>${L("Ending", "结果")} ${seg("outcome", [
			["all", "All moved", "全部完成"],
			["partial", "Left behind", "有遗留"],
			["settings", "Settings failed", "设置失败"],
		])}</label><button class="btn ghost" data-a="reset">${icon("rotate-ccw", "sm")}${L("Start over", "重新开始")}</button>`;
	const [body, foot] = page();
	document.getElementById("stage").innerHTML =
		`<div class="win">${title()}${stepper()}<div class="page"><div class="scroll-body">${body}</div><div class="foot">${foot}</div></div></div>`;
};

const boot = () => {
	Object.assign(s, fresh());
	const list = foundList();
	if (list.length === 1) {
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
			s.stopped = s.done < s.total;
			s.step = "done";
		} else if (s.done >= s.total) {
			halt();
			s.step = "done";
		}
		if (
			s.step === "done" &&
			!s.stopped &&
			result().kind === "all" &&
			!s.finished.includes(s.src)
		)
			s.finished.push(s.src);
		render();
	}, 120);
};
const actions = {
	reset: () => {
		halt();
		boot();
	},
	back: () => {
		s.step = "find";
		s.src = null;
	},
	recheck: () => {
		s.block = "none";
	},
	change: () => {
		if (s.block === "space") s.block = "none";
	},
	folder: () => {
		s.src = s.src === "folder" ? "blank" : "folder";
		s.step = "review";
	},
	convert: () => {
		s.total = selected().n;
		s.tried = false;
		run();
	},
	stop: () => {
		s.stopping = true;
	},
	rest: () => run(s.done),
	retry: () => {
		s.tried = true;
		if (!s.finished.includes(s.src)) s.finished.push(s.src);
	},
	another: () => {
		s.step = "find";
		s.src = null;
	},
	detail: () => {
		s.detail = !s.detail;
	},
	noop: () => {},
};
document.addEventListener("click", (event) => {
	const set = event.target.closest("#tools [data-s]");
	if (set) {
		const [key, value] = set.dataset.s.split(":");
		s[key] = value;
		if (key === "found") {
			halt();
			boot();
		}
		return render();
	}
	const button = event.target.closest(
		"#tools [data-a], #stage [data-a], #stage [data-src], #stage [data-toggle]",
	);
	if (!button) return;
	const data = button.dataset;
	if (data.a) actions[data.a]();
	else if (data.src) {
		Object.assign(s, {
			src: data.src,
			step: "review",
			rec: true,
			shot: true,
			settings: true,
			detail: false,
		});
	} else s[data.toggle] = !s[data.toggle];
	render();
});

const table = (id, head, rows) => {
	document.getElementById(id).innerHTML =
		`<table><tr>${head.map((pair) => `<th>${L(...pair)}</th>`).join("")}</tr>${rows.map((row) => `<tr>${row.map((cell, i) => (i ? `<td>${L(...cell)}</td>` : `<th>${L(...cell)}</th>`)).join("")}</tr>`).join("")}</table>`;
};
table(
	"step-table",
	[
		["Step", "步骤"],
		["Shows", "显示"],
		["Blue button", "蓝色按钮"],
	],
	[
		[
			["Choose", "选择"],
			[
				"Each install found, with counts and size. Skipped when exactly one is found; Back still reaches it. A folder picked by hand joins at Review.",
				"找到的每一处数据，含数量和大小。只找到一处时跳过，返回后仍可回到这里。手动选的文件夹直接进入确认。",
			],
			[
				"None. With nothing found: Choose a folder",
				"没有。什么都没找到时：选择文件夹",
			],
		],
		[
			["Review", "确认"],
			[
				"A checkbox per kind, the destination with Change, and what moving means here. Amber warnings sit above the list.",
				"每个类别一个复选框、带“更改”的目标位置，以及这次移动意味着什么。琥珀色警告放在列表上方。",
			],
			["Convert 60 items", "转换 60 项"],
		],
		[
			["Convert", "转换"],
			[
				"Percentage, the bar, the item being moved, and size left when crossing drives. Stop finishes the current item first.",
				"百分比、进度条、当前项，跨磁盘时显示剩余大小。停止会先完成当前一项。",
			],
			["None", "没有"],
		],
		[
			["Done, all moved", "完成，全部成功"],
			[
				"The hand-drawn tick, drawn once, and the count.",
				"画一次手绘的勾和数量。",
			],
			[
				"Open FrameEver, or Show folder if it isn't installed",
				"打开 FrameEver；没安装时是“显示文件夹”",
			],
		],
		[
			["Done, items left behind", "完成，有遗留"],
			[
				"Each item with its reason. Raw error text sits behind Details.",
				"每项和原因。原始错误文字放在“详情”里。",
			],
			["Try again", "再试一次"],
		],
		[
			["Done, settings failed", "完成，设置失败"],
			[
				"The count, plus an amber line. Recordings aren't affected.",
				"数量加一行琥珀色提示。录制不受影响。",
			],
			["Open FrameEver", "打开 FrameEver"],
		],
		[
			["Done, stopped", "完成，已停止"],
			["What moved and that the rest stayed.", "已移动多少，其余留在原处。"],
			["Convert the rest", "转换剩下的"],
		],
	],
);
table(
	"rule-table",
	[
		["Rule", "规则"],
		["Decision", "决定"],
	],
	[
		[
			["Tokens and parts", "Token 和组件"],
			[
				"The app's tokens and GPUI Kit button, checkbox and progress. The converter's private palette in theme.rs goes.",
				"用应用的 token 和 GPUI Kit 的按钮、复选框、进度条。theme.rs 里 Converter 的私有色板去掉。",
			],
		],
		[
			["Window", "窗口"],
			[
				"560 × 560, not resizable, minimise and close only. Steps show as text and can't be clicked; Back moves between them.",
				"560 × 560，不能调整大小，只有最小化和关闭。步骤是文字，不能点击，用“返回”切换。",
			],
		],
		[
			["Blue", "蓝色"],
			[
				"At most one Klein button per step, on checkboxes, the progress bar, the app mark. Nothing else.",
				"每步最多一个 Klein 按钮，勾选的复选框，进度条，应用标记。其他都不用蓝。",
			],
		],
		[
			["Closing while converting", "转换时关闭窗口"],
			[
				"The caption close button does what Stop does. Today it's refused.",
				"窗口关闭按钮等同于“停止”。现在是拒绝关闭。",
			],
		],
		[
			["Stopping", "停止"],
			[
				"Each item is a separate move, so stopping between items is safe. No confirmation, since nothing is lost.",
				"每项是单独移动的，在两项之间停止是安全的。不丢内容，所以不用确认。",
			],
		],
		[
			["Not enough space", "空间不够"],
			[
				"Only checked when crossing drives. Convert stays off until the selection fits or the destination changes.",
				"只在跨磁盘时检查。选的内容放得下或换了位置之前，“转换”不可用。",
			],
		],
		[
			["Theme and language", "主题和语言"],
			[
				"Follows Windows dark or light. Language from FrameEver's setting, else Windows.",
				"跟随 Windows 的深浅色。语言取 FrameEver 的设置，没有就跟随 Windows。",
			],
		],
		[
			["Chinese font", "中文字体"],
			[
				"Stays Microsoft YaHei UI: HarmonyOS Sans is 20 MB and this is a small download. It's the one visible difference from the app.",
				"继续用 Microsoft YaHei UI：HarmonyOS Sans 有 20 MB，而这是个小下载。这是和应用唯一看得出的差别。",
			],
		],
		[
			["Hand-drawn", "手绘"],
			[
				"ALL_SET when everything moved, drawn once, then still. Never with a warning. This is the tenth place on the list.",
				"全部成功时画一次 ALL_SET，然后静止，有警告时不画。这是清单上的第十处。",
			],
		],
	],
);
table(
	"key-table",
	[
		["Key", "按键"],
		["Does", "作用"],
	],
	[
		[
			["Tab", "Tab"],
			["Moves through every control", "在所有控件之间移动"],
		],
		[
			["Space", "空格"],
			["Ticks the focused checkbox", "勾选当前复选框"],
		],
		[
			["Enter", "Enter"],
			[
				"Runs the focused control, else the blue button",
				"触发当前控件，没有焦点时触发蓝色按钮",
			],
		],
		[
			["Esc", "Esc"],
			[
				"Goes back from Review. Does nothing while converting",
				"在确认页返回。转换时不起作用",
			],
		],
	],
);
table(
	"inv-table",
	[
		["Today", "现在"],
		["Becomes", "改为"],
	],
	[
		[
			[
				"Find: rows of installs, Choose a folder, Close",
				"查找：数据列表、选择文件夹、关闭",
			],
			["Choose, the same", "选择，不变"],
		],
		[
			[
				"Preview: counts, settings, destination, moved-not-copied note",
				"预览：数量、设置、目标位置、移动提示",
			],
			[
				"Review, with checkboxes, sizes and Change",
				"确认，加复选框、大小和“更改”",
			],
		],
		[
			["Nothing to convert here", "这里没有可转换的内容"],
			["The same, with Choose another", "不变，加“换一个文件夹”"],
		],
		[
			["Converting: text only", "转换中：只有文字"],
			["Percentage, bar, current item, Stop", "百分比、进度条、当前项、停止"],
		],
		[
			["Report: items left behind with reasons", "报告：遗留项和原因"],
			["The same, with Try again and Details", "不变，加“再试一次”和“详情”"],
		],
		[
			["Open storage folder", "打开存储文件夹"],
			["Show folder", "显示文件夹"],
		],
		[
			["Convert more", "继续转换"],
			[
				"Convert another, only when another install is left. Otherwise picking a folder is on Choose",
				"转换另一处，只在还有其他数据时显示。其他情况到“选择”里选文件夹",
			],
		],
		[
			["Could not convert: {error}", "无法转换：{error}"],
			[
				"Couldn't move this recording. It's still where it was. [Details]",
				"无法移动这个录制。它还在原来的位置。[详情]",
			],
		],
		[
			["Settings were not carried over: {error}", "设置没有带过来：{error}"],
			[
				"Couldn't carry over your settings. Your recordings aren't affected. [Details]",
				"无法带上你的设置。录制不受影响。[详情]",
			],
		],
		[
			["录制文件", "录制文件"],
			["Recordings, 录制", "录制"],
		],
	],
);
table(
	"add-table",
	[
		["Addition", "新增"],
		["Why", "原因"],
		["Code it needs", "要改的代码"],
	],
	[
		[
			["Choose by kind", "按类别选择"],
			[
				"Today everything in a source converts. OBS and Thunderbird both let you tick what comes over.",
				"现在一处数据只能整体转换。OBS 和 Thunderbird 都能勾选要带的内容。",
			],
			["convert() takes a selection", "convert() 增加选择参数"],
		],
		[
			["Real progress and Stop", "真实进度和停止"],
			[
				"Today the window says only Converting… and can't be closed.",
				"现在窗口只写“正在转换…”，也关不掉。",
			],
			["Progress callback and a stop flag", "进度回调和停止标记"],
		],
		[
			["Sizes and free space", "大小和剩余空间"],
			[
				"A move to another drive copies first, so it can run out of space partway.",
				"移到另一个磁盘要先复制，可能中途空间不足。",
			],
			[
				"Sum sizes in the preview, read free space",
				"预览时累计大小、读取剩余空间",
			],
		],
		[
			["Change the destination", "更改目标位置"],
			[
				"Lets a user with a full C: pick another drive.",
				"C 盘满时可以换到别的磁盘。",
			],
			[
				"Save the choice with choose_storage_folder",
				"用 choose_storage_folder 保存选择",
			],
		],
		[
			["Warn when Cap is running", "Cap 运行时提醒"],
			[
				"Open projects fail as in use. Saying so first avoids a left-behind list.",
				"打开着的项目会因占用而失败。先提醒，就不会出现一串遗留项。",
			],
			["Look for the old app's process", "检测旧程序进程"],
		],
		[
			["Try again", "再试一次"],
			[
				"The core already converts only what's left when run again.",
				"核心库再次运行时本来就只转换剩下的。",
			],
			["None in the core", "核心库不用改"],
		],
		[
			["Open FrameEver", "打开 FrameEver"],
			["The natural next step.", "最自然的下一步。"],
			["Find the installed app", "查找已安装的应用"],
		],
		[
			["Skip Choose with one install", "只有一处时跳过选择"],
			["Nothing to choose.", "没什么可选的。"],
			["None in the core", "核心库不用改"],
		],
	],
);
table(
	"ref-table",
	[
		["Source", "来源"],
		["Used for", "用在哪"],
	],
	[
		[
			["Upstream f7d351611", "上游 f7d351611"],
			[
				"No import or migration screen, so there's nothing to port.",
				"没有导入或迁移界面，无需移植。",
			],
		],
		[
			["OBS Studio scene collection import", "OBS Studio 场景集合导入"],
			[
				"Searches other apps' known locations and lists what it finds, with a checkbox per row. Read from its knowledge base and third-party guides, not its source.",
				"搜索其他应用的已知位置，列出找到的内容，每行一个复选框。依据是它的知识库和第三方教程，没有读源码。",
			],
		],
		[
			["Thunderbird import tool", "Thunderbird 导入工具"],
			[
				"Source, what to import, then a summary with Back before starting. Steps aren't clickable.",
				"来源、要导入的内容，开始前有摘要和返回。步骤不能点击。",
			],
		],
		[
			["Fluent 2 progress controls", "Fluent 2 进度控件"],
			[
				"A determinate bar when the end is known, with a line of text saying what's going on.",
				"终点已知时用确定进度条，配一行文字说明正在做什么。",
			],
		],
		[
			["Other layouts", "其他方案"],
			[
				"Sources beside contents, and every item in a list, are on the branch prototype/converter.",
				"来源和内容并排、逐项列表两个方案在 prototype/converter 分支上。",
			],
		],
	],
);

const copy = {
	"t-window": ["FrameEver Converter window", "FrameEver Converter 窗口"],
	"l-window": [
		"One step at a time: Choose, Review, Convert, Done. Click through it; the controls above it set what this PC has and how the run ends.",
		"一步一屏：选择、确认、转换、完成。可以直接点下去；上面的选项决定这台电脑上有什么、转换怎么结束。",
	],
	"t-steps": ["Every step", "所有步骤"],
	"l-steps": [
		"What each step shows and its one blue button.",
		"每一步显示什么，以及它唯一的蓝色按钮。",
	],
	"t-rules": ["Rules", "规则"],
	"l-rules": [
		"ADR-0006 settles what converts; these settle how the window behaves.",
		"ADR-0006 定了转换什么，这里定窗口的行为。",
	],
	"t-keys": ["Keyboard", "键盘"],
	"l-keys": [
		"Every action is reachable from the keyboard.",
		"所有操作都能用键盘完成。",
	],
	"t-inv": ["Everything in the converter today", "现有的全部内容"],
	"l-inv": [
		"Where each piece of today's wizard goes.",
		"现有向导里每一项的去向。",
	],
	"t-add": ["Additions", "新增"],
	"l-add": [
		"None of these exist today. Each needs a small change outside the window.",
		"这些现在都没有。每项都要在窗口之外做一点小改动。",
	],
	"t-ref": ["References", "参考"],
	"l-ref": ["What this design was checked against.", "这个设计对照过的资料。"],
};
for (const [id, pair] of Object.entries(copy))
	document.getElementById(id).innerHTML = L(...pair);

boot();
shell({
	here: "converter",
	depth: 1,
	sections: [
		["window", "Window", "窗口"],
		["steps", "Every step", "所有步骤"],
		["rules", "Rules", "规则"],
		["keyboard", "Keyboard", "键盘"],
		["inventory", "Today", "现状"],
		["additions", "Additions", "新增"],
		["references", "References", "参考"],
	],
});
onChange(render);
render();
