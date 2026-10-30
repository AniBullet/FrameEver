const { L, icon, pick } = FE;

Object.assign(window.FRAMEEVER_ICONS, {
	captions: '<rect width="18" height="14" x="3" y="5" rx="2" ry="2"/><path d="M7 15h4M15 15h2M7 11h2M13 11h4"/>',
	workflow: '<rect width="8" height="8" x="3" y="3" rx="2"/><path d="M7 11v4a2 2 0 0 0 2 2h4"/><rect width="8" height="8" x="13" y="13" rx="2"/>',
	terminal: '<polyline points="4 17 10 11 4 5"/><line x1="12" x2="20" y1="19" y2="19"/>',
});

const v = {
	theme: "system",
	lang: "system",
	notify: true,
	mainStart: "close",
	afterDelete: "nothing",
	instantRes: "1080",
	studioQ: "balanced",
	fps: "60",
	countdown: "0",
	confirmMic: true,
	cursor: true,
	keys: true,
	crash: true,
	afterShot: "edit",
	afterStudio: "edit",
	afterInstant: "library",
	studioSound: true,
	autoZoom: false,
	zoom: 1.5,
	compress: false,
	keepCompressed: false,
	template: "{recording_mode} {date} {time}",
	channel: "stable",
	muxer: false,
	hotkeys: {
		shotArea: ["Ctrl", "Shift", "A"],
		shotWindow: [],
		shotDisplay: [],
		recArea: ["Ctrl", "Shift", "R"],
		recWindow: [],
		recDisplay: [],
		studio: [],
		instant: [],
		picker: [],
		cycle: [],
		stop: ["Ctrl", "Shift", "S"],
		pause: ["Ctrl", "Shift", "P"],
		restart: [],
	},
	terms: ["FrameEver", "HEVC", "帧刻"],
	excluded: ["FrameEver camera", "FrameEver recording controls", "FrameEver settings"],
};

const shot = [["edit", "Open editor", "打开编辑器"], ["copy", "Copy", "复制"], ["both", "Copy and edit", "复制并编辑"], ["none", "Do nothing", "不做任何操作"]];

