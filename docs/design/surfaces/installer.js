const { L, icon, pick, shell, onChange } = FE;

Object.assign(window.FRAMEEVER_ICONS, {
	"shield-check": '<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="m9 12 2 2 4-4"/>',
});

const GROUPS = [
	[["Install", "安装"], [["welcome", "Welcome", "欢迎"], ["options", "Options", "选项"], ["installing", "Installing", "安装中"], ["running", "App in use", "正在运行"], ["recording", "Recording", "录制中"], ["space", "No space", "空间不足"], ["failed", "Failed", "失败"]]],
	[["Existing", "已安装"], [["older", "Older", "旧版本"], ["same", "Same", "同版本"], ["newer", "Newer", "新版本"]]],
	[["Update", "更新"], [["update", "Updating", "更新中"], ["updatefail", "Failed", "失败"]]],
	[["Uninstall", "卸载"], [["uninstall", "Confirm", "确认"], ["uninstalling", "Removing", "卸载中"], ["removed", "Removed", "已卸载"]]],
	[["Fallback", "回退"], [["fallback", "NSIS", "NSIS"]]],
];
const s = { st: "welcome", pct: 0, details: false, desktop: true, start: true, path: true, delSettings: false, timer: null };
const VERSION = "1.0.0";
const TAGLINE = ["Record, edit and export, all on your own computer.", "录屏、剪辑、导出，全部留在你自己的电脑上。"];
const NAME = ["FrameEver", "帧刻"];

const logo = (size) => `<svg width="${size}" height="${size}" viewBox="0 0 48 48" aria-hidden="true"><rect width="48" height="48" rx="10.8" fill="#1F3FD8"/><rect x="9" y="25" width="30" height="7" rx="3.5" fill="#FFFDED"/><rect x="9" y="34" width="17" height="7" rx="3.5" fill="#87BFFF"/><circle cx="30.5" cy="13.5" r="9.5" fill="#fff"/><circle cx="30.5" cy="13.5" r="7.6" fill="#000F5C"/><circle cx="30.5" cy="13.5" r="6.1" fill="#FF3B30"/></svg>`;
const check = (on) => `<span class="check ${on ? "on" : ""}">${icon("check", "sm")}</span>`;
const option = (key, en, zh, noteEn, noteZh) => `<label class="opt" data-toggle="${key}" tabindex="0">${check(s[key])}<span>${L(en, zh)}${noteEn ? `<small>${L(noteEn, noteZh)}</small>` : ""}</span></label>`;
const optionsBody = () => `<div class="opts"><div><div class="label">${L("Install location", "安装位置")}</div><div class="path"><label class="input"><input id="install-path" value="%LocalAppData%\\Programs\\FrameEver" aria-label="Install location"></label><button class="btn">${L("Change…", "更改…")}</button></div><small style="margin-top:4px" class="tnum">${L("Needs 380 MB · 58 GB free", "需要 380 MB · 可用 58 GB")}</small></div>${option("desktop", "Desktop shortcut", "桌面快捷方式")}${option("start", "Start menu shortcut", "开始菜单快捷方式")}${option("path", "Add frameever to PATH", "把 frameever 加入 PATH", "Run the CLI from any terminal.", "在任意终端里运行 CLI。")}</div>`;
const details = () => (s.details ? '<div class="err fade">NSIS exit code 2 · %TEMP%\\frameever-setup\\install.log</div>' : "");
const keep = (en, zh) => `<div class="keep">${icon("shield-check", "sm")}<span>${L(en, zh)}</span></div>`;

