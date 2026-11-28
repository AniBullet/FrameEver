const { L, icon, pick, shell, onChange } = FE;

const s = { state: "available", policy: "manual" };
const NEW = "1.2.0";
const NOW = "1.1.4";
const PERCENT = 42;

const states = [
	["available", "Available", "有新版本"],
	["downloading", "Downloading", "下载中"],
	["ready", "Ready", "已就绪"],
	["installing", "Installing", "安装中"],
	["failed", "Failed", "失败"],
	["current", "Up to date", "已是最新"],
];

const ring = (percent) => {
	const r = 5.5;
	const length = 2 * Math.PI * r;
	return `<svg class="ring" viewBox="0 0 14 14" aria-hidden="true"><circle class="track" cx="7" cy="7" r="${r}"/><circle class="fill" cx="7" cy="7" r="${r}" stroke-dasharray="${(length * percent) / 100} ${length}"/></svg>`;
};

const pill = () => {
	if (s.state === "available")
		return `<button class="up-pill">${icon("download", "sm")}${L(`${NEW} available`, `${NEW} 可用`)}</button>`;
	if (s.state === "downloading")
		return `<button class="up-pill">${ring(PERCENT)}<span class="tnum">${L(`Downloading ${PERCENT}%`, `正在下载 ${PERCENT}%`)}</span></button>`;
	if (s.state === "ready")
		return `<button class="up-pill">${icon("refresh-cw", "sm")}${L("Restart to update", "重启以更新")}</button>`;
	if (s.state === "installing")
		return `<button class="up-pill">${ring(100)}${L("Installing…", "正在安装…")}</button>`;
	if (s.state === "failed")
		return `<button class="up-pill failed">${icon("triangle-alert", "sm")}${L("Update failed", "更新失败")}</button>`;
	return "";
};

const titleBar = (en, zh) =>
	`<div class="bar-row"><span class="grow">${L(en, zh)}</span>${pill()}<button class="btn ghost icon" aria-label="${pick("Settings", "设置")}">${icon("settings", "sm")}</button></div><div class="up-body"></div>`;

const trayRow = () => {
	if (s.state === "available")
		return `<button><span class="grow">${L(`Update to ${NEW}…`, `更新到 ${NEW}…`)}</span><span class="up-dot"></span></button>`;
	if (s.state === "downloading")
		return `<div class="item status">${ring(PERCENT)}<span class="grow tnum">${L(`Downloading ${NEW} · ${PERCENT}%`, `正在下载 ${NEW} · ${PERCENT}%`)}</span></div>`;
	if (s.state === "ready")
		return `<button><span class="grow">${L(`Restart to update to ${NEW}`, `重启以更新到 ${NEW}`)}</span><span class="up-dot"></span></button>`;
	if (s.state === "installing")
		return `<div class="item status">${ring(100)}<span class="grow">${L(`Installing ${NEW}…`, `正在安装 ${NEW}…`)}</span></div>`;
	if (s.state === "failed")
		return `<button><span class="grow">${L("Update failed · Try again", "更新失败 · 再试一次")}</span></button>`;
	return "";
};

const tray = () => {
	const row = trayRow();
	return `<div class="up-tray"><span class="up-tray-icon">${row && s.state !== "installing" && s.state !== "downloading" ? `<i></i>` : ""}</span><small>${L("Tray icon", "托盘图标")}</small></div><div class="menu" role="menu">${row ? `${row}<hr>` : ""}<button><span class="grow">${L("New recording", "新录制")}</span></button><button><span class="grow">${L("Open library", "打开资源库")}</span></button><button><span class="grow">${L("Settings", "设置")}</span></button></div>`;
};

const sidebar = () => {
	const badge =
		s.state === "available" || s.state === "ready" || s.state === "failed"
			? `<span class="badge klein" style="margin-left:auto">1</span>`
			: "";
	return `<div class="menu up-nav"><button><span class="grow">${L("General", "通用")}</span></button><button><span class="grow">${L("Shortcuts", "快捷键")}</span></button><button class="on"><span class="grow">${L("About", "关于")}</span>${badge}</button></div>`;
};