const sections = [
	{
		id: "general", icon: "settings", en: "General", zh: "通用", sumEn: "Theme, language, notifications", sumZh: "主题、语言、通知",
		groups: [
			{ en: "Appearance", zh: "外观", rows: [
				{ k: "theme", en: "Theme", zh: "主题", c: "theme", was: ["General · Appearance", "通用 · 外观"], kit: "radio::RadioGroup + card body" },
				{ k: "lang", en: "Language", zh: "界面语言", den: "The tray menu and window titles switch after a restart.", dzh: "托盘菜单和窗口标题在重启后切换。", c: "seg", opts: [["system", "System", "跟随系统"], ["en", "English", "English"], ["zh", "简体中文", "简体中文"]], was: ["General · Language", "通用 · 语言"], kit: "tab::TabBar segmented" },
			] },
			{ en: "App", zh: "应用", rows: [
				{ k: "notify", en: "Notifications", zh: "系统通知", den: "Saved files, clipboard copies and finished compression.", dzh: "保存文件、复制到剪贴板、压缩完成时提醒。", c: "switch", was: ["General · App", "通用 · 应用"], kit: "switch" },
				{ k: "mainStart", en: "When recording starts", zh: "开始录制时", den: "What the main window does.", dzh: "主窗口如何处理。", c: "select", opts: [["close", "Close", "关闭"], ["minimise", "Minimise", "最小化"]], was: ["General · Recording", "通用 · 录制"], kit: "select" },
				{ k: "afterDelete", en: "After deleting a recording", zh: "删除录制后", c: "select", opts: [["nothing", "Do nothing", "不做任何操作"], ["reopen", "Reopen recorder", "重新打开录制窗口"]], was: ["General · Recording", "通用 · 录制"], kit: "select" },
			] },
		],
	},
	{
		id: "recording", icon: "video", en: "Recording", zh: "录制", sumEn: "Quality, countdown, what's captured", sumZh: "画质、倒计时、捕获内容",
		groups: [
			{ en: "Quality", zh: "画质", rows: [
				{ k: "studioQ", en: "Studio quality", zh: "工作室画质", c: "tiers", opts: [["balanced", "Balanced", "均衡", "Sharp at a sensible size. Recommended.", "清晰且大小适中，推荐。"], ["small", "Smaller files", "更小的文件", "Saves disk space; fine detail can soften.", "省空间，细节可能变软。"], ["max", "Maximum detail", "最高细节", "For heavy editing; uses the most space.", "适合大量后期，最占空间。"]], was: ["Recording quality", "录制画质"], kit: "radio::RadioGroup + card body" },
				{ k: "instantRes", en: "Instant resolution", zh: "即时录制分辨率", den: "The most Instant recordings are saved at.", dzh: "即时录制保存时的最高分辨率。", c: "seg", opts: [["720", "720p", "720p"], ["1080", "1080p", "1080p"], ["1440", "1440p", "1440p"], ["2160", "4K", "4K"]], was: ["Recording quality", "录制画质"], kit: "tab::TabBar segmented" },
				{ k: "fps", en: "Maximum frame rate", zh: "最高帧率", den: "Higher rates can drop frames on some PCs.", dzh: "部分电脑上较高帧率可能掉帧。", c: "select", opts: [["24", "24 FPS", "24 帧/秒"], ["30", "30 FPS", "30 帧/秒"], ["60", "60 FPS", "60 帧/秒"], ["120", "120 FPS", "120 帧/秒"]], was: ["General · Recording", "通用 · 录制"], kit: "select" },
			] },
			{ en: "Before you record", zh: "开始之前", rows: [
				{ k: "countdown", en: "Countdown", zh: "倒计时", c: "select", opts: [["0", "Off", "关闭"], ["3", "3 seconds", "3 秒"], ["5", "5 seconds", "5 秒"], ["10", "10 seconds", "10 秒"]], was: ["General · Recording", "通用 · 录制"], kit: "select" },
				{ k: "confirmMic", en: "Confirm without microphone", zh: "无麦克风时确认", den: "Ask first if no microphone is selected or available.", dzh: "未选麦克风或麦克风不可用时先询问。", c: "switch", was: ["General · Recording", "通用 · 录制"], kit: "switch" },
			] },
			{ en: "Captured with the video", zh: "随视频捕获", rows: [
				{ k: "cursor", en: "Separate cursor", zh: "单独捕获光标", den: "Studio only. Adjust size and smoothing in the editor.", dzh: "仅工作室。可在编辑器调整大小和平滑度。", c: "switch", was: ["General · Recording", "通用 · 录制"], kit: "switch" },
				{ k: "keys", en: "Key presses", zh: "按键", den: "Lets you add keyboard overlays in the editor.", dzh: "用于在编辑器添加键盘叠加层。", c: "switch", was: ["General · Recording", "通用 · 录制"], kit: "switch" },
				{ k: "crash", en: "Crash recovery", zh: "崩溃恢复", den: "Records in fragments, slightly larger while capturing.", dzh: "分段录制，录制期间文件略大。", c: "switch", was: ["General · Recording", "通用 · 录制"], kit: "switch" },
			] },
			{ en: "Hidden from recordings", zh: "在录制中隐藏", rows: [
				{ k: "excluded", en: "Excluded windows", zh: "排除的窗口", c: "excluded", stack: true, was: ["General · Excluded windows", "通用 · 排除的窗口"], kit: "setting + list + button" },
			] },
			{ en: "When it's done", zh: "完成之后", rows: [
				{ k: "afterShot", en: "After a screenshot", zh: "截图后", c: "select", opts: shot, was: ["Shortcuts · outcome panel", "快捷键 · 结果面板"], kit: "select" },
				{ k: "afterStudio", en: "After a Studio recording", zh: "工作室录制后", c: "select", opts: [["edit", "Open editor", "打开编辑器"], ["show", "Show FrameEver", "显示帧刻"]], was: ["Shortcuts · outcome panel", "快捷键 · 结果面板"], kit: "select" },
				{ k: "afterInstant", en: "After an Instant recording", zh: "即时录制后", c: "select", opts: [["library", "Show in library", "在资源库中显示"], ["copy", "Copy file", "复制文件"], ["none", "Do nothing", "不做任何操作"]], was: ["Shortcuts · outcome panel (share link, unused)", "快捷键 · 结果面板（分享链接，已无效）"], kit: "select", fresh: true },
			] },
		],
	},
	{
		id: "editing", icon: "pencil", en: "Editing", zh: "编辑", sumEn: "Defaults for new projects, captions", sumZh: "新项目默认值、字幕",
		groups: [
			{ en: "New Studio projects", zh: "新建工作室项目", rows: [
				{ k: "studioSound", en: "Studio Sound", zh: "录音棚音质", den: "Clean up microphone audio. Turn it off per project.", dzh: "自动清理麦克风音频，可按项目关闭。", c: "switch", was: ["General · Recording", "通用 · 录制"], kit: "switch" },
				{ k: "autoZoom", en: "Auto zoom on clicks", zh: "点击时自动缩放", den: "Adds zoom segments around mouse clicks.", dzh: "围绕鼠标点击添加缩放片段。", c: "switch", was: ["General · Recording", "通用 · 录制"], kit: "switch" },
				{ k: "zoom", en: "Zoom amount", zh: "缩放倍数", den: "Also used for zoom segments you add yourself.", dzh: "手动添加的缩放片段也用这个倍数。", c: "slider", was: ["General · Recording", "通用 · 录制"], kit: "slider::Slider" },
			] },
			{ en: "Captions", zh: "字幕", rows: [
				{ k: "terms", en: "Remembered terms", zh: "记住的词条", den: "Names and jargon Whisper should spell your way.", dzh: "让 Whisper 按你的写法拼写的名称和术语。", c: "terms", stack: true, was: ["Transcription", "转写"], kit: "input + tag" },
			] },
		],
	},
	{
		id: "storage", icon: "folder", en: "Storage", zh: "存储", sumEn: "Storage folder, names, compression", sumZh: "存储文件夹、命名、压缩",
		groups: [
			{ en: "Storage folder", zh: "存储文件夹", rows: [
				{ k: "folder", en: "Storage folder", zh: "存储文件夹", c: "folder", stack: true, was: ["General · Storage", "通用 · 存储"], kit: "setting + button; native folder picker" },
			] },
			{ en: "Default project name", zh: "默认项目名称", rows: [
				{ k: "template", en: "Name template", zh: "名称模板", c: "template", stack: true, was: ["General · Default project name", "通用 · 默认项目名称"], kit: "input + tag" },
			] },
			{ en: "Compression", zh: "压缩", rows: [
				{ k: "compress", en: "Compress Instant recordings", zh: "自动压缩即时录制", den: "Makes an HEVC copy about half the size. The original stays.", dzh: "生成约一半大小的 HEVC 副本，原版保留。", c: "switch", was: ["General · Recording", "通用 · 录制"], kit: "switch" },
				{ k: "keepCompressed", en: "Keep only the copy", zh: "只保留压缩版", den: "Deletes the original to save space.", dzh: "删除原版以节省空间。", c: "switch", dep: "compress", was: ["General · Recording", "通用 · 录制"], kit: "switch" },
			] },
		],
	},
	{
		id: "shortcuts", icon: "keyboard", en: "Shortcuts", zh: "快捷键", sumEn: "Work from any app", sumZh: "在任何应用中可用",
		groups: [
			{ en: "Screenshots", zh: "截图", rows: [
				{ k: "shotArea", en: "Screenshot an area", zh: "截取区域", c: "hotkey" },
				{ k: "shotWindow", en: "Screenshot a window", zh: "截取窗口", c: "hotkey" },
				{ k: "shotDisplay", en: "Screenshot the display", zh: "截取显示器", c: "hotkey" },
			] },
			{ en: "Start recording", zh: "开始录制", rows: [
				{ k: "recArea", en: "Record an area", zh: "录制区域", c: "hotkey" },
				{ k: "recWindow", en: "Record a window", zh: "录制窗口", c: "hotkey" },
				{ k: "recDisplay", en: "Record a display", zh: "录制显示器", c: "hotkey" },
				{ k: "studio", en: "Start Studio", zh: "开始工作室录制", c: "hotkey" },
				{ k: "instant", en: "Start Instant", zh: "开始即时录制", c: "hotkey" },
				{ k: "picker", en: "Open picker", zh: "打开录制选择器", c: "hotkey" },
				{ k: "cycle", en: "Cycle mode", zh: "切换录制模式", c: "hotkey" },
			] },
			{ en: "While recording", zh: "录制时", rows: [
				{ k: "stop", en: "Stop", zh: "停止录制", c: "hotkey" },
				{ k: "pause", en: "Pause or resume", zh: "暂停或继续", c: "hotkey" },
				{ k: "restart", en: "Restart", zh: "重新开始", c: "hotkey" },
			] },
		],
	},
	{
		id: "automations", icon: "workflow", en: "Automations", zh: "自动化", sumEn: "2 rules, local only", sumZh: "2 条规则，仅在本机运行",
		groups: [
			{ en: "Rules", zh: "规则", rows: [
				{ k: "rules", en: "Rules", zh: "规则", c: "rules", bare: true, was: ["Automations", "自动化"], kit: "setting + collapsible + select + input" },
			] },
		],
	},
	{
		id: "advanced", icon: "sliders-horizontal", en: "Advanced", zh: "高级", sumEn: "CLI, diagnostics, logs", sumZh: "CLI、诊断、日志",
		groups: [
			{ en: "Command line", zh: "命令行", rows: [
				{ k: "cli", en: "frameever command", zh: "frameever 命令", den: "For terminals, scripts and AI agents.", dzh: "用于终端、脚本和 AI 代理。", c: "cli", was: ["CLI", "CLI"], kit: "setting + button" },
			] },
			{ en: "Reliability", zh: "可靠性", rows: [
				{ k: "muxer", en: "Out-of-process muxer", zh: "独立进程封装", den: "Writes video in a separate process so a crash loses less.", dzh: "在独立进程写入视频，崩溃时损失更少。", c: "switch", was: ["Experimental", "实验功能"], kit: "switch" },
			] },
			{ en: "Troubleshooting", zh: "排查问题", rows: [
				{ k: "diag", en: "Diagnostic report", zh: "诊断报告", den: "A short test that flashes and beeps. Stays on this PC.", dzh: "一次会闪烁和发声的短测试，报告只存在本机。", c: "button", label: ["Run test", "运行测试"], was: ["Feedback", "反馈"], kit: "setting + button" },
				{ k: "logs", en: "Logs", zh: "日志", den: "Stay on this PC until you attach them yourself.", dzh: "只存在本机，由你决定是否附到问题报告。", c: "logs", was: ["Feedback", "反馈"], kit: "setting + button" },
			] },
		],
	},
	{
		id: "about", icon: "info", en: "About", zh: "关于", sumEn: "Version 1.0.0, updates, licences", sumZh: "版本 1.0.0、更新、许可",
		groups: [
			{ en: "", zh: "", rows: [
				{ k: "version", en: "Version", zh: "版本", c: "version", bare: true, was: ["Sidebar footer", "侧栏底部"], kit: "setting + button" },
			] },
			{ en: "Updates", zh: "更新", rows: [
				{ k: "channel", en: "Update channel", zh: "更新通道", den: "Nightly builds come first and may be unstable.", dzh: "预览版抢先发布，可能不稳定。", c: "seg", opts: [["stable", "Stable", "正式版"], ["nightly", "Nightly", "预览版"]], was: ["General · Updates", "通用 · 更新"], kit: "tab::TabBar segmented" },
				{ k: "report", en: "Report a problem", zh: "报告问题", den: "Opens GitHub in your browser. Nothing is sent until you submit.", dzh: "在浏览器打开 GitHub，提交前不发送任何内容。", c: "button", label: ["Open GitHub", "打开 GitHub"], was: ["Feedback", "反馈"], kit: "setting + button" },
			] },
			{ en: "What's new", zh: "更新日志", rows: [
				{ k: "log", en: "Changelog", zh: "更新日志", c: "changelog", bare: true, was: ["Changelog", "更新日志"], kit: "collapsible" },
			] },
			{ en: "Licences", zh: "许可", rows: [
				{ k: "credits", en: "Licences", zh: "许可", c: "credits", stack: true, was: ["Sidebar footer", "侧栏底部"], kit: "setting" },
			] },
		],
	},
];