const model = () => {
	const btn = (cls, en, zh, go) => `<button class="btn ${cls}" data-go="${go}">${L(en, zh)}</button>`;
	const detailLink = `<button class="disclose" data-details>${L("Details", "详情")}${icon(s.details ? "chevron-up" : "chevron-down", "sm")}</button>`;
	const installTitle = ["Install FrameEver", "安装帧刻"];
	const updateTitle = ["Update FrameEver", "更新帧刻"];
	const removeTitle = ["Uninstall FrameEver", "卸载帧刻"];
	const pages = {
		welcome: { win: installTitle, hero: true, h: NAME, sub: TAGLINE, actions: btn("primary", "Install", "安装", "installing"), toggle: "options" },
		options: { win: installTitle, hero: true, h: NAME, sub: TAGLINE, body: optionsBody(), actions: btn("primary", "Install", "安装", "installing"), toggle: "welcome", tall: true },
		installing: { win: installTitle, hero: true, h: NAME, sub: TAGLINE, progress: s.pct >= 100 ? ["Opening FrameEver…", "正在打开帧刻…"] : ["Installing…", "正在安装…"], locked: true },
		running: { win: installTitle, h: ["Close FrameEver to continue", "关闭帧刻后继续"], sub: ["FrameEver is open, so its files can't be replaced.", "帧刻正在运行，文件无法替换。"], actions: btn("", "Cancel", "取消", "welcome") + btn("primary", "Close FrameEver", "关闭帧刻", "installing") },
		recording: { win: installTitle, h: ["Finish your recording first", "先结束正在进行的录制"], sub: ["FrameEver is recording. The installer won't close it. Stop the recording, then try again.", "帧刻正在录制，安装器不会关闭它。停止录制后再试一次。"], actions: btn("", "Cancel", "取消", "welcome") + btn("primary", "Try again", "再试一次", "running") },
		space: { win: installTitle, h: ["Couldn't install FrameEver", "无法安装帧刻"], sub: ["Drive C has 120 MB free and FrameEver needs 380 MB. Free up space or choose another drive.", "C 盘还有 120 MB，帧刻需要 380 MB。清理空间，或换一个磁盘。"], actions: btn("", "Change location", "更改位置", "options") + btn("primary", "Try again", "再试一次", "installing") },
		failed: { win: installTitle, h: ["Couldn't install FrameEver", "无法安装帧刻"], sub: ["Setup stopped before it finished. Try again.", "安装中途停止了。再试一次。"], body: details(), left: detailLink, actions: btn("", "Export logs", "导出日志", "failed") + btn("primary", "Try again", "再试一次", "installing"), tall: s.details },
		older: { win: updateTitle, hero: true, h: ["Update to 1.0.0", "更新到 1.0.0"], sub: ["Version 0.9.2 is installed. Location, shortcuts and settings stay as they are.", "已安装 0.9.2。位置、快捷方式和设置都保持不变。"], left: `<button class="disclose" data-go="uninstall">${L("Uninstall instead", "改为卸载")}</button>`, actions: btn("primary", "Update", "更新", "installing") },
		same: { win: installTitle, hero: true, h: ["FrameEver 1.0.0 is installed", "已安装帧刻 1.0.0"], sub: ["Reinstall it to repair missing files. Settings stay.", "重新安装可以修复缺失的文件，设置保留。"], left: `<button class="disclose" data-go="uninstall">${L("Uninstall", "卸载")}</button>`, actions: btn("primary", "Reinstall", "重新安装", "installing") },
		newer: { win: installTitle, hero: true, h: ["A newer version is installed", "已安装更新的版本"], sub: ["FrameEver 1.1.0 is on this computer. This installer is 1.0.0.", "这台电脑上是帧刻 1.1.0，这个安装程序是 1.0.0。"], actions: btn("", "Install 1.0.0", "安装 1.0.0", "installing") + btn("primary", "Keep 1.1.0", "保留 1.1.0", "welcome") },
		updatefail: { h: ["Couldn't update FrameEver", "无法更新帧刻"], sub: ["Try again, or get the installer from GitHub Releases.", "再试一次，或从 GitHub Releases 下载安装程序。"], left: detailLink, body: details(), actions: btn("", "Open Releases", "打开 Releases", "updatefail") + btn("primary", "Try again", "再试一次", "update") },
		uninstall: { win: removeTitle, h: ["Uninstall FrameEver?", "卸载帧刻？"], sub: ["This removes the app, its shortcuts and its PATH entry.", "这会移除应用、快捷方式和 PATH 项。"], body: `${keep("Recordings, screenshots and exports stay in D:\\Videos\\FrameEver.", "录制、截图和导出保留在 D:\\Videos\\FrameEver。")}${option("delSettings", "Also delete settings", "同时删除设置", "Settings and logs in the app data folder.", "应用数据文件夹里的设置和日志。")}`, actions: btn("", "Cancel", "取消", "welcome") + btn("danger", "Uninstall", "卸载", "uninstalling") },
		uninstalling: { win: removeTitle, h: ["Uninstalling FrameEver", "正在卸载帧刻"], sub: ["Your recordings stay where they are.", "你的录制保留在原处。"], progress: ["Uninstalling…", "正在卸载…"], locked: true },
		removed: { win: removeTitle, h: ["FrameEver is uninstalled", "帧刻已卸载"], sub: ["Your recordings, screenshots and exports are still in D:\\Videos\\FrameEver.", "你的录制、截图和导出还在 D:\\Videos\\FrameEver。"], actions: btn("", "Open folder", "打开文件夹", "removed") + btn("primary", "Close", "关闭", "welcome") },
	};
	return pages[s.st];
};