const toast = () =>
	`<div class="up-toast"><b>FrameEver</b><span>${
		s.state === "ready"
			? L(
					`Version ${NEW} is ready. Restart to update.`,
					`${NEW} 版本已就绪，重启即可更新。`,
				)
			: L(`Version ${NEW} is available.`, `${NEW} 版本已发布。`)
	}</span><small>${L("Click to see what's new", "点击查看更新内容")}</small></div>`;

const notes = () =>
	`<div class="up-notes"><b>${L(`What's new in ${NEW}`, `${NEW} 更新内容`)}</b><ul><li>${L("Library: open an export with the default app", "资源库：用默认应用打开导出文件")}</li><li>${L("Compression shows its queue in the library toolbar", "压缩队列显示在资源库工具栏")}</li><li>${L("Fixed: update prompt hidden behind other windows", "修复：更新提示被其他窗口挡住")}</li></ul><a class="link">${L("Full changelog", "完整更新日志")}${icon("external-link", "sm")}</a></div>`;

const panelBody = (state) => {
	if (state === "available")
		return `${notes()}<div class="up-acts"><button class="btn ghost">${L("Skip this version", "跳过此版本")}</button><span class="grow"></span><button class="btn">${L("Remind me later", "稍后提醒")}</button><button class="btn primary">${L("Download and install", "下载并安装")}</button></div>`;
	if (state === "downloading")
		return `${notes()}<div class="up-progress"><div class="line tnum"><span class="grow">${L(`Downloading · ${PERCENT}% · 38 of 91 MB`, `正在下载 · ${PERCENT}% · 38/91 MB`)}</span><button class="btn ghost">${L("Cancel", "取消")}</button></div><div class="progress"><i style="width:${PERCENT}%"></i></div></div><div class="up-acts"><span class="grow"></span><button class="btn">${L("Hide", "隐藏")}</button></div>`;
	if (state === "ready")
		return `${notes()}<div class="up-acts"><span class="hint">${L("Your recordings and projects are kept.", "录制和项目不受影响。")}</span><span class="grow"></span><button class="btn">${L("Later", "稍后")}</button><button class="btn primary">${L("Restart to update", "重启以更新")}</button></div>`;
	if (state === "installing")
		return `<div class="up-progress"><div class="line"><span class="grow">${L(`Installing ${NEW}… FrameEver reopens by itself.`, `正在安装 ${NEW}…FrameEver 会自动重新打开。`)}</span></div><div class="progress"><i style="width:100%"></i></div></div>`;
	if (state === "failed")
		return `<div class="up-error">${icon("triangle-alert", "sm")}<span>${L("Couldn't download the update. Check your connection and try again.", "无法下载更新。检查网络后再试一次。")}</span></div><div class="up-acts"><a class="link">${L("Download from GitHub", "从 GitHub 下载")}${icon("external-link", "sm")}</a><span class="grow"></span><button class="btn">${L("Close", "关闭")}</button><button class="btn primary">${L("Try again", "再试一次")}</button></div>`;
	return `<div class="up-current">${icon("circle-check", "sm")}<span>${L(`FrameEver ${NOW} is the newest version.`, `FrameEver ${NOW} 已是最新版本。`)}</span></div><div class="up-acts"><span class="grow"></span><button class="btn">${L("Close", "关闭")}</button></div>`;
};

const panel = (state) =>
	`<div class="up-panel" role="dialog"><header><b>${state === "current" ? L("FrameEver is up to date", "FrameEver 已是最新版本") : L(`FrameEver ${NEW}`, `FrameEver ${NEW}`)}</b><small class="tnum">${state === "current" ? L(`Checked just now · Stable`, `刚刚检查 · 正式版`) : L(`You have ${NOW} · Stable · 28 Nov 2026`, `当前 ${NOW} · 正式版 · 2026-11-28`)}</small><button class="btn ghost icon" aria-label="${pick("Close", "关闭")}">${icon("x", "sm")}</button></header>${panelBody(state)}</div>`;