const ui = { page: "general", search: "", popup: null, listening: null, conflict: null, needMod: false, ruleOpen: 0, flash: null, stateTab: "update", updateState: "ready" };
const rowOf = (key) => {
	for (const section of sections) {
		for (const group of section.groups) {
			const row = group.rows.find((candidate) => candidate.k === key);
			if (row) return { section, group, row };
		}
	}
	return null;
};
const allRows = () => sections.flatMap((section) => section.groups.flatMap((group) => group.rows.map((row) => ({ section, group, row }))));
const hotkeyText = (keys) => keys.map((key) => `<span class="kbd">${key}</span>`).join("");
const seg = (k, opts) => `<div class="seg">${opts.map(([value, en, zh]) => `<button class="${v[k] === value ? "on" : ""}" data-pick="${k}:${value}">${L(en, zh)}</button>`).join("")}</div>`;
const miniColors = { system: ["var(--neutral-3)", "#F4F5F9"], light: ["#E6E8F0", "#F8F9FC"], dark: ["#1B1F2E", "#10131E"] };

const controls = {
	switch: (row) => `<div class="row-control"><button class="switch ${v[row.k] ? "on" : ""}" data-toggle="${row.k}" role="switch" aria-checked="${v[row.k]}" aria-label="${pick(row.en, row.zh)}" style="border:0;cursor:pointer"></button></div>`,
	seg: (row) => `<div class="row-control">${seg(row.k, row.opts)}</div>`,
	select: (row) => {
		const current = row.opts.find(([value]) => value === v[row.k]);
		const open = ui.popup === row.k;
		const menu = open ? `<div class="menu pop" style="top:34px;right:0">${row.opts.map(([value, en, zh]) => `<button data-pick="${row.k}:${value}">${L(en, zh)}${value === v[row.k] ? icon("check") : ""}</button>`).join("")}</div>` : "";
		return `<div class="row-control" style="position:relative"><button class="pick-select" data-popup="${row.k}"><span>${L(current[1], current[2])}</span>${icon("chevron-down")}</button>${menu}</div>`;
	},
	theme: () => `<div class="row-control"><div class="themes">${[["system", "System", "跟随系统"], ["light", "Light", "浅色"], ["dark", "Dark", "深色"]].map(([value, en, zh]) => `<button class="theme-card ${v.theme === value ? "on" : ""}" data-pick="theme:${value}"><span class="theme-mini"><i style="background:${miniColors[value][0]}"></i><i style="background:${miniColors[value][1]}"></i></span>${L(en, zh)}</button>`).join("")}</div></div>`,
	tiers: (row) => `<div class="tiers">${row.opts.map(([value, en, zh, den, dzh]) => `<button class="tier ${v[row.k] === value ? "on" : ""}" data-pick="${row.k}:${value}"><b>${L(en, zh)}</b><small>${L(den, dzh)}</small></button>`).join("")}</div>`,
	slider: (row) => `<div class="row-control"><input type="range" min="1" max="4" step="0.1" value="${v[row.k]}" data-range="${row.k}" style="width:160px;accent-color:var(--primary)"><span class="tnum hint" style="width:32px">${Number(v[row.k]).toFixed(1)}x</span></div>`,
	button: (row) => `<div class="row-control"><button class="btn">${L(...row.label)}</button></div>`,
	logs: () => `<div class="row-control"><button class="btn">${L("Export logs", "导出日志")}</button><button class="btn ghost">${L("Open folder", "打开文件夹")}</button></div>`,
	cli: () => `<div class="row-control"><span class="badge pass">${L("On PATH", "已在 PATH")}</span><button class="btn ghost">${L("Uninstall", "卸载")}</button></div>`,
	hotkey: (row) => {
		const listening = ui.listening === row.k;
		const keys = v.hotkeys[row.k];
		const text = listening ? L("Press keys…  Esc cancels", "按下按键…  Esc 取消") : keys.length ? hotkeyText(keys) : L("Not set", "未设置");
		const clear = keys.length && !listening ? `<button class="btn ghost icon" data-clear="${row.k}" title="${pick("Clear", "清除")}">${icon("x")}</button>` : "";
		return `<div class="row-control"><button class="hotkey ${listening ? "listening" : ""}" data-listen="${row.k}">${text}</button>${clear}</div>`;
	},
	excluded: () => `<div class="chips">${v.excluded.map((name) => `<span class="tag">${name}</span>`).join("")}<button class="btn ghost" style="height:24px">${icon("plus")}${L("Add window", "添加窗口")}</button></div><div class="hint">${L("Over Remote Desktop or a streaming app, FrameEver windows stay visible.", "通过远程桌面或串流软件使用时，帧刻的窗口会留在画面里。")}</div>`,
	terms: () => `<div class="chips">${v.terms.map((term) => `<span class="tag on">${term}</span>`).join("")}</div><div class="input">${icon("plus")}<input data-ph-en="Add a term" data-ph-zh="添加词条" placeholder="${pick("Add a term", "添加词条")}"></div>`,
	folder: (row, broken) => `<div class="path">${icon("folder")}<span>C:\\Users\\you\\Videos\\FrameEver</span></div>${broken ? `<div class="warn">${icon("triangle-alert")}<span>${L("This folder isn't available. New projects go to the default folder until it's back.", "这个文件夹当前不可用。恢复之前，新项目会保存到默认文件夹。")}</span></div>` : ""}<div class="chips"><button class="btn">${L("Change folder", "更改文件夹")}</button><button class="btn ghost">${icon("folder-open")}${L("Show folder", "显示文件夹")}</button></div><div class="hint">${L("Previous folders. Their projects stay in the library.", "以前用过的文件夹，里面的项目仍在资源库中。")}</div><div class="path"><span>D:\\Old recordings</span><button class="btn ghost icon" title="${pick("Remove", "移除")}">${icon("x")}</button></div>`,
	template: () => `<div class="input"><input data-range="template" value="${v.template}"></div><div class="hint">${L("Preview", "预览")}: ${pick("Studio", "工作室")} 2026-10-30 14.05</div><div class="chips">${["{recording_mode}", "{target_name}", "{date}", "{time}"].map((token) => `<button class="tag chip-btn" data-token="${token}">${token}</button>`).join("")}</div>`,
	rules: () => {
		const rules = [
			[["Compress long Instant recordings", "压缩较长的即时录制"], ["When an Instant recording finishes, if longer than 10 min", "即时录制完成后，时长超过 10 分钟"], ["Export MP4 to D:\\Clips", "导出 MP4 到 D:\\Clips"]],
			[["Open exports folder", "打开导出文件夹"], ["When an export finishes", "导出完成后"], ["Open folder", "打开文件夹"]],
		];
		return `<div class="group-card">${rules.map(([name, when, then], index) => `<div class="rule"><button class="btn ghost icon" data-rule="${index}">${icon(ui.ruleOpen === index ? "chevron-down" : "chevron-right")}</button><div class="grow"><b style="font-weight:500">${L(...name)}</b><small>${L(...when)} → ${L(...then)}</small></div><button class="btn ghost">${L("Test", "测试")}</button><button class="switch on" style="border:0"></button></div>${ui.ruleOpen === index ? `<div class="rule-editor"><div class="rule-line"><span>${L("When", "当")}</span><button class="pick-select"><span>${L("Recording finished", "录制完成")}</span>${icon("chevron-down")}</button></div><div class="rule-line"><span>${L("If", "如果")}</span><button class="pick-select"><span>${L("Duration", "时长")}</span>${icon("chevron-down")}</button><div class="input" style="width:90px"><input value="10 min"></div><button class="btn ghost">${icon("plus")}${L("Condition", "条件")}</button></div><div class="rule-line"><span>${L("Then", "那么")}</span><button class="pick-select"><span>${L("Export", "导出")}</span>${icon("chevron-down")}</button><button class="btn ghost">${icon("plus")}${L("Action", "操作")}</button></div></div>` : ""}`).join("")}</div><div class="chips" style="margin-top:10px"><button class="btn primary">${icon("plus")}${L("Add rule", "添加规则")}</button><button class="btn ghost">${L("From template", "从模板添加")}</button></div>`;
	},
	version: () => {
		const state = ui.updateState;
		const right = {
			idle: `<button class="btn">${L("Check for updates", "检查更新")}</button>`,
			checking: `<button class="btn" disabled>${L("Checking…", "正在检查…")}</button>`,
			latest: `<span class="hint">${L("You're up to date", "已是最新版本")}</span><button class="btn ghost">${L("Check again", "再次检查")}</button>`,
			downloading: `<span class="hint tnum">${L("Downloading 1.0.1… 42%", "正在下载 1.0.1… 42%")}</span><div class="progress" style="width:120px"><i style="width:42%"></i></div>`,
			ready: `<button class="btn primary">${L("Restart to update", "重启以更新")}</button>`,
			failed: `<span class="hint">${L("Couldn't check for updates.", "无法检查更新。")}</span><button class="btn">${L("Try again", "再试一次")}</button>`,
		}[state];
		return `<div class="group-card about-head"><div class="logo">F</div><div class="grow"><b style="font-weight:600">FrameEver · 帧刻</b><div class="hint">${L("Version", "版本")} 1.0.0</div></div>${right}</div>`;
	},
	changelog: () => `<div class="group-card"><div class="log-entry" style="border-top:0"><b>1.0.0</b> · ${L("First release of FrameEver.", "帧刻的第一个版本。")}</div><div class="log-entry"><button class="btn ghost">${L("All versions", "全部版本")}${icon("chevron-right")}</button></div></div>`,
	credits: () => `<div class="hint">${L("Based on Cap (AGPL-3.0). Icons: Lucide (ISC), Tabler (MIT). Chinese font: HarmonyOS Sans by Huawei.", "基于 Cap（AGPL-3.0）。图标：Lucide（ISC）、Tabler（MIT）。中文字体：华为 HarmonyOS Sans。")}</div>`,
};

