const { L, icon, pick, shell, onChange } = FE;

Object.assign(window.FRAMEEVER_ICONS, {
	"shield-check": '<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="m9 12 2 2 4-4"/>',
});

const VARIANTS = {
	A: ["One column", "单栏"],
	B: ["Klein plate", "Klein 色板"],
	C: ["Doodle stage", "手绘舞台"],
};
const GROUPS = [
	[["Install", "安装"], [["welcome", "Welcome", "欢迎"], ["options", "Options", "选项"], ["installing", "Installing", "安装中"], ["running", "App in use", "正在运行"], ["recording", "Recording", "录制中"], ["space", "No space", "空间不足"], ["failed", "Failed", "失败"]]],
	[["Existing", "已安装"], [["older", "Older", "旧版本"], ["same", "Same", "同版本"], ["newer", "Newer", "新版本"]]],
	[["Update", "更新"], [["update", "Updating", "更新中"], ["updatefail", "Failed", "失败"]]],
	[["Uninstall", "卸载"], [["uninstall", "Confirm", "确认"], ["uninstalling", "Removing", "卸载中"], ["removed", "Removed", "已卸载"]]],
	[["Fallback", "回退"], [["fallback", "NSIS", "NSIS"]]],
];
const initial = () => {
	try {
		return new URLSearchParams(location.search).get("variant");
	} catch {
		return null;
	}
};
const s = { variant: VARIANTS[initial()] ? initial() : "A", st: "welcome", pct: 0, details: false, desktop: true, start: true, path: true, delSettings: false, uninstallAction: "reinstall", timer: null, done: false };
const VERSION = "1.0.0";

const logo = (size) => `<svg width="${size}" height="${size}" viewBox="0 0 48 48" aria-hidden="true"><rect width="48" height="48" rx="10.8" fill="#1F3FD8"/><rect x="9" y="25" width="30" height="7" rx="3.5" fill="#FFFDED"/><rect x="9" y="34" width="17" height="7" rx="3.5" fill="#87BFFF"/><circle cx="30.5" cy="13.5" r="9.5" fill="#fff"/><circle cx="30.5" cy="13.5" r="7.6" fill="#000F5C"/><circle cx="30.5" cy="13.5" r="6.1" fill="#FF3B30"/></svg>`;
const SCREEN = [
	["M 18 12 L 102 12 Q 110 12 110 20 L 110 66 Q 110 74 102 74 L 18 74 Q 10 74 10 66 L 10 20 Q 10 12 18 12 Z", "foreground", 4, 0, 0.6],
	["M 52 74 L 49 88 M 68 74 L 71 88 M 40 90 L 80 90", "foreground", 4, 0.6, 0.8],
	["M 24 24 L 96 24 L 96 62 L 24 62 Z", "link", 2.5, 0.8, 0.95],
];
const doodle = (width, pct) => {
	const draw = (from, to) => Math.min(1, Math.max(0, (pct / 100 - from) / (to - from)));
	const dot = pct >= 95 ? '<circle cx="33" cy="34" r="4.5" fill="var(--recording)"/>' : "";
	return `<svg width="${width}" viewBox="0 0 120 100" aria-hidden="true">${SCREEN.map(([d, ink, w, a, b]) => `<path d="${d}" fill="none" stroke="var(--${ink})" stroke-width="${w}" stroke-linecap="round" stroke-linejoin="round" pathLength="1" stroke-dasharray="1 2" stroke-dashoffset="${1 - draw(a, b)}"/>`).join("")}${dot}</svg>`;
};
const check = (on) => `<span class="check ${on ? "on" : ""}">${icon("check", "sm")}</span>`;
const option = (key, en, zh, noteEn, noteZh) => `<label class="opt" data-toggle="${key}" tabindex="0">${check(s[key])}<span>${L(en, zh)}${noteEn ? `<small>${L(noteEn, noteZh)}</small>` : ""}</span></label>`;
const optionsBody = () => `<div class="opts"><div><div class="label">${L("Install location", "安装位置")}</div><div class="path"><label class="input"><input value="C:\\Users\\Ada\\AppData\\Local\\Programs\\FrameEver" aria-label="Install location"></label><button class="btn">${L("Change…", "更改…")}</button></div><small style="margin-top:4px" class="tnum">${L("Needs 380 MB · 58 GB free", "需要 380 MB · 可用 58 GB")}</small></div>${option("desktop", "Desktop shortcut", "桌面快捷方式")}${option("start", "Start menu shortcut", "开始菜单快捷方式")}${option("path", "Add frameever to PATH", "把 frameever 加入 PATH", "Run the CLI from any terminal.", "在任意终端里运行 CLI。")}</div>`;
const details = () => (s.details ? `<div class="err fade">NSIS exit code 2 · C:\\Users\\Ada\\AppData\\Local\\Temp\\frameever-setup\\install.log</div>` : "");
const keep = (en, zh) => `<div class="keep">${icon("shield-check", "sm")}<span>${L(en, zh)}</span></div>`;