const policies = [
	["auto", "Auto", "自动"],
	["manual", "Ask first", "先询问"],
	["never", "Never", "从不"],
];

const policyHint = {
	auto: [
		"Downloads new versions in the background, then shows Restart to update.",
		"在后台下载新版本，下载好后提示重启以更新。",
	],
	manual: [
		"Shows when a new version is out. Nothing downloads until you choose to.",
		"有新版本时提示你，你选了才下载。",
	],
	never: [
		"Doesn't check by itself. Use Check for updates.",
		"不自动检查，需要时点“检查更新”。",
	],
};

const versionAction = () => {
	if (s.state === "downloading")
		return `<span class="hint tnum">${L(`Downloading ${NEW}… ${PERCENT}%`, `正在下载 ${NEW}… ${PERCENT}%`)}</span>`;
	if (s.state === "installing")
		return `<span class="hint">${L(`Installing ${NEW}…`, `正在安装 ${NEW}…`)}</span>`;
	if (s.state === "ready")
		return `<button class="btn primary">${L("Restart to update", "重启以更新")}</button>`;
	if (s.state === "available")
		return `<button class="btn">${L(`See ${NEW}`, `查看 ${NEW}`)}</button>`;
	if (s.state === "failed")
		return `<button class="btn">${L("Try again", "再试一次")}</button>`;
	return `<button class="btn">${L("Check for updates", "检查更新")}</button>`;
};

const settingsRows = () =>
	`<div class="up-rows"><div class="up-row"><span class="grow"><b>${L("Version", "版本")}</b><small class="tnum">FrameEver ${NOW}</small></span>${versionAction()}</div><div class="up-row"><span class="grow"><b>${L("Update downloads", "更新下载")}</b><small>${L(...policyHint[s.policy])}</small></span><div class="seg">${policies.map(([v, en, zh]) => `<button class="${s.policy === v ? "on" : ""}" data-policy="${v}">${L(en, zh)}</button>`).join("")}</div></div><div class="up-row"><span class="grow"><b>${L("Update channel", "更新通道")}</b></span><div class="seg"><button class="on">${L("Stable", "正式版")}</button><button>${L("Pre-release", "预发布")}</button></div></div></div>`;

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
		`<div class="tool"><span>${L("State", "状态")}</span><div class="seg" data-tool="state">${states.map(([v, en, zh]) => `<button class="${s.state === v ? "on" : ""}" data-v="${v}">${L(en, zh)}</button>`).join("")}</div></div>`;
	const hidden =
		s.state === "current"
			? L(
					"Nothing shows when there is no update, after Remind me later, or after Skip this version",
					"没有更新、点了“稍后提醒”或“跳过此版本”后，这里什么都不显示",
				)
			: "";
	document.getElementById("place-grid").innerHTML = [
		figure(
			titleBar("FrameEver", "FrameEver"),
			"Main window title bar, left of Settings. Clicking it opens the update panel",
			"主窗口标题栏，设置按钮左边。点它打开更新面板",
		),
		figure(
			titleBar("Library", "资源库"),
			"Library title bar, the same pill",
			"资源库标题栏，同一个标签",
		),
		figure(
			tray(),
			"Tray: a dot on the icon and the first menu row while FrameEver sits in the tray",
			"托盘：FrameEver 待在托盘里时，图标上一个点，菜单第一行",
		),
		figure(
			sidebar(),
			"Settings sidebar: a count on About",
			"设置侧栏：“关于”上的数字",
		),
		figure(
			s.state === "available" || s.state === "ready"
				? toast()
				: `<span class="hint">${L("No notification in this state", "这个状态不发通知")}</span>`,
			"System notification: at most one per version, when a version is found and, with Auto, when it is ready. Clicking it opens the update panel",
			"系统通知：每个版本最多一条，发现新版本时发，“自动”模式下就绪时再发一条。点它打开更新面板",
		),
	].join("");
	if (hidden)
		document
			.getElementById("place-grid")
			.insertAdjacentHTML(
				"afterbegin",
				`<p class="hint up-span">${hidden}</p>`,
			);
	document.getElementById("panel-grid").innerHTML = states
		.map(([v, en, zh]) => figure(panel(v), en, zh))
		.join("");
	document.getElementById("settings-grid").innerHTML = figure(
		settingsRows(),
		"Settings → About. The Version button follows the state above; the description follows the policy you pick",
		"设置 → 关于。版本行的按钮跟着上面的状态变，说明文字跟着选中的下载方式变",
	);
};