const rowHtml = (row) => {
	if (row.bare) return controls[row.c](row);
	const off = row.dep && !v[row.dep];
	const label = `<div class="row-label"><b>${L(row.en, row.zh)}</b>${row.den ? `<small>${L(row.den, row.dzh)}</small>` : ""}</div>`;
	const stacked = row.stack || row.c === "tiers";
	const hideLabel = row.c === "folder" || row.c === "template" || row.c === "excluded" || row.c === "credits";
	return `<div class="row ${stacked ? "stack" : ""} ${row.dep || row.k === "zoom" ? "dep" : ""} ${off || (row.k === "zoom" && !v.autoZoom) ? "off" : ""} ${ui.flash === row.k ? "flash" : ""}" id="row-${row.k}">${hideLabel ? "" : label}${controls[row.c](row)}</div>`;
};
const groupHtml = (group) => {
	const bare = group.rows.every((row) => row.bare);
	return `<div class="group">${group.en ? `<h4>${L(group.en, group.zh)}</h4>` : ""}${bare ? group.rows.map(rowHtml).join("") : `<div class="group-card">${group.rows.map(rowHtml).join("")}</div>`}</div>`;
};
const pageHtml = (section) => {
	const hint = section.id === "shortcuts" ? `<div class="warn" style="margin-bottom:12px;${ui.conflict || ui.needMod ? "" : "display:none"}">${icon("triangle-alert")}<span>${ui.needMod ? L("Add Ctrl, Alt or Shift so typing doesn't trigger it.", "加上 Ctrl、Alt 或 Shift，免得打字时误触发。") : L(`Moved here from "${ui.conflict?.en}", which now has no shortcut.`, `已从“${ui.conflict?.zh}”移到这里，那一项现在没有快捷键。`)}</span></div>` : "";
	return hint + section.groups.map(groupHtml).join("");
};
const titleBar = (extra = "") => `<div class="titlebar">${extra}<span class="grow"></span><span class="caption">${icon("minus")}</span><span class="caption">${icon("square")}</span><span class="caption">${icon("x")}</span></div>`;
const searchBox = (cls = "") => `<div class="input ${cls}">${icon("search")}<input data-search value="${ui.search}" data-ph-en="Search settings  Ctrl+F" data-ph-zh="搜索设置  Ctrl+F" placeholder="${pick("Search settings  Ctrl+F", "搜索设置  Ctrl+F")}"></div>`;
const matches = () => {
	const q = ui.search.trim().toLowerCase();
	if (!q) return [];
	return allRows().filter(({ row, section }) => [row.en, row.zh, row.den, row.dzh, section.en, section.zh].some((text) => text?.toLowerCase().includes(q)));
};
const resultList = () => {
	const hits = matches();
	if (hits.length === 0) return `<div class="empty">${L(`Nothing matches "${ui.search}"`, `没有匹配“${ui.search}”的设置`)}</div>`;
	return `<div class="group-card">${hits.map(({ row, section, group }) => `<button class="result" data-jump="${row.k}"><span><b style="font-weight:500">${L(row.en, row.zh)}</b>${row.den ? `<small>${L(row.den, row.dzh)}</small>` : ""}</span><span class="where">${L(section.en, section.zh)}${group.en && group.en !== row.en ? ` › ${L(group.en, group.zh)}` : ""}</span></button>`).join("")}</div>`;
};