const model = () => {
	const btn = (cls, en, zh, go) => `<button class="btn ${cls}" data-go="${go}">${L(en, zh)}</button>`;
	const detailLinks = `<button class="disclose" data-details>${L("Details", "详情")}${icon(s.details ? "chevron-up" : "chevron-down", "sm")}</button>`;
	const installTitle = ["Install FrameEver", "安装帧刻"];
	const m = {
		welcome: { win: installTitle, hero: true, h: ["FrameEver", "帧刻"], sub: ["Record, edit and export, all on your own computer.", "录屏、剪辑、导出，全部留在你自己的电脑上。"], actions: btn("primary", "Install", "安装", "installing"), toggle: "options" },
		options: { win: installTitle, hero: true, h: ["FrameEver", "帧刻"], sub: ["Record, edit and export, all on your own computer.", "录屏、剪辑、导出，全部留在你自己的电脑上。"], body: optionsBody(), actions: btn("primary", "Install", "安装", "installing"), toggle: "welcome", tall: true, sheet: true },
		installing: { win: installTitle, hero: true, h: ["FrameEver", "帧刻"], sub: ["Record, edit and export, all on your own computer.", "录屏、剪辑、导出，全部留在你自己的电脑上。"], progress: s.pct >= 100 ? ["Opening FrameEver…", "正在打开帧刻…"] : ["Installing…", "正在安装…"], locked: true },
		running: { win: installTitle, h: ["Close FrameEver to continue", "关闭帧刻后继续"], sub: ["FrameEver is open, so its files can't be replaced.", "帧刻正在运行，文件无法替换。"], actions: btn("", "Cancel", "取消", "welcome") + btn("primary", "Close FrameEver", "关闭帧刻", "installing") },
		recording: { win: installTitle, h: ["Finish your recording first", "先结束正在进行的录制"], sub: ["FrameEver is recording. The installer won't close it. Stop the recording, then try again.", "帧刻正在录制，安装器不会关闭它。停止录制后再试一次。"], actions: btn("", "Cancel", "取消", "welcome") + btn("primary", "Try again", "再试一次", "running") },
		space: { win: installTitle, h: ["Couldn't install FrameEver", "无法安装帧刻"], sub: ["Drive C has 120 MB free and FrameEver needs 380 MB. Free up space or choose another drive.", "C 盘还有 120 MB，帧刻需要 380 MB。清理空间，或换一个磁盘。"], actions: btn("", "Change location", "更改位置", "options") + btn("primary", "Try again", "再试一次", "installing") },
		failed: { win: installTitle, h: ["Couldn't install FrameEver", "无法安装帧刻"], sub: ["Setup stopped before it finished. Try again.", "安装中途停止了。再试一次。"], body: details(), left: detailLinks, actions: btn("", "Export logs", "导出日志", "failed") + btn("primary", "Try again", "再试一次", "installing"), tall: s.details },
		older: { win: ["Update FrameEver", "更新帧刻"], hero: true, h: ["Update to 1.0.0", "更新到 1.0.0"], sub: ["Version 0.9.2 is installed. Location, shortcuts and settings stay as they are.", "已安装 0.9.2。位置、快捷方式和设置都保持不变。"], left: `<button class="disclose" data-go="uninstall">${L("Uninstall instead", "改为卸载")}</button>`, actions: btn("primary", "Update", "更新", "installing") },
		same: { win: installTitle, hero: true, h: ["FrameEver 1.0.0 is installed", "已安装帧刻 1.0.0"], sub: ["Reinstall it to repair missing files. Settings stay.", "重新安装可以修复缺失的文件，设置保留。"], left: `<button class="disclose" data-go="uninstall">${L("Uninstall", "卸载")}</button>`, actions: btn("primary", "Reinstall", "重新安装", "installing") },
		newer: { win: installTitle, hero: true, h: ["A newer version is installed", "已安装更新的版本"], sub: ["FrameEver 1.1.0 is on this computer. This installer is 1.0.0.", "这台电脑上是帧刻 1.1.0，这个安装程序是 1.0.0。"], actions: btn("", "Install 1.0.0", "安装 1.0.0", "installing") + btn("primary", "Keep 1.1.0", "保留 1.1.0", "welcome") },
		updatefail: { compact: true, win: ["Update FrameEver", "更新帧刻"], h: ["Couldn't update FrameEver", "无法更新帧刻"], sub: ["Try again, or get the installer from GitHub Releases.", "再试一次，或从 GitHub Releases 下载安装程序。"], left: detailLinks, body: details(), actions: btn("", "Open Releases", "打开 Releases", "updatefail") + btn("primary", "Try again", "再试一次", "update") },
		uninstall: { win: ["Uninstall FrameEver", "卸载帧刻"], h: ["Uninstall FrameEver?", "卸载帧刻？"], sub: ["This removes the app, its shortcuts and its PATH entry.", "这会移除应用、快捷方式和 PATH 项。"], body: `${keep("Recordings, screenshots and exports stay in D:\\Videos\\FrameEver.", "录制、截图和导出保留在 D:\\Videos\\FrameEver。")}${option("delSettings", "Also delete settings", "同时删除设置", "Settings and logs in the app data folder.", "应用数据文件夹里的设置和日志。")}`, actions: btn("", "Cancel", "取消", "welcome") + btn("danger", "Uninstall", "卸载", "uninstalling"), tall: true },
		uninstalling: { win: ["Uninstall FrameEver", "卸载帧刻"], h: ["Uninstalling FrameEver", "正在卸载帧刻"], sub: ["Your recordings stay where they are.", "你的录制保留在原处。"], progress: ["Uninstalling…", "正在卸载…"], locked: true },
		removed: { win: ["Uninstall FrameEver", "卸载帧刻"], h: ["FrameEver is uninstalled", "帧刻已卸载"], sub: ["Your recordings, screenshots and exports are still in D:\\Videos\\FrameEver.", "你的录制、截图和导出还在 D:\\Videos\\FrameEver。"], actions: btn("", "Open folder", "打开文件夹", "removed") + btn("primary", "Close", "关闭", "welcome") },
	};
	return m[s.st];
};