const titlebar = (c) => `<div class="tb">${logo(16)}<span>${L(...c.win)}</span><span class="grow"></span><b>${icon("minus", "sm")}</b><b class="${c.locked ? "off" : ""}" title="${c.locked ? pick("Can't be closed while files are being copied", "复制文件时不能关闭") : ""}">${icon("x", "sm")}</b></div>`;
const progressBar = (c) => `<div class="pwrap"><div class="pmeta"><b>${L(...c.progress)}</b><span>${s.pct}%</span></div><div class="progress"><i style="width:${s.pct}%"></i></div></div>`;
const toggle = (c) => (c.toggle ? `<button class="disclose" data-go="${c.toggle}">${L("Options", "选项")}${icon(s.st === "options" ? "chevron-up" : "chevron-down", "sm")}</button>` : "");
const mainWin = (c) => {
	const body = c.body ? `<div class="slot fade">${c.body}</div>` : "";
	const bottom = c.progress ? progressBar(c) : `<div class="acts">${c.actions ?? ""}</div>`;
	return `<div class="win A" style="width:440px;height:${c.tall ? 600 : 400}px">${titlebar(c)}<div class="main fade">${logo(c.hero ? 72 : 40)}<h3>${L(...c.h)}</h3><p class="sub">${L(...c.sub)}</p>${body}<div class="bottom">${bottom}${toggle(c)}${c.left ?? ""}</div></div></div>`;
};
const updateWin = (c) =>
	s.st === "update"
		? `<div class="win" style="width:400px"><div class="up"><div class="top">${logo(32)}<div><b>${L("Updating FrameEver…", "正在更新帧刻…")}</b><span class="small tnum">0.9.2 → 1.0.0</span></div><span class="small tnum">${s.pct}%</span></div><div class="progress"><i style="width:${s.pct}%"></i></div></div></div>`
		: `<div class="win" style="width:400px"><div class="up"><div class="top">${logo(32)}<div><b>${L(...c.h)}</b><span class="small">${L(...c.sub)}</span></div></div>${c.body ?? ""}<div class="acts" style="justify-content:flex-end;align-items:center">${c.left}<span style="flex:1"></span>${c.actions}</div></div></div>`;
const nsisWin = () => `<div class="nsis"><div class="cap"><span style="display:flex;gap:6px;align-items:center">${logo(16)}FrameEver ${VERSION}</span><span style="width:46px;text-align:center">✕</span></div><div class="pane"><div class="side">${logo(48)}<b>FrameEver</b><i></i></div><div class="txt"><h5>${L("Welcome to FrameEver Setup", "欢迎使用帧刻安装程序")}</h5><p>${L("Setup will guide you through the installation of FrameEver.", "安装程序将引导你完成帧刻的安装。")}</p><p>${L("Click Next to continue.", "单击“下一步”继续。")}</p></div></div><div class="btns"><span>${L("Next >", "下一步(N) >")}</span><span>${L("Cancel", "取消(C)")}</span></div></div>`;

