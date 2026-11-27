const { L, icon, pick, shell, onChange } = FE;

const s = { percent: 42 };

const ring = (percent) => {
	const r = 5.5;
	const length = 2 * Math.PI * r;
	return `<svg class="ring" viewBox="0 0 14 14" aria-hidden="true"><circle class="track" cx="7" cy="7" r="${r}"/><circle class="fill" cx="7" cy="7" r="${r}" stroke-dasharray="${(length * percent) / 100} ${length}"/></svg>`;
};

const job = (state, extra = "") => {
	if (state === "running")
		return `<span class="job running ${extra}">${ring(s.percent)}<span class="tnum">${L(`Compressing ${s.percent}%`, `正在压缩 ${s.percent}%`)}</span></span>`;
	if (state === "waiting")
		return `<span class="job waiting ${extra}">${ring(0)}${L("Waiting", "排队中")}</span>`;
	if (state === "done")
		return `<span class="job done ${extra}">${icon("circle-check", "sm")}${L("64% smaller", "小了 64%")}</span>`;
	return `<span class="job failed ${extra}">${icon("triangle-alert", "sm")}${L("Couldn't compress", "压缩失败")}</span>`;
};

const tile = (state) =>
	`<div class="tile"><div class="thumb"><span class="scene"></span>${job(state, "on-picture")}${state === "running" || state === "waiting" ? `<div class="job-bar"><b style="width:${state === "running" ? s.percent : 0}%"></b></div>` : ""}<span class="corner br tnum">0:48</span></div><span class="name">${L("Bug report", "问题复现")}</span><span class="meta tnum">${state === "running" ? L(`Instant · Compressing ${s.percent}%`, `即时 · 正在压缩 ${s.percent}%`) : state === "waiting" ? L("Instant · Waiting", "即时 · 排队中") : L("Instant · 24 MB", "即时 · 24 MB")}</span></div>`;

const toolbar = (open) =>
	`<div class="bar-row"><span class="grow">${L("Library", "资源库")}</span><button class="btn ghost" style="padding:0">${job("running")}</button></div>${
		open
			? `<div class="menu queue-pop" role="dialog">
		<div class="item"><div class="line"><span class="grow">${L("Bug report", "问题复现")}</span><small class="tnum">${s.percent}%</small><button class="btn ghost icon" title="${pick("Cancel", "取消")}">${icon("x", "sm")}</button></div><div class="progress"><i style="width:${s.percent}%"></i></div><small>${L("Automatic, after recording", "录制后自动压缩")}</small></div>
		<div class="item"><div class="line"><span class="grow">${L("Login flow", "登录流程")}</span><small>${L("Waiting", "排队中")}</small><button class="btn ghost icon" title="${pick("Cancel", "取消")}">${icon("x", "sm")}</button></div></div>
		<hr><button><span class="grow">${L("Cancel all", "全部取消")}</span></button></div>`
			: ""
	}`;

const detail = (state) => {
	if (state === "idle")
		return `<div class="detail-row"><div class="line"><span class="grow"><b>${L("Compressed copy", "压缩副本")}</b><small>${L("Not made yet", "还没有")}</small></span><button class="btn">${L("Compress", "压缩")}</button></div></div>`;
	if (state === "running")
		return `<div class="detail-row"><div class="line"><span class="grow"><b>${L("Compressed copy", "压缩副本")}</b><small class="tnum">${L(`Compressing ${s.percent}% · about 1 min left`, `正在压缩 ${s.percent}% · 约剩 1 分钟`)}</small></span><button class="btn">${L("Cancel", "取消")}</button></div><div class="progress"><i style="width:${s.percent}%"></i></div></div>`;
	return `<div class="detail-row"><div class="line"><span class="grow"><b>${L("Compressed copy", "压缩副本")}</b><small>${L("Waiting for 1 recording", "排在 1 个录制后面")}</small></span><button class="btn">${L("Cancel", "取消")}</button></div><div class="progress"><i style="width:0%"></i></div></div>`;
};