const titlebar = (c, showTitle = true) => `<div class="tb">${showTitle ? `${logo(16)}<span>${L(...c.win)}</span>` : ""}<span class="grow"></span><b>${icon("minus", "sm")}</b><b class="${c.locked ? "off" : ""}" title="${c.locked ? pick("Can't be closed while files are being copied", "复制文件时不能关闭") : ""}">${icon("x", "sm")}</b></div>`;
const progress = (c, wrapClass = "") => `<div class="${wrapClass}"><div class="pmeta"><b>${L(...c.progress)}</b><span>${s.pct}%</span></div><div class="progress"><i style="width:${s.pct}%"></i></div></div>`;
const toggle = (c) => (c.toggle ? `<button class="disclose" data-go="${c.toggle}">${L("Options", "选项")}${icon(s.st === "options" ? "chevron-up" : "chevron-down", "sm")}</button>` : "");

const varA = (c) => {
	const height = c.tall ? 600 : 400;
	const body = c.body ? `<div class="slot fade">${c.body}</div>` : "";
	const bottom = c.progress ? progress(c, "pwrap") : `<div class="acts">${c.actions ?? ""}</div>`;
	return `<div class="win A" style="width:440px;height:${height}px">${titlebar(c)}<div class="main fade">${c.hero ? logo(72) : logo(40)}<h3>${L(...c.h)}</h3><p class="sub">${L(...c.sub)}</p>${body}<div class="bottom">${bottom}${toggle(c)}${c.left ?? ""}</div></div></div>`;
};
const varB = (c) => {
	const body = c.body ? `<div class="slot fade">${c.body}</div>` : "";
	const bottom = c.progress ? `<div class="bottom full">${progress(c)}</div>` : `<div class="bottom">${toggle(c)}${c.left ?? ""}<div class="acts">${c.actions ?? ""}</div></div>`;
	return `<div class="win B" style="width:680px;height:${c.tall ? 440 : 400}px"><div class="plate"><div class="tb-p">${L(...c.win)}</div>${logo(64)}<strong>FrameEver</strong><p>${L("Screen recorder and editor", "录屏和剪辑")}</p><span class="ver">${VERSION}</span></div><div class="col">${titlebar({ ...c }, false)}<div class="right fade"><h3>${L(...c.h)}</h3><p class="sub">${L(...c.sub)}</p>${body}${bottom}</div></div></div>`;
};
const varC = (c) => {
	const hero = c.hero;
	const pct = c.progress ? s.pct : 100;
	const inline = c.body && !c.sheet ? `<div class="slot fade">${c.body}</div>` : "";
	const sheet = c.sheet ? `<div class="sheet"><h4>${L("Options", "选项")}</h4>${c.body}</div>` : "";
	const stage = `<div class="stage-c ${hero ? "" : "plain"} ${c.sheet ? "with-sheet" : ""}">${hero ? doodle(176, pct) : ""}<h3>${L(...c.h)}</h3><p class="sub">${L(...c.sub)}</p>${inline}${sheet}</div>`;
	const foot = c.progress ? `<div class="foot p">${progress(c)}</div>` : `<div class="foot">${toggle(c)}${c.left ?? ""}<div class="acts">${c.actions ?? ""}</div></div>`;
	return `<div class="win C" style="width:680px;height:460px">${titlebar(c)}${stage}${foot}</div>`;
};
const updateWin = (c) => `<div class="win" style="width:400px">${s.st === "update" ? `<div class="up"><div class="top">${logo(32)}<div><b>${L("Updating FrameEver…", "正在更新帧刻…")}</b><span class="small tnum">0.9.2 → 1.0.0</span></div><span class="small tnum">${s.pct}%</span></div><div class="progress"><i style="width:${s.pct}%"></i></div></div>` : `<div class="up"><div class="top">${logo(32)}<div><b>${L(...c.h)}</b><span class="small">${L(...c.sub)}</span></div></div>${c.body ?? ""}<div class="acts" style="justify-content:flex-end;align-items:center">${c.left}<span style="flex:1"></span>${c.actions}</div></div>`}</div>`;
const nsisWin = () => `<div class="nsis"><div class="cap"><span style="display:flex;gap:6px;align-items:center">${logo(16)}FrameEver ${VERSION}</span><span style="width:46px;text-align:center">✕</span></div><div class="pane"><div class="side">${logo(48)}<b>FrameEver</b><i></i></div><div class="txt"><h5>${L("Welcome to FrameEver Setup", "欢迎使用帧刻安装程序")}</h5><p>${L("Setup will guide you through the installation of FrameEver.", "安装程序将引导你完成帧刻的安装。")}</p><p>${L("Click Next to continue.", "单击“下一步”继续。")}</p></div></div><div class="btns"><span>${L("Next >", "下一步(N) >")}</span><span>${L("Cancel", "取消(C)")}</span></div></div>`;