const narrate = {
	welcome: ["Enter installs with every default. Options is one click away. There is no licence page and no language prompt.", "按 Enter 就用默认值安装。选项在一次点击之内。没有许可页，也不问语言。"],
	options: ["The window snaps to 600 px and the options fade in. Checkboxes, not switches: nothing happens until Install. Space needed updates when the location changes.", "窗口跳到 600 px，选项淡入。用复选框而不是开关：按安装之前什么都不会发生。改位置时重新计算所需空间。"],
	installing: ["The bar takes the button's place. Real percentage, one bar that never moves back. Close is off while files copy. At 100% FrameEver opens and this window goes.", "进度条出现在按钮的位置。真实百分比，一根进度条，不回退。复制文件时不能关闭。到 100% 打开帧刻，窗口随即消失。"],
	running: ["Restart Manager finds what holds the files, including the CLI. Close FrameEver quits it normally.", "用 Restart Manager 找出占用文件的程序，包括 CLI。“关闭帧刻”是正常退出。"],
	recording: ["The installer never closes a recording. The in-app updater already waits for one to finish.", "安装器绝不关闭录制中的应用。应用内更新本来就会等录制结束。"],
	space: ["Checked before anything is copied, so a full disk never leaves half an install.", "复制之前就检查，磁盘满了不会留下装到一半的文件。"],
	failed: ["What happened and what to do, raw text behind Details, Export logs as the way out.", "说清发生了什么、接下来怎么做，原始信息收在详情里，出路是导出日志。"],
	older: ["Replaces NSIS's “uninstall before installing” page. Updating is the primary action, with the old location and shortcuts kept.", "替代 NSIS 的“安装前先卸载”页。更新是主操作，沿用原来的位置和快捷方式。"],
	same: ["The reinstall or uninstall choice from #31, as one action plus one link.", "#31 定下的重新安装或卸载，做成一个主操作加一个链接。"],
	newer: ["Keeping the newer version is the safe default. Downgrading stays possible.", "保留新版本是安全的默认值。降级仍然可以做。"],
	update: ["Update mode: no title bar, progress only, doesn't take focus. FrameEver relaunches at 100%.", "更新模式：没有标题栏，只有进度，不抢焦点。到 100% 重新打开帧刻。"],
	updatefail: ["The small window stays until you act, so a failed update doesn't vanish silently.", "小窗口会留着等你处理，更新失败不会悄悄消失。"],
	uninstall: ["The storage folder line comes first and shows the real path. Hand-drawn never appears on delete dialogs.", "存储文件夹那一行放在最前，显示真实路径。删除对话框不用手绘。"],
	uninstalling: ["Same bar as installing. The exe runs from a temp copy so it can remove itself.", "和安装同样的进度条。程序从临时副本运行，才能删除自己。"],
	removed: ["Nothing else on screen confirms it, so this page does, and offers the folder that was kept.", "屏幕上没有别的东西能确认结果，所以由这一页确认，并提供保留下来的文件夹。"],
	fallback: ["Only when GPUI can't start. See Fallback below.", "只在 GPUI 启动不了时出现。见下面的“回退”。"],
};