const card = (state) => {
	const body =
		state === "ready"
			? `<div class="rc-acts"><button class="btn">${icon("play", "sm")}${L("Open", "打开")}</button><button class="btn">${icon("copy", "sm")}${L("Copy", "复制")}</button><button class="btn ghost icon" title="${pick("Compress", "压缩")}" aria-label="${pick("Compress", "压缩")}">${icon("minimize", "sm")}</button><button class="btn ghost icon" title="${pick("Show in folder", "在文件夹中显示")}">${icon("folder-open", "sm")}</button></div>`
			: state === "running"
				? `<div class="rc-status"><span class="grow tnum">${L(`Compressing ${s.percent}%`, `正在压缩 ${s.percent}%`)}</span><button class="btn ghost">${L("Cancel", "取消")}</button></div><div class="progress"><i style="width:${s.percent}%"></i></div>`
				: `<div class="rc-status good">${icon("circle-check", "sm")}<span class="grow">${L("Compressed · 64% smaller", "已压缩 · 小了 64%")}</span><button class="btn ghost">${L("Copy", "复制")}</button></div>`;
	return `<div class="rc" role="status"><div class="rc-thumb"><div class="scene"></div><span class="rc-chip">0:48</span><button class="rc-close" aria-label="${pick("Close", "关闭")}">${icon("x", "sm")}</button></div><div class="rc-body"><div class="rc-name"><b>${L("Bug report", "问题复现")}</b><span>0:48 · 24 MB</span></div>${body}</div></div>`;
};

const tray = () =>
	`<div class="menu" role="menu"><div class="item status">${ring(s.percent)}<span class="grow tnum">${L(`Compressing 1 of 2 · ${s.percent}%`, `正在压缩 1/2 · ${s.percent}%`)}</span></div><hr><button><span class="grow">${L("New recording", "新录制")}</span></button><button><span class="grow">${L("Open library", "打开资源库")}</span></button></div>`;