const narrate = {
	welcome: ["Enter installs with every default. Options sits one click away. There is no licence page and no language prompt.", "按 Enter 就用默认值安装。选项在一次点击之内。没有许可页，也不问语言。"],
	options: ["Checkboxes, not switches: nothing happens until you press Install. Space needed updates when the location changes.", "用复选框而不是开关：按安装之前什么都不会发生。改位置时重新计算所需空间。"],
	installing: ["Real percentage, one bar that never moves back. Close is off while files copy. The taskbar button shows the same progress. At 100% FrameEver opens and this window goes.", "真实百分比，一根进度条，不回退。复制文件时不能关闭。任务栏按钮显示同样的进度。到 100% 打开帧刻，窗口随即消失。"],
	running: ["New: Restart Manager finds what holds the files, including the CLI. Close FrameEver quits it normally.", "新增：用 Restart Manager 找出占用文件的程序，包括 CLI。“关闭帧刻”是正常退出。"],
	recording: ["New: the installer never closes a recording. The in-app updater already waits for one to finish.", "新增：安装器绝不关闭录制中的应用。应用内更新本来就会等录制结束。"],
	space: ["New: checked before anything is copied, so a full disk never leaves half an install.", "新增：复制之前就检查，磁盘满了不会留下装到一半的文件。"],
	failed: ["What happened and what to do, raw text behind Details, Export logs as the way out.", "说清发生了什么、接下来怎么做，原始信息收在详情里，出路是导出日志。"],
	older: ["Replaces NSIS's “uninstall before installing” page. Updating is the obvious action, with the old location and shortcuts kept.", "替代 NSIS 的“安装前先卸载”页。更新是明显的主操作，沿用原来的位置和快捷方式。"],
	same: ["The reinstall or uninstall choice from #31, as one action plus one link.", "#31 定下的重新安装或卸载，做成一个主操作加一个链接。"],
	newer: ["Keeping the newer version is the safe default. Downgrading stays possible.", "保留新版本是安全的默认值。降级仍然可以做。"],
	update: ["Update mode, the same in all three variants: no title bar, progress only, doesn't take focus. FrameEver relaunches when it reaches 100%.", "更新模式，三个方案都一样：没有标题栏，只有进度，不抢焦点。到 100% 重新打开帧刻。"],
	updatefail: ["The small window stays until you act, so a failed update doesn't vanish silently.", "小窗口会留着等你处理，更新失败不会悄悄消失。"],
	uninstall: ["The storage folder line comes first and shows the real path. No doodle here: hand-drawn never appears on delete dialogs.", "存储文件夹那一行放在最前，显示真实路径。这里没有手绘：删除对话框不用手绘。"],
	uninstalling: ["Same bar as installing. The exe runs from a temp copy so it can remove itself.", "和安装同样的进度条。程序从临时副本运行，才能删除自己。"],
	removed: ["Nothing else on screen confirms it, so this page does, and offers the folder that was kept.", "屏幕上没有别的东西能确认结果，所以由这一页确认，并提供保留下来的文件夹。"],
	fallback: ["Only when GPUI can't start. Light only, system controls. The sidebar drops its blue tint for the near-achromatic surface, and nothing else changes.", "只在 GPUI 启动不了时出现。只有浅色，系统控件。侧栏去掉偏蓝的底色，换成近乎无彩的灰，其他不变。"],
};