const pageRows = [
	[["Welcome", "欢迎"], ["No FrameEver found", "没找到帧刻"], ["440 × 400. 72 px icon, name, one sentence, Install at 40 px, Options below it", "440 × 400。72 px 图标、名字、一句话，40 px 高的安装按钮，下面是选项"]],
	[["Options", "选项"], ["Options pressed", "按下选项"], ["440 × 600. Location with space needed, three checkboxes, all on", "440 × 600。安装位置和所需空间，三个复选框，默认全开"]],
	[["Installing", "安装中"], ["Install pressed", "按下安装"], ["Bar and label where the button was. The window keeps its size", "进度条和文字出现在按钮的位置。窗口保持原来的尺寸"]],
	[["App in use", "正在运行"], ["Files are held and no recording runs", "文件被占用，且没有在录制"], ["40 px icon, heading, sentence. Close FrameEver, Cancel", "40 px 图标、标题、一句话。关闭帧刻、取消"]],
	[["Recording", "录制中"], ["work_in_flight is true", "work_in_flight 为真"], ["Same layout. Try again, Cancel. Never closes the app", "同样的布局。再试一次、取消。绝不关闭应用"]],
	[["No space", "空间不足"], ["Free space is below what's needed, before copying", "复制前发现可用空间不够"], ["Change location, Try again", "更改位置、再试一次"]],
	[["Failed", "失败"], ["NSIS returns an error", "NSIS 返回错误"], ["Details, Export logs, Try again", "详情、导出日志、再试一次"]],
	[["Older installed", "已装旧版本"], ["Installed version is lower", "已安装的版本更低"], ["Update, Uninstall instead. Options is hidden: location stays", "更新、改为卸载。没有选项：位置不变"]],
	[["Same installed", "已装同版本"], ["Versions match", "版本相同"], ["Reinstall, Uninstall", "重新安装、卸载"]],
	[["Newer installed", "已装新版本"], ["Installed version is higher", "已安装的版本更高"], ["Keep 1.1.0 closes the window, Install 1.0.0 downgrades", "保留 1.1.0 关闭窗口，安装 1.0.0 就是降级"]],
	[["Update", "更新"], ["The in-app updater starts update mode", "应用内更新启动更新模式"], ["400 px strip, frameless, doesn't take focus, centred on the primary display", "400 px 宽的小条，无边框，不抢焦点，位于主显示器中央"]],
	[["Update failed", "更新失败"], ["NSIS returns an error", "NSIS 返回错误"], ["Details, Open Releases, Try again", "详情、打开 Releases、再试一次"]],
	[["Uninstall", "卸载"], ["The Windows uninstall entry", "Windows 的卸载入口"], ["Storage folder line, Also delete settings off, Uninstall in danger colour, Cancel", "存储文件夹那一行、同时删除设置默认关、危险色的卸载按钮、取消"]],
	[["Removed", "已卸载"], ["Uninstall finished", "卸载完成"], ["Open folder, Close", "打开文件夹、关闭"]],
];
const ruleRows = [
	[["Tokens", "Token"], ["Klein Field dark or light by Windows mode. Inter and HarmonyOS Sans SC embedded. Klein blue only on the icon, the primary button and the bar, all already on the list", "按 Windows 深浅色用 Klein Field，内嵌 Inter 和 HarmonyOS Sans SC。Klein 蓝只出现在图标、主按钮和进度条，都在白名单里"]],
	[["Language", "语言"], ["Windows display language at start: Simplified Chinese on zh-CN, English otherwise. Written to settings as the first language", "启动时读取 Windows 显示语言：zh-CN 用简体中文，其他用英文。写入设置作为首次界面语言"]],
	[["Motion", "动效"], ["Pages fade in (duration-mid). The bar's width follows real progress with ease-effects, no overshoot. The window snaps, only its content moves. Windows's “Show animations” off means no fades", "页面淡入（duration-mid）。进度条宽度跟随真实进度，用 ease-effects，不回弹。窗口直接跳到新尺寸，只有内容在动。Windows 关闭“显示动画”时不淡入"]],
	[["Hand-drawn", "手绘"], ["None. Hand-drawn stays in the nine places on the tokens page, and FrameEver's welcome page follows right after", "不用。手绘留在 Token 页列出的九处，帧刻的欢迎页紧接着就会出现"]],
	[["Closing", "关闭"], ["Close works everywhere except while files copy. NSIS silent mode has no clean stop, so there is no Cancel there", "除了复制文件时都能关闭。NSIS 静默模式没有干净的中止方式，所以那时没有取消"]],
	[["Progress", "进度"], ["Percentage only, never a time estimate. One bar for the whole job, so it never moves back. The taskbar button shows it too, and turns red on failure", "只显示百分比，不给时间估计。一根进度条走完全程，不回退。任务栏按钮同步显示，失败时变红"]],
	[["After install", "安装之后"], ["At 100% FrameEver opens and the window closes. There is no finish page. If it can't open, the page says so with Try again", "到 100% 打开帧刻并关闭窗口，没有完成页。打不开时页面说明情况并提供再试一次"]],
	[["Command line", "命令行"], ["The download keeps /S and /D= for scripts and winget. With /S no window appears and FrameEver doesn't open", "下载的文件保留 /S 和 /D=，给脚本和 winget 用。用 /S 时没有窗口，也不打开帧刻"]],
	[["PATH", "PATH"], ["The option uses the same cli-install crate as Settings, so Settings shows the true state afterwards", "这个选项和设置页共用 cli-install crate，之后设置页显示的状态是准确的"]],
];
const keyRows = [
	["Enter", ["The primary button", "主按钮"]],
	["Esc", ["Cancel, or close options. Nothing while files copy", "取消，或收起选项。复制文件时无效"]],
	["Tab", ["Moves through the controls. Focus starts on the primary button", "在控件之间移动。焦点从主按钮开始"]],
	["Space", ["Toggles the focused checkbox or Options", "切换聚焦的复选框或选项"]],
];
const fallbackRows = [
	[["When", "时机"], ["GPUI can't start: no usable GPU path, remote desktop or a VM. No message; the NSIS window simply appears", "GPUI 启动不了：没有可用的 GPU、远程桌面或虚拟机。没有提示，直接出现 NSIS 窗口"]],
	[["Install", "安装"], ["NSIS with its own pages, light only, Windows controls", "NSIS 自己的页面，只有浅色，Windows 控件"]],
	[["Update", "更新"], ["NSIS with /P /R /UPDATE, exactly as today", "按现在的方式用 /P /R /UPDATE 运行 NSIS"]],
	[["Uninstall", "卸载"], ["NSIS's own pages", "NSIS 自己的页面"]],
	[["The one change", "唯一的改动"], ["render_brand.py swaps the sidebar's blue-tinted SURFACE for the near-achromatic neutral, then rebuilds both bitmaps. Nothing else is designed", "render_brand.py 把侧栏偏蓝的 SURFACE 换成近乎无彩的灰，再重新生成两张位图。其余不再设计"]],
];
const invRows = [
	[["Language selector", "语言选择"], ["Removed. Follows Windows", "去掉。跟随 Windows"]],
	[["Welcome and finish pages", "欢迎页和完成页"], ["Welcome is one page with Install. Finish is gone", "欢迎页和安装合成一页。完成页去掉"]],
	[["Reinstall page", "重新安装页"], ["Three pages by version: older, same, newer", "按版本分三页：旧、同、新"]],
	[["Install directory", "安装目录"], ["Install location under Options", "选项里的安装位置"]],
	[["Desktop shortcut on the finish page", "完成页的桌面快捷方式"], ["Desktop shortcut under Options, on", "选项里的桌面快捷方式，默认开"]],
	[["Run app on the finish page", "完成页的运行应用"], ["FrameEver always opens", "总是打开帧刻"]],
	[["Delete app data on uninstall", "卸载时删除应用数据"], ["Also delete settings, off, never touching the storage folder", "同时删除设置，默认关，绝不碰存储文件夹"]],
	[["App running prompt", "应用运行中的提示"], ["App in use and Recording pages", "正在运行页和录制中页"]],
	[["/P /R /UPDATE", "/P /R /UPDATE"], ["Update mode, with NSIS as its fallback", "更新模式，NSIS 作为回退"]],
	[["/S and /D=", "/S 和 /D="], ["Kept on the download", "下载的文件上保留"]],
];
const addRows = [
	[["Recording guard", "录制保护"], ["The installer can be started by hand during a recording. It must not end it", "录制时可以手动运行安装程序，它不能结束录制"], ["Our own rule; the updater already has it", "我们自己的规则，更新已经这样做"]],
	[["Space check first", "先检查空间"], ["A full disk stops before anything changes", "磁盘满时在任何改动之前停下"], ["Space needed under Options comes from the same check", "选项里的所需空间来自同一项检查"]],
	[["Restart Manager", "Restart Manager"], ["Finds any process holding the files, not only FrameEver.exe", "找出所有占用文件的进程，而不只是 FrameEver.exe"], ["Replaces CheckIfAppIsRunning", "替代 CheckIfAppIsRunning"]],
	[["Taskbar progress", "任务栏进度"], ["Minimised or behind other windows, progress stays visible", "最小化或被挡住时也能看到进度"], ["ITaskbarList3", "ITaskbarList3"]],
	[["Open folder after uninstall", "卸载后打开文件夹"], ["Shows the recordings really stayed", "让你看到录制真的留下了"], ["Our own addition", "我们自己加的"]],
];
const refRows = [
	["Fluent 2 progress bar", "Fluent 2 进度条", "Determinate where possible, one bar for related steps, -ing label with …, no time estimates, the error replaces the status text", "尽量用确定进度，相关步骤共用一根，用“正在…”，不给时间估计，错误替换状态文字", "https://fluent2.microsoft.design/components/web/react/core/progressbar/usage"],
	["Windows Restart Manager", "Windows Restart Manager", "Custom installers can use it to find and close what holds a file, instead of asking for a reboot", "自定义安装器可以用它找到并关闭占用文件的程序，不用要求重启", "https://learn.microsoft.com/en-us/windows/win32/rstmgr/about-restart-manager"],
	["Tauri's NSIS template", "Tauri 的 NSIS 模板", "The full list of pages and existing-install cases we keep or replace", "我们保留或替换的页面和已安装情况的完整清单", "https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi"],
	["WixUI minimal dialog", "WixUI 极简对话框", "One-page install is an established shape, between one click and a full wizard", "单页安装是成熟的形态，介于一键安装和完整向导之间", "https://firegiant.com/wix/tutorial/user-interface/ui-wizardry"],
	["Upstream Cap at f7d351611", "上游 Cap，f7d351611", "No installer design change since our merge; still stock NSIS. Its Settings CLI install is already in our cli-install crate", "我们合并之后安装器设计没有变化，仍是默认 NSIS。设置页安装 CLI 的功能已在我们的 cli-install crate 里", "https://github.com/CapSoftware/Cap"],
];