const table = (id, head, list) => {
	const cell = (c) => (Array.isArray(c) ? L(...c) : c);
	document.getElementById(id).innerHTML =
		`<table class="inv"><thead><tr>${head.map((h) => `<th>${L(...h)}</th>`).join("")}</tr></thead><tbody>${list.map((r) => `<tr>${r.map((c) => `<td>${cell(c)}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
};
const text = (id, en, zh) => {
	document.getElementById(id).innerHTML = L(en, zh);
};

const figure = (html, en, zh) =>
	`<figure><div class="place">${html}</div><figcaption>${L(en, zh)}</figcaption></figure>`;

const paint = () => {
	document.getElementById("tools").innerHTML =
		`<div class="tool"><span>${L("Progress", "进度")}</span><div class="seg" data-tool="percent">${[
			[8, "8%"],
			[42, "42%"],
			[91, "91%"],
		]
			.map(
				([v, label]) =>
					`<button class="${s.percent === v ? "on" : ""}" data-v="${v}">${label}</button>`,
			)
			.join("")}</div></div>`;
	document.getElementById("marker-row").innerHTML = [
		["running", "Running", "进行中"],
		["waiting", "Waiting", "排队"],
		["done", "Finished", "完成"],
		["failed", "Failed", "失败"],
	]
		.map(
			([state, en, zh]) =>
				`<figure>${job(state)}<figcaption>${L(en, zh)}</figcaption></figure>`,
		)
		.join("");
	document.getElementById("place-grid").innerHTML = [
		figure(
			tile("running"),
			"Library tile: the marker sits on the picture, the bar runs along its foot",
			"资源库卡片：标记压在缩略图上，进度条贴底",
		),
		figure(
			tile("waiting"),
			"Library tile, waiting its turn",
			"资源库卡片，排队中",
		),
		figure(
			toolbar(false),
			"Library toolbar while anything compresses, also automatic ones",
			"有任何压缩在跑（包括自动压缩）时，资源库工具栏右侧",
		),
		figure(
			`${toolbar(true)}<div style="height:200px"></div>`,
			"Clicking it lists the queue; each job can be cancelled",
			"点开是队列，每项都能取消",
		),
		figure(
			detail("running"),
			"Detail panel while it compresses",
			"详情栏，压缩中",
		),
		figure(detail("waiting"), "Detail panel, waiting", "详情栏，排队中"),
		figure(
			card("running"),
			"Result card after an Instant recording with automatic compression on",
			"开了录制后自动压缩：Instant 录制后的结果卡片",
		),
		figure(
			card("ready"),
			"Automatic compression off: Compress is one of the card's buttons",
			"没开自动压缩：结果卡片多一个“压缩”按钮",
		),
		figure(
			card("done"),
			"When it finishes, the card says how much it saved",
			"压缩完成，卡片上说明省了多少",
		),
		figure(
			tray(),
			"Tray menu, first row while the queue runs",
			"托盘菜单，队列在跑时的第一行",
		),
	].join("");
};

const copy = () => {
	text("t-marker", "One marker", "同一个标记");
	text(
		"l-marker",
		"Every place that shows a compression uses this marker: a filled ring and the percentage while it runs, an empty ring while it waits. The bar under a picture or a row is the same progress, never a different number.",
		"所有显示压缩的地方都用这个标记：进行中是实心进度环加百分比，排队是空环。缩略图或行下面的进度条是同一个进度，数字永远一致。",
	);
	text("t-places", "Where it shows", "显示位置");
	text(
		"l-places",
		"Automatic and manual compressions are one queue, so each place shows both. The result card and the tray are the places that are on screen while the library is closed.",
		"自动和手动压缩是同一个队列，所以每个位置都同时显示两者。资源库没开时，结果卡片和托盘是能看到进度的地方。",
	);
	text("t-entries", "Where to start one", "从哪里开始压缩");
	text(
		"l-entries",
		"Compression applies to Instant recordings that have no compressed copy yet. Every entry adds to the same queue.",
		"压缩只适用于还没有压缩副本的 Instant 录制。每个入口都加进同一个队列。",
	);
	table(
		"entry-table",
		[
			["Entry", "入口"],
			["What it compresses", "压缩什么"],
			["Status", "状态"],
		],
		[
			[
				["After an Instant recording, automatically", "Instant 录制后自动"],
				["The new recording (Settings → Instant)", "新录制（设置 → Instant）"],
				["Exists", "已有"],
			],
			[
				["Result card · Compress", "结果卡片 · 压缩"],
				["The recording just made", "刚录完的录制"],
				["New", "新增"],
			],
			[
				["Library detail panel · Compress", "资源库详情栏 · 压缩"],
				["The selected recording", "选中的录制"],
				["Exists", "已有"],
			],
			[
				["Library right-click · Compress", "资源库右键 · 压缩"],
				["Every selected Instant recording", "选中的所有 Instant 录制"],
				["Exists", "已有"],
			],
			[
				["Library batch panel · Compress n", "资源库批量面板 · 压缩 n 项"],
				["Every selected Instant recording", "选中的所有 Instant 录制"],
				["Exists", "已有"],
			],
			[
				["Command palette · Compress selected", "命令面板 · 压缩所选"],
				["The library selection", "资源库里选中的"],
				["New", "新增"],
			],
			[
				["Library folder or smart folder, right-click · Compress all", "资源库文件夹或智能文件夹右键 · 全部压缩"],
				[
					"Every recording in the folder, such as Instant recordings over 100 MB",
					"文件夹里的全部录制，例如大于 100 MB 的 Instant 录制",
				],
				["New", "新增"],
			],
			[
				["CLI · frameever compress", "命令行 · frameever compress"],
				["Recordings named on the command line", "命令行给出的录制"],
				["New", "新增"],
			],
		],
	);
	text("t-rules", "Behaviour", "行为");
	text(
		"l-rules",
		"What the pictures cannot show.",
		"图上表现不出来的部分。",
	);
	table(
		"rule-table",
		[
			["Case", "情况"],
			["What happens", "结果"],
		],
		[
			[
				[
					"Compress is pressed on a recording that is already compressing or waiting",
					"对正在压缩或排队中的录制再点压缩",
				],
				[
					"Nothing new starts; the button shows the running job. Before 1.1.4 this broke the recording's versions (#258)",
					"不会再开一个，按钮显示已有的那个任务。1.1.4 之前这样会把录制的版本弄坏（#258）",
				],
			],
			[
				[
					"Keep only the compressed version is on",
					"开了“只保留压缩版”",
				],
				[
					"The original goes in the same job, so no place ever shows half of it done",
					"删原版属于同一个任务，任何地方都不会显示做了一半的状态",
				],
			],
			[
				["Cancel", "取消"],
				[
					"The half-written copy is deleted; the original is untouched",
					"删掉写了一半的副本，原版不动",
				],
			],
			[
				["The library window closes", "关掉资源库窗口"],
				[
					"The queue keeps going; the tray shows it",
					"队列继续，托盘上能看到",
				],
			],
			[
				["The queue finishes", "队列跑完"],
				[
					"One notification for the whole queue, or a dialog for failures when notifications are off",
					"整个队列一条通知；关了通知时只在失败时弹框",
				],
			],
			[
				["A recording starts", "开始录制"],
				[
					"The queue pauses and continues when the recording ends, so the recording gets the processor",
					"队列暂停，录制结束后继续，让录制优先用处理器",
				],
			],
		],
	);
	text("t-inv", "Where everything went", "功能位置");
	text(
		"l-inv",
		"Every compression feature and its place.",
		"每个压缩相关功能所在的位置。",
	);
	table(
		"inv-table",
		[
			["Feature", "功能"],
			["Place", "位置"],
		],
		[
			[
				["Progress of one recording", "单个录制的进度"],
				["Library tile and list row, detail panel, result card", "资源库卡片和列表行、详情栏、结果卡片"],
			],
			[
				["Progress of the whole queue", "整个队列的进度"],
				["Library toolbar marker and its popover, tray menu", "资源库工具栏标记和弹出层、托盘菜单"],
			],
			[
				["Switch between original and compressed", "原版和压缩版切换"],
				["Library detail panel · Version", "资源库详情栏 · 版本"],
			],
			[
				["Delete one of the two versions", "删除其中一个版本"],
				["Delete dialog's three-way choice", "删除弹框的三选一"],
			],
			[
				["Compress automatically after recording", "录制后自动压缩"],
				["Settings → Instant", "设置 → Instant"],
			],
		],
	);
	text("t-ref", "References", "参考");
	text(
		"l-ref",
		"Where each choice comes from.",
		"每个选择的出处。",
	);
	table(
		"ref-table",
		[
			["Choice", "选择"],
			["From", "出处"],
		],
		[
			[
				["Progress drawn on the item itself", "进度画在条目本身上"],
				[
					"Windows Explorer and macOS Finder copy progress on file icons",
					"Windows 资源管理器和 macOS Finder 在文件图标上显示复制进度",
				],
			],
			[
				["One activity marker that opens the queue", "一个活动标记，点开是队列"],
				[
					"Shotcut's Jobs panel; HandBrake's queue; browser download buttons",
					"Shotcut 的任务面板；HandBrake 的队列；浏览器的下载按钮",
				],
			],
			[
				["Compress straight from the after-recording card", "录制后的卡片上直接压缩"],
				[
					"ShareX's after-capture tasks",
					"ShareX 的截图后任务",
				],
			],
		],
	);
};

const renderAll = () => {
	copy();
	paint();
};

document.addEventListener("click", (event) => {
	const option = event.target.closest("[data-tool] [data-v]");
	if (!option) return;
	s.percent = Number(option.dataset.v);
	paint();
});

shell({
	here: "compression",
	depth: 1,
	sections: [
		["marker", "Marker", "标记"],
		["places", "Where it shows", "显示位置"],
		["entries", "Where to start", "入口"],
		["behaviour", "Behaviour", "行为"],
		["inventory", "Where everything went", "功能位置"],
		["references", "References", "参考"],
	],
});
renderAll();
onChange(renderAll);
