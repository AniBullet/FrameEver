const { L, icon, pick, shell, onChange } = FE;

const STATES = [
	["idle", "Idle", "空闲"],
	["countdown", "Countdown", "倒计时"],
	["recording", "Recording", "录制中"],
	["paused", "Paused", "已暂停"],
	["exporting", "Exporting", "导出中"],
	["setup", "First run", "首次运行"],
];
const MODES = [
	["studio", "Studio", "Studio"],
	["instant", "Instant", "即时"],
	["screenshot", "Screenshot", "截图"],
];
const RECENT = [
	["Product demo", "产品演示", "clapperboard"],
	["Bug report", "问题复现", "zap"],
	["Pricing page", "定价页面", "image"],
	["Weekly update", "每周更新", "clapperboard"],
	["Login flow", "登录流程", "zap"],
	["Settings panel", "设置面板", "image"],
];
const s = { st: "idle", mode: "studio", win: "dark", recent: true, open: true, sub: false, hidden: false, log: null };

const RED = "#FF3B30";
const KLEIN = "#1F3FD8";
const AMBER = "#F5A524";
const plate = `<rect width="48" height="48" rx="10.8" fill="${KLEIN}"/>`;
const ring = '<circle cx="24" cy="24" r="15" fill="none" stroke="#fff" stroke-width="5"/>';
const glyphs = {
	idle: `${plate}<rect x="9" y="25" width="30" height="7" rx="3.5" fill="#FFFDED"/><rect x="9" y="34" width="17" height="7" rx="3.5" fill="#87BFFF"/><circle cx="30.5" cy="13.5" r="9.5" fill="#fff"/><circle cx="30.5" cy="13.5" r="6.1" fill="${RED}"/>`,
	recording: `${plate}${ring}<rect x="16" y="16" width="16" height="16" rx="2.5" fill="${RED}"/>`,
	paused: `${plate}${ring}<rect x="17" y="17" width="6" height="14" rx="2" fill="${AMBER}"/><rect x="25" y="17" width="6" height="14" rx="2" fill="${AMBER}"/>`,
	exporting: `${plate}<circle cx="24" cy="24" r="15" fill="none" stroke="#fff" stroke-opacity="0.3" stroke-width="5"/><circle cx="24" cy="24" r="15" fill="none" stroke="#fff" stroke-width="5" stroke-linecap="round" stroke-dasharray="58.4 94.2" transform="rotate(-90 24 24)"/>`,
};
const trayIcon = (state, size) => {
	const shape = glyphs[state] ?? glyphs.idle;
	return `<svg width="${size}" height="${size}" viewBox="0 0 48 48" aria-hidden="true">${shape}</svg>`;
};

const verb = (target) =>
	s.mode === "screenshot"
		? { display: ["Screenshot display", "截取显示器"], window: ["Screenshot window", "截取窗口"], area: ["Screenshot area", "截取区域"] }[target]
		: { display: ["Record display", "录制显示器"], window: ["Record window", "录制窗口"], area: ["Record area", "录制区域"] }[target];