const tableOf = (rows, head) => `<table>${head ? `<thead><tr>${head.map((h) => `<th>${L(...h)}</th>`).join("")}</tr></thead>` : ""}<tbody>${rows.join("")}</tbody></table>`;
const fill = (id, html) => {
	document.getElementById(id).innerHTML = html;
};
const titles = {
	windows: ["Windows", "窗口", "One column, chosen on #62. One window, one primary button, and the same place for what happens next. The buttons in the window move between pages; every page is in the rows below.", "在 #62 选定的单栏方案。一个窗口、一个主按钮，下一步总在同一个位置。窗口里的按钮可以走流程，所有页面都在下面这几行。"],
	pages: ["Every page", "所有页面", "What shows it and what it holds.", "什么情况下出现，上面有什么。"],
	rules: ["Rules", "规则", "These apply to every page.", "每一页都遵守。"],
	keys: ["Keyboard", "键盘", "Every action works without a mouse.", "所有操作都不需要鼠标。"],
	fallback: ["Fallback", "回退", "ADR-0009 keeps NSIS's own UI so installing always works. It isn't designed further.", "ADR-0009 保留 NSIS 自己的界面，保证总能安装，不再进一步设计。"],
	inv: ["Everything in the installer today", "现在安装器里的所有东西", "Nothing the current NSIS installer does is lost.", "现在的 NSIS 安装器能做的事都没有丢。"],
	add: ["Additions", "新增", "Beyond the list in #31.", "#31 之外加入的内容。"],
	ref: ["References", "参考", "Current sources on installer design are thin: searches returned mostly vendor pages, and nothing on how Figma, Discord or Linear look on Windows.", "安装器设计的最新资料很少：搜索到的大多是厂商页面，没有 Figma、Discord、Linear 在 Windows 上的安装界面。"],
};
const renderStatic = () => {
	for (const [key, [en, zh, leadEn, leadZh]] of Object.entries(titles)) {
		fill(`t-${key}`, L(en, zh));
		fill(`l-${key}`, L(leadEn, leadZh));
	}
	fill("page-table", tableOf(pageRows.map(([a, b, c]) => `<tr><th>${L(...a)}</th><td>${L(...b)}</td><td>${L(...c)}</td></tr>`), [["Page", "页面"], ["Shows when", "出现时机"], ["Holds", "内容"]]));
	fill("rule-table", tableOf(ruleRows.map(([a, b]) => `<tr><th>${L(...a)}</th><td>${L(...b)}</td></tr>`)));
	fill("key-table", tableOf(keyRows.map(([k, v]) => `<tr><th><span class="kbd">${k}</span></th><td>${L(...v)}</td></tr>`)));
	fill("fallback-table", tableOf(fallbackRows.map(([a, b]) => `<tr><th>${L(...a)}</th><td>${L(...b)}</td></tr>`)));
	fill("inv-table", tableOf(invRows.map(([a, b]) => `<tr><th>${L(...a)}</th><td>${L(...b)}</td></tr>`), [["Today", "现在"], ["Goes to", "去向"]]));
	fill("add-table", tableOf(addRows.map(([a, b, c]) => `<tr><th>${L(...a)}</th><td>${L(...b)}</td><td>${L(...c)}</td></tr>`), [["Addition", "新增"], ["Why", "原因"], ["Source", "来源"]]));
	fill("ref-table", tableOf(refRows.map(([a, b, c, d, url]) => `<tr><th><a href="${url}" target="_blank" rel="noreferrer">${L(a, b)}</a></th><td>${L(c, d)}</td></tr>`)));
};