const renderA = () => {
	const section = sections.find((entry) => entry.id === ui.page);
	const nav = sections.map((entry) => `<button class="nav-item ${!ui.search && entry.id === ui.page ? "on" : ""}" data-page="${entry.id}">${icon(entry.icon)}${L(entry.en, entry.zh)}${entry.id === "about" && ui.updateState === "ready" ? `<span class="badge klein" style="margin-left:auto">1</span>` : ""}</button>`).join("");
	const content = ui.search
		? `<div class="page-title">${L("Results", "搜索结果")}</div><div class="page-sub">${L(`${matches().length} settings`, `${matches().length} 项设置`)}</div>${resultList()}`
		: `<div class="page-title">${L(section.en, section.zh)}</div><div class="page-sub">${L(section.sumEn, section.sumZh)}</div>${pageHtml(section)}`;
	return `<div class="win"><div class="body"><aside class="side"><div class="side-head">${searchBox()}</div><div class="side-list">${nav}</div></aside><div style="flex:1;min-width:0;display:flex;flex-direction:column">${titleBar()}<div class="content" data-scroll>${content}</div></div></div></div>`;
};

const stage = document.getElementById("stage");
const render = () => {
	const scroller = stage.querySelector("[data-scroll]");
	const top = scroller?.scrollTop ?? 0;
	const search = document.activeElement?.matches?.("#stage [data-search]");
	const caret = search ? document.activeElement.selectionStart : 0;
	stage.innerHTML = renderA();
	const next = stage.querySelector("[data-scroll]");
	if (next) next.scrollTop = top;
	if (search) {
		const input = stage.querySelector("[data-search]");
		input.focus();
		input.setSelectionRange(caret, caret);
	}
};
const jump = (key) => {
	const { section } = rowOf(key);
	ui.page = section.id;
	ui.search = "";
	ui.flash = key;
	render();
	document.getElementById(`row-${key}`)?.scrollIntoView({ block: "center" });
	setTimeout(() => {
		ui.flash = null;
	}, 1300);
};