const idleRows = () => [
	{ t: ["Open FrameEver", "打开帧刻"], a: "open", bold: true },
	{ t: ["Open library", "打开资源库"], a: "library" },
	{ sep: 1 },
	{ modes: 1 },
	{ t: verb("display"), a: "display", ic: "monitor" },
	{ t: verb("window"), a: "window", ic: "app-window" },
	{ t: verb("area"), a: "area", ic: "scan", key: "Ctrl Shift 4" },
	{ t: s.mode === "screenshot" ? ["Screenshot last area", "截取上次区域"] : ["Record last area", "录制上次区域"], a: "last", ic: "rotate-ccw" },
	...(s.mode === "screenshot" ? [] : [{ t: ["Take a screenshot", "截图"], a: "shot", ic: "image", key: "Ctrl Shift 2" }]),
	{ sep: 1 },
	...(s.recent ? [{ t: ["Recent", "最近"], sub: 1, ic: "film" }] : []),
	{ t: ["Import media…", "导入媒体…"], a: "import", ic: "download" },
	{ t: ["Settings", "设置"], a: "settings", ic: "settings" },
	{ sep: 1 },
	{ t: ["Quit FrameEver", "退出帧刻"], a: "quit", ver: true },
];
const hide = () => ({ t: s.hidden ? ["Show controls", "显示控制条"] : ["Hide controls", "隐藏控制条"], a: "hide", ic: "eye-off", key: "Ctrl Shift H" });
const rows = () => {
	if (s.st === "setup") return [{ t: ["Continue setup", "继续设置"], a: "setup", bold: true }, { sep: 1 }, { t: ["Quit FrameEver", "退出帧刻"], a: "quit", ver: true }];
	if (s.st === "countdown") return [{ status: ["Starting in 3", "3 秒后开始"] }, { sep: 1 }, { t: ["Cancel", "取消"], a: "cancel", bold: true, ic: "x" }];
	if (s.st === "recording")
		return [
			{ status: ["Recording · 1:23", "录制中 · 1:23"], dot: "rec" },
			{ sep: 1 },
			{ t: ["Stop recording", "停止录制"], a: "stop", bold: true, ic: "square" },
			{ t: ["Pause", "暂停"], a: "pause", ic: "pause" },
			{ t: ["Restart…", "重新开始…"], a: "restart", ic: "rotate-ccw" },
			{ sep: 1 },
			hide(),
		];
	if (s.st === "paused")
		return [
			{ status: ["Paused · 1:23", "已暂停 · 1:23"], dot: "amber" },
			{ sep: 1 },
			{ t: ["Resume", "继续"], a: "resume", bold: true, ic: "play" },
			{ t: ["Stop recording", "停止录制"], a: "stop", ic: "square" },
			{ t: ["Restart…", "重新开始…"], a: "restart", ic: "rotate-ccw" },
			{ sep: 1 },
			hide(),
		];
	if (s.st === "exporting") return [{ exp: 1 }, { sep: 1 }, ...idleRows()];
	return idleRows();
};
const kbd = (text) => text.split(" ").map((key) => `<span class="kbd">${key}</span>`).join(" ");
const row = (r) => {
	if (r.sep) return "<hr>";
	if (r.status) return `<div class="status tnum">${r.dot ? `<i class="dot ${r.dot === "amber" ? "amber" : ""}"></i>` : ""}<span>${L(...r.status)}</span></div>`;
	if (r.modes) return `<div class="pad"><div class="seg">${MODES.map(([v, en, zh]) => `<button class="${s.mode === v ? "on" : ""}" data-mode="${v}">${L(en, zh)}</button>`).join("")}</div></div>`;
	if (r.exp) return `<button data-a="export"><span class="grow tnum">${L("Exporting Product demo · 42%", "正在导出 产品演示 · 42%")}</span></button><div class="bar"><div class="progress"><i style="width:42%"></i></div></div>`;
	if (r.sub) return `<button data-sub class="${s.sub ? "hover" : ""}" aria-haspopup="menu">${icon(r.ic)}<span class="grow">${L(...r.t)}</span><span class="chev">${icon("chevron-right", "sm")}</span></button>`;
	return `<button data-a="${r.a}" class="${r.bold ? "bold" : ""}">${r.ic ? icon(r.ic) : ""}<span class="grow">${L(...r.t)}</span>${r.key ? kbd(r.key) : ""}${r.ver ? '<span class="ver tnum">1.0.0</span>' : ""}</button>`;
};
const menu = () => `<div class="menu tm" role="menu">${rows().map(row).join("")}</div>`;
const recent = () => `<div class="menu tm" role="menu">${RECENT.map(([en, zh, ic]) => `<button data-a="recent"><span class="thumb">${icon(ic, "sm")}</span><span class="grow">${L(en, zh)}</span></button>`).join("")}<hr><button data-a="library"><span class="grow">${L("All in library", "在资源库里查看全部")}</span></button></div>`;