const compare = [
	[["Size", "尺寸"], ["440 × 400, snaps to 440 × 600 with options", "440 × 400，展开选项时跳到 440 × 600"], ["680 × 400", "680 × 400"], ["680 × 460", "680 × 460"]],
	[["First look", "第一眼"], ["Icon, name and one big button", "图标、名字和一个大按钮"], ["A Klein block with the brand", "一块带品牌的 Klein 蓝"], ["The welcome page's screen, drawn as files copy", "欢迎页的屏幕，随文件复制画出来"]],
	[["Options", "选项"], ["Open in place", "就地展开"], ["Open in the right column", "在右栏展开"], ["Sheet over the stage", "盖在舞台上的面板"]],
	[["Progress", "进度"], ["Where the button was", "出现在按钮的位置"], ["Bottom of the right column", "右栏底部"], ["Footer, with the doodle tracking it", "底栏，手绘同步进展"]],
	[["Design system", "设计系统"], ["Fits as it stands", "完全符合现有规则"], ["Needs a new large-Klein entry", "要在 Klein 蓝白名单加一项大面积用法"], ["Needs a new hand-drawn place, and drawing tied to progress", "要加一处手绘位置，还要让描线跟随进度"]],
	[["Fallback gap", "和回退的差距"], ["Largest: different layout", "最大：布局不同"], ["Smallest: NSIS has the same sidebar", "最小：NSIS 也有同样的侧栏"], ["Large", "较大"]],
	[["Next to the app", "和应用的关系"], ["Like the compact main window", "像紧凑主窗口"], ["Unlike any app window", "不像任何应用窗口"], ["Leads into the welcome page", "接到欢迎页"]],
];
const shared = [
	[["Language and theme", "语言和主题"], ["Windows display language and dark or light mode, read at start. Written to settings as the first language.", "启动时读取 Windows 显示语言和深浅色，并写入设置作为首次界面语言。"]],
	[["Keyboard", "键盘"], ["Focus starts on the primary button. Enter acts, Esc cancels or goes back, Tab walks the controls, Space toggles.", "焦点从主按钮开始。Enter 执行，Esc 取消或返回，Tab 移动，空格切换。"]],
	[["Motion", "动效"], ["Page changes fade (duration-mid). The window snaps, only its content moves. Windows's “Show animations” setting off means no fades.", "切换页面时淡入（duration-mid）。窗口直接跳到新尺寸，只有内容在动。Windows 关闭“显示动画”时不淡入。"]],
	[["Closing", "关闭"], ["Close works everywhere except while files copy. NSIS silent mode has no clean stop, so there is no Cancel there.", "除了复制文件时都能关闭。NSIS 静默模式没有干净的中止方式，所以那时没有取消。"]],
	[["Taskbar", "任务栏"], ["The taskbar button shows progress in every mode, and turns red on failure.", "每种模式都在任务栏按钮上显示进度，失败时变红。"]],
	[["Command line", "命令行"], ["The download keeps /S and /D= for scripts and winget. With /S no window appears and FrameEver doesn't open.", "下载的文件保留 /S 和 /D=，给脚本和 winget 用。用 /S 时没有窗口，也不打开帧刻。"]],
	[["PATH", "PATH"], ["The option uses the same cli-install crate as Settings, so Settings shows the true state afterwards.", "这个选项和设置页共用 cli-install crate，之后设置页显示的状态是准确的。"]],
	[["Update fallback", "更新时的回退"], ["If GPUI can't start in update mode, run NSIS with /P /R /UPDATE as today. Uninstall falls back to NSIS's own pages.", "更新模式下 GPUI 启动不了时，按现在的方式用 /P /R /UPDATE 运行 NSIS。卸载回退到 NSIS 自己的页面。"]],
];
const refs = [
	["Windows Installer, WiX, Inno Setup wizards", "Windows Installer、WiX、Inno Setup 向导", "Page-by-page wizards are what we are replacing; WixUI's minimal dialog shows the one-page shape is established", "分页向导是我们要替换的；WixUI 的单对话框说明单页形态早已成熟", "https://firegiant.com/wix/tutorial/user-interface/ui-wizardry"],
	["Fluent 2 progress bar", "Fluent 2 进度条", "Determinate where possible, one bar for related steps, -ing label with …, no time estimates, the error replaces the status text", "尽量用确定进度，相关步骤共用一根，用“正在…”，不给时间估计，错误替换状态文字", "https://fluent2.microsoft.design/components/web/react/core/progressbar/usage"],
	["Windows Restart Manager", "Windows Restart Manager", "Custom installers can use it to find and close what holds a file, instead of asking for a reboot", "自定义安装器可以用它找到并关闭占用文件的程序，不用要求重启", "https://learn.microsoft.com/en-us/windows/win32/rstmgr/about-restart-manager"],
	["Tauri's NSIS template", "Tauri 的 NSIS 模板", "The full list of pages and existing-install cases we have to keep or replace", "我们要保留或替换的页面和已安装情况的完整清单", "https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi"],
	["Upstream Cap at f7d351611", "上游 Cap，f7d351611", "No installer design change since our merge; the installer is still stock NSIS. Its Settings CLI install is already in our cli-install crate", "我们合并之后安装器设计没有变化，仍是默认 NSIS。设置页安装 CLI 的功能已在我们的 cli-install crate 里", "https://github.com/CapSoftware/Cap"],
	["WeChat, Feishu, JianYing", "微信、飞书、剪映", "From memory, not checked: one window, one button, a collapsed custom install. Searches found no source, so A's shape rests on #31, not on these", "凭记忆，没有核实：一个窗口、一个按钮、折叠的自定义安装。搜索没找到来源，所以方案 A 的形态依据是 #31，不是这些产品", ""],
];

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
const go = (st) => {
	s.st = st;
	s.details = false;
	tick();
	render();
};
const draw = () => {
	const desk = document.getElementById("desk");
	if (!desk) return;
	let inner;
	if (s.st === "fallback") inner = nsisWin();
	else {
		const c = model();
		if (s.st === "update" || s.st === "updatefail") inner = updateWin(c ?? {});
		else inner = { A: varA, B: varB, C: varC }[s.variant](c);
	}
	desk.innerHTML = inner;
	const win = desk.firstElementChild;
	win.style.zoom = "";
	const room = (desk.clientWidth - 24) / win.offsetWidth;
	if (room < 1) win.style.zoom = room;
	document.getElementById("line").innerHTML = L(...narrate[s.st]);
};
const render = () => {
	const root = document.getElementById("root");
	if (!document.getElementById("desk")) {
		const table = (id) => `<div class="tbl" id="${id}"></div>`;
		root.innerHTML = `<section><h2>${L("Installer windows", "安装器窗口")}</h2><p class="lead">${L("Three layouts for the same pages. Press the arrows at the bottom to switch; the buttons in the window really move between pages. Every state from the ticket is in the row below, in both themes and languages from the top bar.", "同一组页面的三种布局。按底部箭头切换；窗口里的按钮真的能走流程。工单要求的所有状态都在下面这几行，主题和语言在顶栏切换。")}</p><div class="flow" id="flow"></div><div class="desk" id="desk"></div><p class="state-line" id="line"></p></section>
		<section><h2>${L("Compare", "对比")}</h2>${table("t-compare")}</section>
		<section><h2>${L("Same in every variant", "每个方案都一样")}</h2>${table("t-shared")}</section>
		<section><h2>${L("References", "参考")}</h2><p class="lead">${L("Current sources on installer design are thin. The searches returned mostly vendor pages for installer tools, and nothing on how Figma, Discord or Linear look on Windows.", "安装器设计的最新资料很少。搜索到的大多是打包工具厂商的页面，没有 Figma、Discord、Linear 在 Windows 上安装界面的资料。")}</p>${table("t-refs")}</section>`;
	}
	document.getElementById("flow").innerHTML = GROUPS.map(([g, items]) => `<div class="row"><span>${L(...g)}</span>${items.map(([k, en, zh]) => `<button class="chip ${k === s.st ? "on" : ""}" data-go="${k}">${L(en, zh)}</button>`).join("")}</div>`).join("");
	document.getElementById("t-compare").className = "tbl wide";
	document.getElementById("t-compare").innerHTML = `<table><thead><tr><th></th>${Object.entries(VARIANTS).map(([k, n]) => `<th>${k} ${L(...n)}</th>`).join("")}</tr></thead><tbody>${compare.map(([k, a, b, c]) => `<tr><th>${L(...k)}</th><td>${L(...a)}</td><td>${L(...b)}</td><td>${L(...c)}</td></tr>`).join("")}</tbody></table>`;
	document.getElementById("t-shared").innerHTML = `<table><tbody>${shared.map(([k, v]) => `<tr><th>${L(...k)}</th><td>${L(...v)}</td></tr>`).join("")}</tbody></table>`;
	document.getElementById("t-refs").innerHTML = `<table><tbody>${refs.map(([a, b, c, d, url]) => `<tr><th>${url ? `<a href="${url}" target="_blank" rel="noreferrer">${L(a, b)}</a>` : L(a, b)}</th><td>${L(c, d)}</td></tr>`).join("")}</tbody></table>`;
	document.getElementById("switcher").innerHTML = `<button data-move="-1" aria-label="Previous">←</button><span>${s.variant} · ${pick(...VARIANTS[s.variant])}</span><button data-move="1" aria-label="Next">→</button>`;
	draw();
};
const move = (by) => {
	const list = Object.keys(VARIANTS);
	s.variant = list[(list.indexOf(s.variant) + by + list.length) % list.length];
	try {
		history.replaceState(null, "", `?variant=${s.variant}${location.hash}`);
	} catch {}
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
	const moveEl = event.target.closest("[data-move]");
	if (moveEl) {
		move(Number(moveEl.dataset.move));
		return;
	}
	const goEl = event.target.closest("[data-go]");
	if (goEl) go(goEl.dataset.go);
});
document.addEventListener("keydown", (event) => {
	if (event.target.closest?.("input")) return;
	if (event.key === "ArrowLeft") move(-1);
	if (event.key === "ArrowRight") move(1);
});
window.addEventListener("resize", draw);

shell({ here: "installer", depth: 1 });
onChange(render);
render();