document.addEventListener("click", (event) => {
	const target = event.target.closest("button, [data-popup]");
	if (event.target.closest("#state-stage")) return handleState(target);
	if (!event.target.closest("#stage")) return;
	const data = target?.dataset ?? {};
	if (data.pick) {
		const [k, value] = data.pick.split(":");
		v[k] = value;
		ui.popup = null;
	} else if (data.toggle) {
		v[data.toggle] = !v[data.toggle];
	} else if (data.popup) {
		ui.popup = ui.popup === data.popup ? null : data.popup;
	} else if (data.page) {
		ui.page = data.page;
		ui.popup = null;
		stage.querySelector("[data-scroll]").scrollTop = 0;
	} else if (data.jump) {
		return jump(data.jump);
	} else if (data.listen) {
		ui.listening = ui.listening === data.listen ? null : data.listen;
		ui.conflict = null;
		ui.needMod = false;
	} else if (data.clear) {
		v.hotkeys[data.clear] = [];
	} else if (data.rule) {
		ui.ruleOpen = ui.ruleOpen === Number(data.rule) ? -1 : Number(data.rule);
	} else if (data.token) {
		v.template = `${v.template} ${data.token}`;
	} else if (ui.popup) {
		ui.popup = null;
	} else return;
	render();
});
document.addEventListener("input", (event) => {
	if (event.target.matches("#stage [data-search]")) {
		ui.search = event.target.value;
		render();
	} else if (event.target.dataset.range) {
		const key = event.target.dataset.range;
		v[key] = event.target.value;
		if (key === "zoom") render();
	}
});
document.addEventListener("keydown", (event) => {
	if (ui.listening) {
		event.preventDefault();
		if (event.key === "Escape") {
			ui.listening = null;
			ui.needMod = false;
			ui.conflict = null;
			return render();
		}
		if (["Control", "Shift", "Alt", "Meta"].includes(event.key)) return;
		const keys = [event.ctrlKey && "Ctrl", event.altKey && "Alt", event.shiftKey && "Shift"].filter(Boolean);
		const isFn = /^F\d+$/.test(event.key);
		if (!keys.length && !isFn) {
			ui.needMod = true;
			return render();
		}
		const combo = [...keys, event.key.length === 1 ? event.key.toUpperCase() : event.key];
		const holder = Object.entries(v.hotkeys).find(([key, value]) => key !== ui.listening && value.join() === combo.join());
		ui.needMod = false;
		ui.conflict = null;
		if (holder) {
			ui.conflict = rowOf(holder[0]).row;
			v.hotkeys[holder[0]] = [];
		}
		v.hotkeys[ui.listening] = combo;
		ui.listening = null;
		return render();
	}
	if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
		event.preventDefault();
		stage.querySelector("[data-search]")?.focus();
		return;
	}
	if (event.key === "Escape" && ui.search) {
		ui.search = "";
		return render();
	}
	if (event.key === "Enter" && event.target.matches?.("#stage [data-search]")) {
		const first = matches()[0];
		if (first) jump(first.row.k);
		return;
	}
});