const tick = () => {
	clearInterval(s.timer);
	s.timer = null;
	if (!["installing", "uninstalling", "update"].includes(s.st)) return;
	s.pct = 0;
	s.timer = setInterval(() => {
		s.pct = Math.min(100, s.pct + 1 + Math.floor(Math.random() * 3));
		if (s.pct >= 100) {
			clearInterval(s.timer);
			s.timer = null;
			if (s.st === "uninstalling") setTimeout(() => go("removed"), 500);
		}
		draw();
	}, 90);
};
const draw = () => {
	const desk = document.getElementById("desk");
	const c = model();
	if (s.st === "fallback") desk.innerHTML = nsisWin();
	else if (s.st === "update" || s.st === "updatefail") desk.innerHTML = updateWin(c);
	else desk.innerHTML = mainWin(c);
	const win = desk.firstElementChild;
	win.style.zoom = "";
	const room = (desk.clientWidth - 24) / win.offsetWidth;
	if (room < 1) win.style.zoom = room;
	fill("line", L(...narrate[s.st]));
};
const render = () => {
	fill("flow", GROUPS.map(([g, items]) => `<div class="row"><span>${L(...g)}</span>${items.map(([k, en, zh]) => `<button class="chip ${k === s.st ? "on" : ""}" data-go="${k}">${L(en, zh)}</button>`).join("")}</div>`).join(""));
	draw();
};
const go = (st) => {
	s.st = st;
	s.details = false;
	tick();
	render();
};
document.addEventListener("click", (event) => {
	const toggleEl = event.target.closest("[data-toggle]");
	if (toggleEl) {
		event.preventDefault();
		s[toggleEl.dataset.toggle] = !s[toggleEl.dataset.toggle];
		draw();
		return;
	}
	if (event.target.closest("[data-details]")) {
		s.details = !s.details;
		draw();
		return;
	}
	const goEl = event.target.closest("[data-go]");
	if (goEl) go(goEl.dataset.go);
});
window.addEventListener("resize", draw);

shell({
	here: "installer",
	depth: 1,
	sections: [["windows", "Windows", "窗口"], ["pages", "Every page", "所有页面"], ["rules", "Rules", "规则"], ["keyboard", "Keyboard", "键盘"], ["fallback-rules", "Fallback", "回退"], ["inventory", "Today", "现状"], ["additions", "Additions", "新增"], ["references", "References", "参考"]],
});
renderStatic();
onChange(render);
render();