const results = {
	open: ["The compact main window opens.", "打开紧凑主窗口。"],
	library: ["The main window opens expanded on the library.", "主窗口展开到资源库。"],
	display: ["The display target is chosen.", "选好显示器目标。"],
	window: ["The window picker opens.", "打开窗口选择。"],
	area: ["The area picker opens.", "打开区域选择。"],
	last: ["The last area is used, with no picker.", "直接用上次的区域，不打开选择器。"],
	shot: ["Takes a screenshot of the display under the cursor.", "截取光标所在的显示器。"],
	recent: ["The library opens with that project selected.", "打开资源库并选中这个项目。"],
	import: ["The file picker opens.", "打开文件选择。"],
	settings: ["Settings opens.", "打开设置。"],
	quit: ["FrameEver quits.", "帧刻退出。"],
	setup: ["Onboarding opens.", "打开引导。"],
	export: ["The export page comes to the front.", "导出页面来到前面。"],
	restart: ["The bar's Restart confirmation shows; hidden controls come back first.", "显示控制条的重新开始确认；控制条隐藏时先恢复。"],
};
const act = (a) => {
	s.open = false;
	s.sub = false;
	if (a === "stop") { s.st = "idle"; s.log = ["The recording stops and is saved.", "录制停止并保存。"]; return; }
	if (a === "cancel") { s.st = "idle"; s.log = ["The countdown is cancelled.", "倒计时已取消。"]; return; }
	if (a === "pause") { s.st = "paused"; s.log = ["Paused. The icon changes.", "已暂停，图标跟着变。"]; return; }
	if (a === "resume") { s.st = "recording"; s.log = ["Recording again.", "继续录制。"]; return; }
	if (a === "hide") { s.hidden = !s.hidden; s.log = s.hidden ? ["The controls are hidden.", "控制条已隐藏。"] : ["The controls are back.", "控制条已恢复。"]; return; }
	s.log = results[a];
};
const leftClick = () => {
	if (s.st === "recording" || s.st === "paused") return act("stop");
	if (s.st === "countdown") return act("cancel");
	if (s.st === "exporting") return act("export");
	if (s.st === "setup") return act("setup");
	return act("open");
};
const rightClick = () => {
	s.open = true;
	s.sub = false;
	s.log = ["The menu opens above the icon.", "菜单在图标上方打开。"];
};