const stateTabs = [
	["update", "Update states", "更新状态"],
	["folder", "Folder unavailable", "文件夹不可用"],
	["nomatch", "No search results", "没有搜索结果"],
];
const updateStates = [["idle", "Idle", "空闲"], ["checking", "Checking", "检查中"], ["latest", "Up to date", "已是最新"], ["downloading", "Downloading", "下载中"], ["ready", "Ready", "可重启"], ["failed", "Failed", "失败"]];
const handleState = (target) => {
	if (target?.dataset.ustate) {
		ui.updateState = target.dataset.ustate;
		renderStates();
		render();
	}
};
const renderStates = () => {
	document.getElementById("state-tabs").innerHTML = stateTabs.map(([key, en, zh]) => `<button class="btn ${ui.stateTab === key ? "primary" : ""}" data-tab="${key}">${L(en, zh)}</button>`).join("");
	const body = {
		update: () => `<div class="win short" style="padding:14px"><div class="chips" style="margin-bottom:12px">${updateStates.map(([key, en, zh]) => `<button class="btn ${ui.updateState === key ? "primary" : "ghost"}" data-ustate="${key}">${L(en, zh)}</button>`).join("")}</div>${controls.version()}<p class="hint" style="margin:10px 0 0">${L("Ready also puts a count on About in the sidebar. Variant A shows it; the others show a badge on the matching row.", "就绪时侧栏“关于”上出现数字，方案 A 如此；其他方案在对应行显示徽标。")}</p></div>`,
		folder: () => `<div class="win short" style="padding:14px"><div class="group-card"><div class="row stack">${controls.folder(null, true)}</div></div></div>`,
		nomatch: () => `<div class="win short" style="padding:14px"><div class="empty">${L('Nothing matches "wxyz"', "没有匹配“wxyz”的设置")}</div></div>`,
	}[ui.stateTab]();
	document.getElementById("state-stage").innerHTML = body;
};
document.addEventListener("click", (event) => {
	const tab = event.target.closest("[data-tab]");
	if (tab) {
		ui.stateTab = tab.dataset.tab;
		renderStates();
	}
});