const copy = () => {
	text("t-places", "Where an update shows", "更新显示在哪里");
	text(
		"l-places",
		"No dialog pops up by itself. A small pill in the title bar, a dot in the tray and a count in settings say a version is out; each opens the same update panel. Pick a state to see every place change together.",
		"不再自己弹对话框。标题栏的小标签、托盘上的点和设置里的数字告诉你有新版本，点哪个都打开同一个更新面板。选一个状态，看所有位置一起变化。",
	);
	text("t-panel", "Update panel", "更新面板");
	text(
		"l-panel",
		"One small window opened from any indicator, from the tray row or from Settings → About. It says what changed before asking anything. Download and install is the region's one primary button.",
		"从任何提示、托盘那一行或设置 → 关于打开的同一个小窗口。先说改了什么再问你。“下载并安装”是这块区域唯一的主按钮。",
	);
	text("t-settings", "Settings", "设置");
	text(
		"l-settings",
		"The three download policies stay; what changes is what each one means and says. Pick one to read its description.",
		"三档下载方式保留，改的是每档的含义和说明文字。选一档看它的说明。",
	);
	table(
		"policy-table",
		[
			["Policy", "方式"],
			["Scheduled check", "定时检查"],
			["When a version is found", "发现新版本时"],
			["When it is downloaded", "下载好后"],
		],
		[
			[
				["Auto", "自动"],
				["Yes", "是"],
				[
					"Downloads in the background; the pill shows the progress",
					"后台下载，标签显示进度",
				],
				[
					"Pill, tray and one notification say Restart to update",
					"标签、托盘和一条通知提示重启以更新",
				],
			],
			[
				["Ask first (default)", "先询问（默认）"],
				["Yes", "是"],
				[
					"Pill, tray, count and one notification. Nothing downloads",
					"标签、托盘、数字和一条通知，不下载",
				],
				["Pill and tray say Restart to update", "标签和托盘提示重启以更新"],
			],
			[
				["Never", "从不"],
				["No", "否"],
				[
					"Only after Check for updates, which opens the panel",
					"只在点“检查更新”后出现，直接打开面板",
				],
				["As Ask first", "同“先询问”"],
			],
		],
	);
	text("t-rules", "Behaviour", "行为");
	text("l-rules", "What the pictures cannot show.", "图上表现不出来的部分。");
	table(
		"rule-table",
		[
			["Case", "情况"],
			["What happens", "结果"],
		],
		[
			[
				["Remind me later", "稍后提醒"],
				[
					"The pill, dot and count go away until the next launch or the next scheduled check, whichever comes first",
					"标签、点和数字消失，到下次启动或下次定时检查（先到者）再出现",
				],
			],
			[
				["Skip this version", "跳过此版本"],
				[
					"That version number is saved and never shown again by a scheduled check. A newer version shows as usual. Check for updates still shows a skipped version, with Skip this version replaced by a note that it was skipped",
					"记住这个版本号，定时检查不再提示它；更新的版本照常提示。手动“检查更新”仍会显示被跳过的版本，“跳过此版本”换成一行“已跳过”的说明",
				],
			],
			[
				["Check for updates", "检查更新"],
				[
					"Always opens the panel with the result, also Up to date and Failed, so a press is never silent",
					"总是打开面板显示结果，包括“已是最新”和“失败”，按了一定有回应",
				],
			],
			[
				[
					"A recording, export, import or transcription is running",
					"正在录制、导出、导入或转写",
				],
				[
					"The pill still shows; the notification waits until the work ends. Restart to update waits too and says why",
					"标签照常显示；通知等工作结束再发。重启以更新也等，并说明原因",
				],
			],
			[
				["The panel's window is closed while downloading", "下载中关掉面板"],
				["The download goes on; the pill shows it", "下载继续，标签显示进度"],
			],
			[
				["The update channel changes", "切换更新通道"],
				[
					"A download or a ready package from the other channel is dropped, as today; the skipped version is kept per channel",
					"另一个通道的下载或已就绪的安装包作废（同现在）；跳过的版本按通道分别记住",
				],
			],
			[
				["Install when FrameEver quits", "退出时安装"],
				[
					"Not in this design. On Windows the installer can ask for administrator rights, which must not appear while quitting. Restart to update stays the only way to install",
					"这版不做。Windows 安装器可能要管理员确认，不能在退出时冒出来。安装只走“重启以更新”",
				],
			],
			[
				[
					"Why the old prompt seemed to say it downloads by itself (owner report)",
					"为什么旧提示看起来像会自动下载（问题反馈）",
				],
				[
					'With Ask first, a scheduled check opened a system dialog (apps/desktop-gpui/src/updates.rs:546) with no parent window, and on Windows bringing the app forward is a no-op (platform/windows.rs:770). While FrameEver sat in the tray the dialog landed behind other windows and only appeared when the settings window brought the app forward. Its text, "FrameEver will download it in the background…", read as if the download had already started. This design removes that dialog',
					"“先询问”模式下，定时检查弹出一个没有父窗口的系统对话框（apps/desktop-gpui/src/updates.rs:546），Windows 上“把应用拉到前台”是空操作（platform/windows.rs:770）。FrameEver 待在托盘里时，对话框落在其他窗口后面，等打开设置窗口把应用带到前台才露出来。它的文字“帧刻会在后台下载……”读起来像已经开始下载。这个设计去掉了这个对话框",
				],
			],
		],
	);
	text("t-inv", "Where everything went", "功能位置");
	text(
		"l-inv",
		"Every update feature and its place.",
		"每个更新相关功能所在的位置。",
	);
	table(
		"inv-table",
		[
			["Feature", "功能"],
			["Before", "原来"],
			["Now", "现在"],
		],
		[
			[
				["A new version is found", "发现新版本"],
				[
					"Modal system dialog (Download / Not now), often hidden",
					"模态系统对话框（下载 / 以后再说），常被挡住",
				],
				[
					"Title bar pill, tray dot and row, About count, one notification",
					"标题栏标签、托盘点和菜单行、“关于”数字、一条通知",
				],
			],
			[
				["What changed", "更新内容"],
				[
					"Settings → About → Changelog, after updating",
					"更新后在设置 → 关于 → 更新日志",
				],
				["Update panel, before downloading", "下载前在更新面板里"],
			],
			[
				["Skip a version", "跳过某个版本"],
				["Not now, forgotten on the next launch", "“以后再说”，下次启动就忘了"],
				["Update panel · Skip this version", "更新面板 · 跳过此版本"],
			],
			[
				["Check for updates", "检查更新"],
				["Settings → About → Version", "设置 → 关于 → 版本"],
				[
					"Same place; the result opens the update panel",
					"同一位置，结果打开更新面板",
				],
			],
			[
				["Download progress", "下载进度"],
				[
					"Settings → About → Version, and the settings sidebar",
					"设置 → 关于 → 版本，设置侧栏",
				],
				[
					"Pill, tray row, update panel, Settings → About",
					"标签、托盘行、更新面板、设置 → 关于",
				],
			],
			[
				["Restart to update", "重启以更新"],
				["Settings → About, a dialog when ready", "设置 → 关于，就绪时弹框"],
				[
					"Pill, tray row, update panel, Settings → About",
					"标签、托盘行、更新面板、设置 → 关于",
				],
			],
			[
				["Download policy", "下载方式"],
				["Settings → About → Update downloads", "设置 → 关于 → 更新下载"],
				["Same place, clearer descriptions", "同一位置，说明更清楚"],
			],
			[
				["Update channel", "更新通道"],
				["Settings → About → Update channel", "设置 → 关于 → 更新通道"],
				["Unchanged", "不变"],
			],
			[
				["Update failed", "更新失败"],
				["An alert dialog", "警告对话框"],
				[
					"Pill and tray say so; the panel offers Try again and Download from GitHub",
					"标签和托盘显示失败，面板里可以再试一次或从 GitHub 下载",
				],
			],
		],
	);
	text("t-ref", "References", "参考");
	text("l-ref", "Where each choice comes from.", "每个选择的出处。");
	table(
		"ref-table",
		[
			["Choice", "选择"],
			["From", "出处"],
		],
		[
			[
				[
					"No modal for background apps; reminders in the app's own UI and the tray",
					"后台应用不弹模态框，提示放在应用自己的界面和托盘里",
				],
				[
					'Sparkle, <a class="link" href="https://sparkle-project.org/documentation/gentle-reminders/">Gentle update reminders</a>: scheduled alerts of background apps show behind other windows, so it adds a title bar button and a badge',
					'Sparkle <a class="link" href="https://sparkle-project.org/documentation/gentle-reminders/">Gentle update reminders</a>：后台应用的定时提示会落在其他窗口后面，所以加标题栏按钮和角标',
				],
			],
			[
				[
					"Skip this version, Remind me later, Install, with release notes",
					"跳过此版本、稍后提醒、安装，并附更新说明",
				],
				[
					"Sparkle's standard update alert; WinSparkle on Windows",
					"Sparkle 的标准更新弹窗；Windows 上的 WinSparkle",
				],
			],
			[
				[
					"A small Update available pill instead of a dialog",
					"用“有可用更新”小标签代替对话框",
				],
				[
					'Rogue Amoeba, <a class="link" href="https://weblog.rogueamoeba.com/2026/04/28/unobtrusive-update-notifications/">Unobtrusive update notifications</a> (28 April 2026)',
					'Rogue Amoeba <a class="link" href="https://weblog.rogueamoeba.com/2026/04/28/unobtrusive-update-notifications/">Unobtrusive update notifications</a>（2026-04-28）',
				],
			],
			[
				["Policies and Restart to update", "下载方式和“重启以更新”"],
				[
					'VS Code <a class="link" href="https://code.visualstudio.com/docs/enterprise/updates">update.mode</a> and the badge on its settings gear; Zed\'s title bar update button',
					'VS Code 的 <a class="link" href="https://code.visualstudio.com/docs/enterprise/updates">update.mode</a> 和设置齿轮上的角标；Zed 标题栏的更新按钮',
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
	if (option) {
		s.state = option.dataset.v;
		paint();
		return;
	}
	const policy = event.target.closest("[data-policy]");
	if (policy) {
		s.policy = policy.dataset.policy;
		paint();
	}
});

shell({
	here: "updates",
	depth: 1,
	sections: [
		["places", "Where it shows", "显示位置"],
		["panel", "Update panel", "更新面板"],
		["settings", "Settings", "设置"],
		["behaviour", "Behaviour", "行为"],
		["inventory", "Where everything went", "功能位置"],
		["references", "References", "参考"],
	],
});
renderAll();
onChange(renderAll);