const seg = (name, value, options) => `<div class="seg" data-tool="${name}">${options.map(([v, en, zh]) => `<button class="${value === v ? "on" : ""}" data-v="${v}">${L(en, zh)}</button>`).join("")}</div>`;
const tool = (en, zh, html) => `<div class="tool"><span>${L(en, zh)}</span>${html}</div>`;
const table = (id, head, list) => {
	const cell = (c) => (Array.isArray(c) ? L(...c) : c);
	document.getElementById(id).innerHTML = `<table class="inv"><thead><tr>${head.map((h) => `<th>${L(...h)}</th>`).join("")}</tr></thead><tbody>${list.map((r) => `<tr>${r.map((c) => `<td>${cell(c)}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
};
const text = (id, en, zh) => {
	document.getElementById(id).innerHTML = L(en, zh);
};

const paint = () => {
	document.getElementById("tools").innerHTML = `${tool("State", "状态", seg("st", s.st, STATES))}${tool("Mode", "模式", seg("mode", s.mode, MODES))}${tool("Taskbar", "任务栏", seg("win", s.win, [["dark", "Dark", "深色"], ["light", "Light", "浅色"]]))}${tool("Recent items", "最近项目", seg("recent", s.recent ? "y" : "n", [["y", "Some", "有"], ["n", "None", "没有"]]))}`;
	const stage = document.getElementById("stage");
	stage.className = `stage win-${s.win}`;
	stage.innerHTML = `${s.open ? `<div class="anchor ${s.sub ? "sub-open" : ""}">${s.sub ? recent() : ""}${menu()}</div>` : ""}<div class="taskbar"><button id="tray" aria-label="FrameEver">${trayIcon(s.st, 20)}</button><div class="clock tnum">14:32<br>2026/10/31</div></div>`;
	document.getElementById("clicks").innerHTML = `<button class="btn" data-click="left">${L("Left click the icon", "左键点图标")}</button><button class="btn" data-click="right">${L("Right click the icon", "右键点图标")}</button><button class="btn ghost" data-click="close">${L("Click elsewhere", "点别处")}</button>`;
	document.getElementById("line").innerHTML = s.log ? L(...s.log) : "";
};

const copy = () => {
	text("t-menu", "Menu", "菜单");
	text("l-menu", "Our own GPUI popup on Klein Field tokens, 280 px wide with 34 px rows. It opens above the icon, follows the app's theme and stays out of recordings. Click the rows and try each state.", "我们自己的 GPUI 弹出窗口，用 Klein Field token，宽 280 px，行高 34 px。在图标上方打开，跟随应用主题，不会录进视频。可以点每一行，切换各个状态。");
	text("t-icons", "Icon", "图标");
	text("l-icons", "Sizes follow Windows' small icon at each scale: 16, 20, 24 and 32 px. Recording and paused use the bar's ring on the app icon's plate, so the tray reads as the same Stop as the recording bar. Nothing blinks.", "尺寸跟随 Windows 各缩放比例下的小图标：16、20、24、32 px。录制中和已暂停在应用图标的底板上用控制条的圆环，所以托盘和控制条的停止是同一个东西。没有闪烁。");
	text("t-clicks", "Clicks", "点击");
	text("l-clicks", "Left click does the most likely thing in each state; right click always opens the menu. Double click does what left click does, once.", "左键做当前状态下最可能要做的事，右键总是打开菜单。双击和左键相同，只执行一次。");
	text("t-rules", "Rules", "规则");
	text("l-rules", "What the build has to keep.", "实现时要保持的规则。");
	text("t-keys", "Keyboard", "键盘");
	text("l-keys", "Every row is reachable without a mouse.", "每一行都能不用鼠标完成。");
	text("t-inv", "Everything in the tray today", "现在托盘里的全部内容");
	text("l-inv", "Compared with upstream at f7d351611. Nothing is lost except cloud features.", "对照上游 f7d351611。除了云功能，没有丢失任何能力。");
	text("t-add", "Added", "新增");
	text("l-add", "One addition from other tools.", "从其他工具加入的一项。");
	text("t-ref", "References", "参考");
	text("l-ref", "What each source decided.", "每个来源决定了什么。");

	document.getElementById("icon-grid").innerHTML = [
		["idle", "Idle", "空闲", "The app icon, from icon.ico. Countdown and first run keep it.", "应用图标，来自 icon.ico。倒计时和首次运行也用它。"],
		["recording", "Recording", "录制中", "White ring and red square. Replaces the black stop PNG, which can't be seen on a dark taskbar.", "白环加红方块。替换现在的黑色 stop PNG，它在深色任务栏上几乎看不见。"],
		["paused", "Paused", "已暂停", "Amber bars in the same ring.", "同样的圆环里放琥珀色暂停条。"],
		["exporting", "Exporting", "导出中", "A white ring that fills in eight steps, at most one step every few seconds.", "白环分八步填满，几秒最多走一步。"],
	].map(([st, en, zh, den, dzh]) => `<div class="icon-card"><h3>${L(en, zh)}</h3>${["dark", "light"].map((bar) => `<div class="icon-bar ${bar}">${[16, 20, 24, 32].map((px) => trayIcon(st, px)).join("")}</div>`).join("")}<p>${L(den, dzh)}</p></div>`).join("");

	table("click-table", [["State", "状态"], ["Left click", "左键"], ["Tooltip", "提示文字"]], [
		[["Idle", "空闲"], ["Opens the compact main window", "打开紧凑主窗口"], ["FrameEver", "帧刻"]],
		[["Countdown", "倒计时"], ["Cancels", "取消"], ["FrameEver · Starting", "帧刻 · 即将开始"]],
		[["Recording", "录制中"], ["Stops", "停止"], ["FrameEver · Recording", "帧刻 · 录制中"]],
		[["Paused", "已暂停"], ["Stops", "停止"], ["FrameEver · Paused", "帧刻 · 已暂停"]],
		[["Exporting", "导出中"], ["Brings the export page forward", "把导出页面带到前面"], ["FrameEver · Exporting 42%", "帧刻 · 正在导出 42%"]],
		[["First run", "首次运行"], ["Opens onboarding", "打开引导"], ["FrameEver", "帧刻"]],
	]);

	table("rule-table", [["Rule", "规则"], ["Detail", "内容"]], [
		[["Window", "窗口"], ["One hidden popup window, created at startup so the first open is immediate. No animation, because windows never animate", "启动时建好一个隐藏的弹出窗口，让第一次打开也够快。没有动画，因为窗口从不做动画"]],
		[["Position", "位置"], ["Right edge at the icon, 12 px from the taskbar, clamped to the work area of the icon's display. Moves for a taskbar on the top or sides", "右边缘对齐图标，离任务栏 12 px，限制在图标所在显示器的工作区内。任务栏在顶部或两侧时相应调整"]],
		[["Dismiss", "关闭"], ["Clicking anywhere else, Esc, or choosing a row", "点别处、按 Esc 或选择一行"]],
		[["In recordings", "录制"], ["Set WDA_EXCLUDEFROMCAPTURE and add the window to the excluded list, so opening the menu during a display recording leaves no trace. Over remote desktop it follows Hide FrameEver windows over remote desktop", "设置 WDA_EXCLUDEFROMCAPTURE 并加入排除窗口列表，录制显示器时打开菜单不留痕迹。远程桌面下遵循 Hide FrameEver windows over remote desktop"]],
		[["Fallback", "回退"], ["The same entry list also builds a native Windows menu, used only if the popup window can't be created. Quit must always be reachable", "同一份条目列表也能生成 Windows 原生菜单，只在弹出窗口建不出来时使用。退出必须始终可用"]],
		[["Rows that don't apply", "不适用的行"], ["Removed, never greyed out: no Recent without recent items, no Record last area until an area has been used", "直接去掉，不置灰：没有最近项目就没有“最近”，没用过区域就没有“录制上次区域”"]],
		[["Recording", "录制中"], ["Only Stop, Pause or Resume, Restart and the controls. No Quit, matching the command palette", "只有停止、暂停或继续、重新开始和控制条。没有退出，和命令面板一致"]],
		[["Mode", "模式"], ["Neutral segments, like every segmented control. Changing mode keeps the menu open and the capture rows follow", "中性分段控件，和所有分段控件一致。切换模式时菜单不关，下面的捕获行跟着变"]],
		[["Recent", "最近"], ["Six items at most, with thumbnails and no emoji. Choosing one opens the library with it selected, as the main window ticket decided", "最多六项，用缩略图，不用 emoji。选择后打开资源库并选中它，这是主窗口工单定的"]],
		[["Shortcuts", "快捷键"], ["Shown only for shortcuts the user has set", "只显示用户设置过的快捷键"]],
		[["macOS", "macOS"], ["Keeps the native status menu; decided with the macOS work", "保留原生状态栏菜单，等 macOS 版时再定"]],
	]);

	table("key-table", [["Key", "按键"], ["Does", "作用"]], [
		[kbd("Win B"), ["Windows moves focus to the tray icons. Enter is left click, Shift F10 opens the menu", "Windows 把焦点移到托盘图标。Enter 相当于左键，Shift F10 打开菜单"]],
		[`${kbd("↑")} ${kbd("↓")}`, ["Move between rows, wrapping at the ends", "在行之间移动，到头后循环"]],
		[`${kbd("←")} ${kbd("→")}`, ["Change mode on the mode row; open and close Recent", "在模式行切换模式；打开和关闭“最近”"]],
		[kbd("Enter"), ["Choose the row", "选择这一行"]],
		[kbd("Esc"), ["Close Recent, then the menu", "先关闭“最近”，再关闭菜单"]],
	]);

	table("inv-table", [["Item", "项目"], ["Today", "现在"], ["1.0", "1.0"]], [
		[["Open Main Window", "打开主窗口"], ["Ours and upstream", "我们和上游都有"], ["Open FrameEver, in bold, and the same as left click", "打开帧刻，加粗，和左键相同"]],
		[["Record display, window, area", "录制显示器、窗口、区域"], ["Ours and upstream", "我们和上游都有"], ["Kept, with icons", "保留，加图标"]],
		[["Screenshot display, window, area", "截取显示器、窗口、区域"], ["Ours arms the target; upstream has separate items that take the screenshot", "我们是选好目标；上游有单独的项目直接截图"], ["Port upstream's items", "移植上游的项目"]],
		[["Take a screenshot", "截图"], ["Ours and upstream", "我们和上游都有"], ["Kept", "保留"]],
		[["Import media", "导入媒体"], ["Ours and upstream", "我们和上游都有"], ["Kept", "保留"]],
		[["Select mode", "选择模式"], ["A submenu with a text check mark", "子菜单，用文字对勾"], ["Segments in the menu", "菜单里的分段控件"]],
		[["Previous", "最近"], ["Submenu with emoji; greyed out when empty; opens the project", "子菜单带 emoji；空的时候置灰；直接打开项目"], ["Recent, with thumbnails; absent when empty; shows the project in the library", "“最近”，用缩略图；空的时候不显示；在资源库里显示项目"]],
		[["View all recordings, View all screenshots", "查看全部录制文件、查看全部截图"], ["Open settings pages that move to the library", "打开的设置页已移到资源库"], ["One Open library row; the library has the type tabs", "合成一行“打开资源库”；资源库有类型标签"]],
		[["Settings", "设置"], ["Ours and upstream", "我们和上游都有"], ["Kept", "保留"]],
		[["Version", "版本"], ["A greyed row", "一个置灰的行"], ["Right of Quit", "放在退出右侧"]],
		[["Quit", "退出"], ["Ours and upstream", "我们和上游都有"], ["Kept; absent while recording", "保留；录制时不显示"]],
		[["Request permissions", "请求权限"], ["The first-run menu", "首次运行菜单"], ["Continue setup, which opens onboarding", "“继续设置”，打开引导"]],
		[["Shortcuts on rows", "行上的快捷键"], ["Upstream only", "仅上游"], ["Port, as chips", "移植，显示成按键样式"]],
		[["Row icons", "行图标"], ["Upstream only", "仅上游"], ["Port, using our Lucide set", "移植，用我们的 Lucide 图标"]],
		[["Stop by clicking the icon", "点图标停止"], ["Ours and upstream: any mouse button stops", "我们和上游：任何鼠标键都会停止"], ["Left click only; right click opens the menu", "只有左键；右键打开菜单"]],
		[["Pause and resume", "暂停和继续"], ["Not in the tray", "托盘里没有"], ["In the menu", "放进菜单"]],
		[["Upload logs", "上传日志"], ["Upstream, disabled", "上游，已禁用"], ["Dropped: cloud", "去掉：云功能"]],
		[["Mode icon in the menu bar, mode picker", "菜单栏的模式图标、模式选择器"], ["macOS", "macOS"], ["For the macOS work", "留到 macOS 版"]],
	]);

	table("add-table", [["Feature", "功能"], ["Who has it", "谁有"], ["Cost", "代价"], ["Recommendation", "建议"]], [
		[["Record or screenshot the last area", "录制或截取上次区域"], "Greenshot", ["Small: it reuses the area saved by the area picker's lock", "小：复用区域选择锁定时保存的区域"], ["Add", "加入"]],
	]);

	table("ref-table", [["Source", "来源"], ["What it decided here", "决定了什么"]], [
		['<a href="https://learn.microsoft.com/en-us/windows/win32/uxguide/winenv-notification">Notification Area guidelines</a>', ["Left click shows what you most likely want, right click the menu with the default in bold, remove rows that don't apply, no flashing or long animation. Written for Windows 7", "左键显示最可能要的内容，右键打开菜单且默认项加粗，不适用的行要去掉，不闪烁也不做长动画。写于 Windows 7 时代"]],
		['<a href="https://github.com/AniBullet/FrameEver/issues/29">Main window and library</a>', ["Open library, and recent items selecting in the library", "打开资源库，以及最近项目在资源库里选中"]],
		['<a href="https://github.com/AniBullet/FrameEver/issues/53">Command palette</a>', ["What stays available while recording", "录制时还能用什么"]],
		['<a href="https://github.com/AniBullet/FrameEver/issues/57">Recording controls</a>', ["Ring and red square, amber for paused, the still recording dot, hidden controls", "圆环和红方块、暂停的琥珀色、静止的录制圆点、隐藏控制条"]],
		[["Upstream Cap f7d351611", "上游 Cap f7d351611"], ["Row icons, shortcuts on rows, separate screenshot items", "行图标、行上的快捷键、单独的截图项目"]],
		['<a href="https://getgreenshot.org/help/">Greenshot</a>', ["Capturing the last region from the tray", "从托盘截取上次区域"]],
	]);
};

const renderAll = () => {
	copy();
	paint();
};

document.addEventListener("click", (event) => {
	const target = event.target;
	const option = target.closest("[data-tool] [data-v]");
	const mode = target.closest("[data-mode]");
	const action = target.closest("[data-a]");
	const click = target.closest("[data-click]");
	if (option) {
		const name = option.parentElement.dataset.tool;
		if (name === "recent") s.recent = option.dataset.v === "y";
		else s[name] = option.dataset.v;
		s.sub = false;
		if (name === "st" || name === "recent") s.open = true;
	} else if (mode) {
		s.mode = mode.dataset.mode;
		s.log = ["The mode changes and the capture rows follow.", "模式切换，下面的捕获行跟着变。"];
	} else if (target.closest("[data-sub]")) {
		s.sub = !s.sub;
	} else if (action) {
		act(action.dataset.a);
	} else if (target.closest("#tray")) {
		leftClick();
	} else if (click) {
		if (click.dataset.click === "left") leftClick();
		if (click.dataset.click === "right") rightClick();
		if (click.dataset.click === "close") {
			s.open = false;
			s.sub = false;
		}
	} else return;
	paint();
});
document.addEventListener("contextmenu", (event) => {
	if (!event.target.closest("#tray")) return;
	event.preventDefault();
	rightClick();
	paint();
});

shell({ here: "tray", depth: 1, sections: [["menu", "Menu", "菜单"], ["icons", "Icon", "图标"], ["clicks-table", "Clicks", "点击"], ["rules", "Rules", "规则"], ["keyboard", "Keyboard", "键盘"], ["inventory", "Everything", "去向"], ["additions", "Added", "新增"], ["references", "References", "参考"]] });
renderAll();
onChange(renderAll);