const openCards = [
	[["Where it opens from", "从哪里打开"], ["The gear in the main window, tray Settings, Ctrl+, in any FrameEver window, and palette results. Palette results land on the row and flash it. Only one settings window exists; opening again focuses it.", "主窗口的齿轮、托盘的设置、任何帧刻窗口里按 Ctrl+,、命令面板结果。面板结果跳到那一行并闪一下。设置窗口只有一个，再次打开时聚焦它。"]],
	[["What happens to the main window", "主窗口怎么办"], ["Compact main window hides while settings is open and comes back when it closes, because it is an on-top panel. An expanded main window stays where it is, as an ordinary window behind settings.", "紧凑态是置顶面板，设置打开时隐藏，关闭后回来。展开态是普通窗口，留在设置后面不动。"]],
	[["Window", "窗口"], ["An ordinary resizable window, 880 × 640, minimum 720 × 520. It reopens on the last page within a session; a fresh launch starts on General. Esc clears search and never closes the window; Ctrl+W closes it.", "普通可调整大小的窗口，880 × 640，最小 720 × 520。同一次运行内回到上次的页面，重新启动后从通用开始。Esc 只清空搜索，不关窗口；Ctrl+W 关闭。"]],
];
document.getElementById("open-grid").innerHTML = openCards.map(([title, text]) => `<div class="card"><h3>${L(...title)}</h3><p>${L(...text)}</p></div>`).join("");

const removed = [
	[["Recordings page", "录制文件页"], ["Moves to the library", "移到资源库"], "—"],
	[["Screenshots page", "截图页"], ["Moves to the library", "移到资源库"], "—"],
	[["Always show dock icon", "始终显示程序坞图标"], ["macOS only: hidden on Windows", "仅 macOS：Windows 上隐藏"], "—"],
	[["MacBook notch", "MacBook 刘海"], ["macOS only: hidden on Windows", "仅 macOS：Windows 上隐藏"], "—"],
	[["Use native app (developer)", "使用原生应用（开发者）"], ["Removed: the Tauri app is gone", "移除：Tauri 应用已删除"], "—"],
	[["Sidebar version block", "侧栏版本区"], ["Moves to About", "移到关于"], "—"],
];
document.getElementById("inv").innerHTML = `<table class="inv"><thead><tr><th>${L("Setting", "设置项")}</th><th>${L("Today", "现在在哪")}</th><th>${L("GPUI Kit", "GPUI Kit")}</th></tr></thead><tbody>${sections.map((section) => `<tr class="sep"><th colspan="3">${L(section.en, section.zh)}</th></tr>${section.groups.flatMap((group) => group.rows).map((row) => `<tr><td>${L(row.en, row.zh)}${row.fresh ? ` <span class="badge klein">${L("New options", "新选项")}</span>` : ""}</td><td>${row.was ? L(...row.was) : L("Shortcuts", "快捷键")}</td><td><code>${row.kit ?? "kbd + custom hotkey capture"}</code></td></tr>`).join("")}`).join("")}<tr class="sep"><th colspan="3">${L("Leaves settings", "离开设置")}</th></tr>${removed.map(([name, where]) => `<tr><td>${L(...name)}</td><td colspan="2">${L(...where)}</td></tr>`).join("")}</tbody></table>`;

const copy = {
	"t-live": ["Settings window", "设置窗口"],
	"l-live": ["The settings window in Klein Field. Eight pages, grouped by what you are doing. Search, shortcuts, dropdowns and switches all respond; press Ctrl+Shift+A on another shortcut row to see a conflict.", "Klein Field 下的设置窗口。8 页，按要做的事分组。搜索、快捷键、下拉框、开关都能操作；在别的快捷键行按 Ctrl+Shift+A 可以看到冲突处理。"],
	"t-states": ["States", "状态"],
	"l-states": ["The states that the live window doesn't show on its own.", "实时窗口里不会自己出现的状态。"],
	"t-open": ["Opening and closing", "打开和关闭"],
	"l-open": ["Decided with the structure.", "随结构一起定。"],
	"t-inv": ["Every setting", "全部设置项"],
	"l-inv": ["Where each current setting lands, and the GPUI Kit part that draws it. All page and row bodies use setting::{SettingPage, SettingGroup, SettingItem}.", "每个现有设置项的去向和绘制它的 GPUI Kit 组件。页面和行都用 setting::{SettingPage, SettingGroup, SettingItem}。"],
};
for (const [id, pair] of Object.entries(copy)) document.getElementById(id).innerHTML = L(...pair);

FE.shell({ here: "settings", depth: 1, sections: [["live", "Prototype", "原型"], ["states", "States", "状态"], ["opening", "Opening", "打开和关闭"], ["inventory", "Every setting", "全部设置项"]] });
FE.onChange(() => {
	render();
	renderStates();
});
render();
renderStates();
