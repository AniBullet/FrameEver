const { L, icon, pick, shell, onChange } = FE;

const VARIANTS = [
	["A", "Full bar", "完整条", "Grows from the bar chosen in Visual direction: time first, actions next, Stop at the far end. Everything is one click away, as in Screen Studio and Screenity. Delete sits behind a divider.", "从视觉方向里选定的控制条长出来：时间在前，操作居中，停止在最后。全部一次点到，和 Screen Studio、Screenity 一样。删除隔在分隔线后面。"],
	["B", "Essentials", "精简条", "Only the time, microphone, Pause and Stop stay out, so it covers the least. Restart, Delete and Draw move into the ⋯ popover, one click further.", "只留时间、麦克风、暂停和停止，挡得最少。重新开始、删除和画笔收进 ⋯，多点一次。"],
	["C", "Docked tab", "贴边胶囊", "Hangs from the top edge showing only the time. Tap it to open the full bar; it folds away after two seconds and stays open while paused. Stop is one click further.", "挂在屏幕顶边，平时只露出时间。点一下展开完整条，移开两秒后收起，暂停时保持展开。停止要多点一次。"],
];

const MICS = ["Default · Realtek Audio", "USB Microphone", "AirPods Pro"];
const CAMS = ["Logitech C920", "OBS Virtual Camera"];

const live = {
	variant: "A", mode: "studio", sec: 83, paused: false, muted: false, mic: 1, startedMic: true, cam: 0, startedCam: true, preview: true,
	pop: null, open: null, ask: null, hidden: false, draw: false, colour: 0, tool: "pencil", tabOpen: false, issue: null, issueHidden: false,
};
const opt = { place: "M1", mode: "studio", open: false, quality: "balanced", res: "1080", fps: "60", countdown: "3", cursor: true, keys: false };

const fmt = (sec) => {
	const h = Math.floor(sec / 3600);
	const m = Math.floor((sec % 3600) / 60);
	const s = String(sec % 60).padStart(2, "0");
	return h ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
};
const seg = (name, value, options) => `<div class="seg" data-seg="${name}">${options.map(([v, en, zh]) => `<button class="${value === v ? "on" : ""}" data-v="${v}">${zh === undefined ? en : L(en, zh)}</button>`).join("")}</div>`;
const btn = (a, name, title, cls = "", extra = "") => `<button class="rb-btn ${cls}" data-a="${a}" title="${title}" aria-label="${title}" ${extra}>${name}</button>`;
const keys = (...list) => list.map((key) => `<span class="kbd">${key}</span>`).join(" ");
const grip = () => `<button class="rb-grip" tabindex="-1" aria-hidden="true" title="${pick("Drag to move", "拖动以移动")}">${icon("grip-vertical", "sm")}</button>`;
const sep = () => `<span class="rb-sep"></span>`;

const mic = (s) => {
	if (s.micState === "lost") return btn("pop", icon("mic-off"), pick("Microphone disconnected", "麦克风已断开"), "alert");
	if (!s.startedMic) return btn("noop", icon("mic-off"), pick("No microphone", "无麦克风"), "", "disabled");
	if (s.muted) return btn("mute", icon("mic-off"), pick("Unmute", "取消静音"), "muted");
	const meter = `<span class="rb-meter"><b></b></span>`;
	if (s.mode === "instant") return btn("mute", icon("mic") + meter, pick("Mute", "静音"));
	return `<span class="rb-btn" style="cursor:default" title="${pick("Microphone", "麦克风")}">${icon("mic")}${meter}</span>`;
};

const timeLabel = (s) => {
	const t = fmt(s.sec);
	if (s.phase === "countdown") return `<span class="rb-time quiet tnum">${L(`Starts in ${s.count}`, `${s.count} 秒后开始`)}</span>`;
	if (s.phase === "starting") return `<span class="rb-time quiet">${icon("rotate-ccw", "sm rb-spin")}${L("Starting…", "正在开始…")}</span>`;
	if (s.phase === "saving") return `<span class="rb-time quiet">${icon("rotate-ccw", "sm rb-spin")}${L("Saving…", "正在保存…")}</span>`;
	if (s.phase === "pausing") return `<span class="rb-time quiet tnum">${L("Pausing…", "正在暂停…")}</span>`;
	if (s.phase === "resuming") return `<span class="rb-time quiet tnum">${L("Resuming…", "正在继续…")}</span>`;
	if (s.paused) return `<span class="rb-time warn tnum"><i class="rb-dot"></i>${L("Paused", "已暂停")} <span data-time>${t}</span></span>`;
	return `<span class="rb-time tnum"><i class="rb-dot"></i><span data-time>${t}</span></span>`;
};

const askCard = (s) => {
	const [title, body, verb] = s.ask === "restart"
		? [L("Restart this recording?", "重新开始录制？"), L("What you've recorded so far is discarded. This can't be undone.", "已录内容会被丢弃，无法撤销。"), L("Restart", "重新开始")]
		: [L("Delete this recording?", "删除这段录制？"), L("It won't be saved. This can't be undone.", "这段录制不会保存，无法撤销。"), L("Delete", "删除")];
	return `<div class="pop ask-card" role="alertdialog"><b>${title}</b><p>${body}</p><div class="ask-actions"><button class="btn" data-a="ask-no">${L("Cancel", "取消")}</button><button class="btn danger" data-a="ask-yes">${verb}</button></div></div>`;
};

const busy = (s) => ["countdown", "starting", "saving", "pausing", "resuming"].includes(s.phase);
const pauseBtn = (s) => s.paused
	? btn("pause", icon("play", "solid"), pick("Resume  Ctrl Shift P", "继续  Ctrl Shift P"), "resume", busy(s) ? "disabled" : "")
	: btn("pause", icon("pause", "solid"), pick("Pause  Ctrl Shift P", "暂停  Ctrl Shift P"), "", busy(s) ? "disabled" : "");
const stopBtn = (s) => `<button class="rb-stop" data-a="stop" title="${pick("Stop  Ctrl Shift S", "停止  Ctrl Shift S")}" aria-label="${pick("Stop", "停止")}" ${s.phase === "saving" ? "disabled" : ""}>${icon("square", "solid")}</button>`;
const warnBtn = (s) => (s.issue ? btn("issue", icon("triangle-alert"), pick("Recording issue", "录制问题"), "alert") : "");

const endBtn = (s) => (s.phase === "countdown" ? btn("stop", icon("x"), pick("Cancel", "取消")) : stopBtn(s));
const barA = (s, cls = "") => `<div class="rb ${s.paused ? "paused" : ""} ${cls}" role="toolbar" aria-label="${pick("Recording controls", "录制控制条")}">
	${grip()}${timeLabel(s)}${mic(s)}${pauseBtn(s)}${btn("ask-restart", icon("rotate-ccw"), pick("Restart", "重新开始"), "", busy(s) ? "disabled" : "")}${warnBtn(s)}${sep()}
	${btn("ask-delete", icon("trash"), pick("Delete", "删除"), "", busy(s) ? "disabled" : "")}${btn("draw", icon("pencil"), pick("Draw on screen", "屏幕画笔"), s.draw ? "on" : "", busy(s) ? "disabled" : "")}${btn("pop", icon("sliders-horizontal"), pick("Recording options", "录制选项"), s.pop ? "on" : "")}${sep()}${endBtn(s)}</div>`;

const barB = (s) => `<div class="rb ${s.paused ? "paused" : ""}" role="toolbar" aria-label="${pick("Recording controls", "录制控制条")}">
	${grip()}${timeLabel(s)}${mic(s)}${warnBtn(s)}${pauseBtn(s)}${btn("pop", icon("more-horizontal"), pick("More", "更多"), s.pop ? "on" : "")}${sep()}${endBtn(s)}</div>`;

const barC = (s) => (s.tabOpen || s.paused || s.pop || s.ask || s.issue
	? barA(s, "edge")
	: `<button class="rb-tab tnum" data-a="tab" aria-label="${pick("Open recording controls", "打开录制控制条")}"><i class="rb-dot"></i><span data-time>${fmt(s.sec)}</span></button>`);

const barFor = (s) => ({ A: barA, B: barB, C: barC })[s.variant](s);

const device = (id, ic, en, zh, value, off, list, s, chosen) => {
	const locked = off;
	const open = s.open === id && !locked;
	const rows = open ? list.map((name, i) => `<button class="pop-row sub" data-a="pick-${id}" data-v="${i}"><span class="grow">${name}</span>${i === chosen ? icon("check", "sm") : ""}</button>`).join("") : "";
	return `<button class="pop-row" data-a="open-${id}" ${locked ? "disabled" : ""}>${icon(ic, "sm")}<span class="grow">${L(en, zh)}</span><em>${locked ? L("Not in this recording", "本次未使用") : value}</em>${locked ? "" : icon(open ? "chevron-up" : "chevron-down", "sm")}</button>${rows}`;
};

const popover = (s) => {
	const mode = s.mode === "studio" ? L("Studio", "工作室") : L("Instant", "即时");
	const extra = s.variant === "B" ? `<button class="pop-row" data-a="draw">${icon("pencil", "sm")}<span class="grow">${L("Draw on screen", "屏幕画笔")}</span></button>
		<button class="pop-row" data-a="ask-restart">${icon("rotate-ccw", "sm")}<span class="grow">${L("Restart", "重新开始")}</span></button>
		<button class="pop-row danger" data-a="ask-delete">${icon("trash", "sm")}<span class="grow">${L("Delete", "删除")}</span></button><hr>` : "";
	return `<div class="pop" role="dialog" aria-label="${pick("Recording options", "录制选项")}">
		<div class="pop-head tnum">${icon(s.mode === "studio" ? "clapperboard" : "zap", "sm")}<span>${mode} · ${L("Display", "显示器")} · 2560 × 1440 · ${L("60 FPS", "60 帧/秒")}</span></div><hr>
		${extra}${device("mic", "mic", "Microphone", "麦克风", MICS[s.mic], !s.startedMic, MICS, s, s.mic)}
		${device("cam", "video", "Camera", "摄像头", CAMS[s.cam], !s.startedCam, CAMS, s, s.cam)}
		<button class="pop-row" data-a="preview" ${s.startedCam ? "" : "disabled"}>${icon("maximize", "sm")}<span class="grow">${L("Show camera preview", "显示摄像头预览")}</span><span class="switch ${s.preview && s.startedCam ? "on" : ""}"></span></button><hr>
		<button class="pop-row" data-a="hide">${icon("eye-off", "sm")}<span class="grow">${L("Hide controls", "隐藏控制条")}</span>${keys("Ctrl", "Shift", "H")}</button>
	</div>`;
};

const drawStrip = (s) => `<div class="draw-strip" role="toolbar" aria-label="${pick("Drawing tools", "画笔工具")}">
	${[["pencil", "Pen", "画笔"], ["highlighter", "Highlighter", "荧光笔"], ["arrow-up-right", "Arrow", "箭头"]].map(([n, en, zh]) => btn("tool", icon(n, "sm"), pick(en, zh), s.tool === n ? "on" : "", `data-v="${n}"`)).join("")}
	${sep()}${["var(--primary)", "var(--danger)", "var(--warning)"].map((c, i) => `<button class="swatch ${s.colour === i ? "on" : ""}" style="background:${c}" data-a="colour" data-v="${i}" aria-label="${pick("Colour", "颜色")} ${i + 1}"></button>`).join("")}
	${sep()}${btn("undo-ink", icon("undo", "sm"), pick("Undo", "撤销"))}${btn("clear-ink", icon("eraser", "sm"), pick("Clear", "清除"))}</div>`;

const issues = {
	mic: ["warn", "Microphone disconnected. Recording silence until it's back.", "麦克风已断开。在它恢复之前录入的是静音。"],
	cam: ["warn", "Camera disconnected. Recording continues without it.", "摄像头已断开。录制继续，只是没有摄像头画面。"],
	storage: ["warn", "Low storage. FrameEver will stop soon to save your recording.", "存储空间不足。帧刻即将停止，以保存你的录制。"],
	pause: ["bad", "Couldn't confirm the capture state. Stop to save what you have.", "无法确认录制状态。点停止保存已录内容。"],
	disk: ["bad", "Couldn't resume. There isn't enough disk space. Free up space and try again, or stop to save.", "无法继续录制。磁盘空间不足，清理后再试，或点停止保存。"],
};
const issueCard = (key, dismiss = true) => {
	const [kind, en, zh] = issues[key];
	return `<div class="issue ${kind === "bad" ? "bad" : ""}" role="status">${icon("triangle-alert", "sm")}<span>${L(en, zh)}</span>${dismiss ? `<button data-a="dismiss" aria-label="${pick("Dismiss", "关闭")}">${icon("x", "sm")}</button>` : ""}</div>`;
};

const wall = `<svg class="wall" viewBox="0 0 960 540" preserveAspectRatio="xMidYMid slice" aria-hidden="true"><rect width="960" height="540" fill="#3A3A3A"/><rect x="60" y="40" width="840" height="460" rx="10" fill="#EDEDED"/><rect x="60" y="40" width="840" height="36" rx="10" fill="#555555"/><rect x="60" y="76" width="190" height="424" fill="#DEDEDE"/><rect x="84" y="104" width="130" height="10" rx="3" fill="#A8A8A8"/><rect x="84" y="128" width="100" height="10" rx="3" fill="#C4C4C4"/><rect x="84" y="152" width="120" height="10" rx="3" fill="#C4C4C4"/><rect x="284" y="104" width="280" height="18" rx="4" fill="#555555"/><rect x="284" y="140" width="560" height="9" rx="3" fill="#A8A8A8"/><rect x="284" y="160" width="510" height="9" rx="3" fill="#A8A8A8"/><rect x="284" y="196" width="260" height="150" rx="6" fill="#CFCFCF"/><rect x="564" y="196" width="280" height="150" rx="6" fill="#BDBDBD"/></svg>`;

const screen = document.getElementById("screen");
let ink = [];

const renderDock = () => {
	const s = { ...live, micState: live.issue === "mic" ? "lost" : "" };
	const host = document.getElementById("dock");
	if (!host) return;
	const top = s.variant === "C";
	host.className = `dock ${top ? "top" : "bottom"}`;
	if (s.hidden) {
		host.innerHTML = "";
		document.getElementById("hidden-chip").hidden = false;
		return;
	}
	document.getElementById("hidden-chip").hidden = true;
	host.innerHTML = `${s.pop ? popover(s) : ""}${s.ask ? askCard(s) : ""}${s.issue && !s.issueHidden ? issueCard(s.issue) : ""}${s.draw ? drawStrip(s) : ""}${barFor(s)}`;
	screen.classList.toggle("drawing", s.draw);
	screen.querySelector(".ink").style.pointerEvents = s.draw ? "auto" : "none";
};

const renderInk = () => {
	const colours = ["var(--primary)", "var(--danger)", "var(--warning)"];
	screen.querySelector(".ink").innerHTML = ink.map((line) => `<polyline points="${line.points.join(" ")}" fill="none" stroke="${colours[line.colour]}" stroke-width="${line.tool === "highlighter" ? 14 : 3}" stroke-opacity="${line.tool === "highlighter" ? 0.4 : 1}" stroke-linecap="round" stroke-linejoin="round"/>`).join("");
};

const renderLive = () => {
	document.getElementById("live-tools").innerHTML = `${seg("mode", live.mode, [["studio", "Studio", "工作室"], ["instant", "Instant", "即时"]])}
		${seg("mic-state", live.startedMic ? "on" : "off", [["on", "With microphone", "有麦克风"], ["off", "No microphone", "无麦克风"]])}
		${seg("issue", live.issue ?? "none", [["none", "No issue", "无问题"], ["mic", "Mic lost", "麦克风断开"], ["storage", "Low storage", "空间不足"]])}`;
	const v = VARIANTS.find(([id]) => id === live.variant);
	document.getElementById("variant-note").innerHTML = L(v[3], v[4]);
	document.getElementById("switcher").innerHTML = `<button data-sw="-1" aria-label="${pick("Previous variant", "上一个方案")}">${icon("chevron-left", "sm")}</button><span class="name">${v[0]} · ${L(v[1], v[2])}</span><button data-sw="1" aria-label="${pick("Next variant", "下一个方案")}">${icon("chevron-right", "sm")}</button>`;
	renderDock();
};

const renderStates = () => {
	const base = { variant: live.variant, mode: live.mode, sec: 83, paused: false, muted: false, startedMic: true, startedCam: true, mic: 1, cam: 0, pop: null, tabOpen: true, ask: null };
	const cards = [
		["Countdown", "倒计时", { phase: "countdown", count: 3 }],
		["Starting", "正在开始", { phase: "starting" }],
		["Recording", "录制中", {}],
		["Pausing", "正在暂停", { phase: "pausing" }],
		["Paused", "已暂停", { paused: true }],
		["Over an hour", "超过一小时", { sec: 3753 }],
		["Instant, muted", "即时，已静音", { mode: "instant", muted: true }],
		["No microphone", "无麦克风", { startedMic: false }],
		["Saving", "正在保存", { phase: "saving" }],
		["Confirm restart", "确认重新开始", { ask: "restart" }],
		["Microphone lost", "麦克风断开", { issue: "mic", micState: "lost" }],
		["Low storage", "空间不足", { issue: "storage" }],
		["Pause failed", "暂停失败", { issue: "pause" }],
		["Can't resume", "无法继续", { issue: "disk", paused: true }],
	];
	document.getElementById("state-grid").innerHTML = cards.map(([en, zh, patch]) => {
		const s = { ...base, ...patch };
		return `<div class="state-card"><p class="label">${L(en, zh)}</p><div class="state-stack">${s.ask ? askCard(s) : ""}${s.issue ? issueCard(s.issue, false) : ""}${barFor(s)}</div></div>`;
	}).join("");
};

const summary = () => {
	const q = { balanced: ["Balanced", "均衡"], small: ["Smaller", "更小"], max: ["Maximum", "最高"] }[opt.quality];
	const first = opt.mode === "studio" ? L(...q) : opt.res === "2160" ? "4K" : `${opt.res}p`;
	const cd = opt.countdown === "0" ? "" : ` · ${opt.countdown} ${L("s", "秒")}`;
	return `<span class="tnum">${first} · ${L(`${opt.fps} FPS`, `${opt.fps} 帧/秒`)}${cd}</span>`;
};

const optPop = (bottom) => {
	const studio = opt.mode === "studio";
	return `<div class="opt-pop" style="bottom:${bottom}px" role="dialog" aria-label="${pick("Recording options", "录制选项")}">
		<div class="opt-row"><span class="grow">${studio ? L("Quality", "画质") : L("Resolution", "分辨率")}</span>${studio ? seg("quality", opt.quality, [["small", "Smaller", "更小"], ["balanced", "Balanced", "均衡"], ["max", "Maximum", "最高"]]) : seg("res", opt.res, [["720", "720p"], ["1080", "1080p"], ["1440", "1440p"], ["2160", "4K"]])}</div>
		<div class="opt-row"><span class="grow">${L("Maximum frame rate", "最高帧率")}</span>${seg("fps", opt.fps, [["24", "24"], ["30", "30"], ["60", "60"], ["120", "120"]])}</div>
		<div class="opt-row"><span class="grow">${L("Countdown", "倒计时")}</span>${seg("countdown", opt.countdown, [["0", "Off", "关"], ["3", "3"], ["5", "5"], ["10", "10"]])}</div>
		${studio ? `<div class="opt-row"><span class="grow">${L("Separate cursor", "单独捕获光标")}</span><button class="switch ${opt.cursor ? "on" : ""}" data-o="cursor" aria-label="${pick("Separate cursor", "单独捕获光标")}"></button></div>
		<div class="opt-row"><span class="grow">${L("Key presses", "按键")}</span><button class="switch ${opt.keys ? "on" : ""}" data-o="keys" aria-label="${pick("Key presses", "按键")}"></button></div>` : ""}
		<hr style="border:0;border-top:1px solid var(--border);margin:4px 0">
		<button class="opt-more">${L("All recording settings", "全部录制设置")}${icon("chevron-right", "sm")}</button></div>`;
};

const renderOptions = () => {
	const { place, mode, open } = opt;
	document.getElementById("opt-tools").innerHTML = `${seg("place", place, [["M1", "M1 · In the footer", "M1 · 放在底栏"], ["M2", "M2 · Its own row", "M2 · 单独一行"], ["M3", "M3 · On the Start button", "M3 · 放在开始按钮上"]])}`;
	const shot = mode === "screenshot";
	const canOpt = !shot;
	const row = place === "M2" && canOpt ? `<button class="device" data-opt="toggle">${icon("sliders-horizontal")}<span class="grow">${L("Options", "录制选项")}</span><em>${summary()}</em>${icon(open ? "chevron-up" : "chevron-down", "sm")}</button>` : "";
	const start = shot ? L("Take screenshot", "截图") : L("Start recording", "开始录制");
	const startBtn = place === "M3" && canOpt
		? `<div class="split"><button class="btn primary tall">${icon("circle", "solid")}${start}</button><button class="btn primary tall" data-opt="toggle" aria-label="${pick("Recording options", "录制选项")}">${icon(open ? "chevron-down" : "chevron-up", "sm")}</button></div>`
		: `<button class="btn primary block tall">${icon("circle", "solid")}${start}</button>`;
	const middle = place === "M1" && canOpt
		? `<button class="sum ${open ? "on" : ""}" data-opt="toggle" title="${pick("Recording options", "录制选项")}">${icon("sliders-horizontal", "sm")}${summary()}</button>`
		: `<span class="grow tnum">${L("10 items", "10 项")}</span>`;
	const popBottom = place === "M1" ? 48 : place === "M3" ? 104 : 104;
	document.getElementById("opt-stage").innerHTML = `<div class="win" style="height:${place === "M2" && canOpt ? 430 : 386}px">
		<div class="titlebar"><span class="mark"></span><span>FrameEver</span></div>
		<div class="rec-main">
			<div class="seg block" data-seg="opt-mode">${[["studio", "clapperboard", "Studio", "工作室"], ["instant", "zap", "Instant", "即时"], ["screenshot", "image", "Screenshot", "截图"]].map(([id, ic, en, zh]) => `<button class="${mode === id ? "on" : ""}" data-v="${id}">${icon(ic, "sm")}${L(en, zh)}</button>`).join("")}</div>
			<div class="targets">${[["monitor", "Display", "显示器"], ["app-window", "Window", "窗口"], ["box-select", "Area", "区域"]].map(([ic, en, zh], i) => `<button class="${i === 0 ? "on" : ""}">${icon(ic, "lg")}${L(en, zh)}</button>`).join("")}</div>
			<div class="device">${icon("video")}<span class="grow">${L("Camera", "摄像头")}</span><em>${L("No camera", "无摄像头")}</em>${icon("chevron-down", "sm")}</div>
			<div class="device">${icon("mic")}<span class="grow">${L("Microphone", "麦克风")}</span><em>${L("Default", "默认")}</em>${icon("chevron-down", "sm")}</div>
			<div class="device">${icon("volume-2")}<span class="grow">${L("System audio", "系统声音")}</span><span class="switch on"></span></div>
			${row}${startBtn}
		</div>
		<div class="rec-foot" style="margin-top:auto"><button class="btn ghost icon" title="${pick("Settings", "设置")}">${icon("settings")}</button>${middle}<button class="btn ghost">${L("Library", "资源库")}${icon("chevron-right")}</button></div>
		${open && canOpt ? optPop(popBottom) : ""}
	</div>`;
};

const table = (id, head, rows) => {
	document.getElementById(id).innerHTML = `<table class="inv"><thead><tr>${head.map(([en, zh]) => `<th>${L(en, zh)}</th>`).join("")}</tr></thead><tbody>${rows.map((row) => `<tr>${row.map((cell, i) => `<td class="${i ? "dim" : ""}">${Array.isArray(cell) ? L(...cell) : cell}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
};

const copy = () => {
	const text = {
		"t-live": ["Recording bar", "录制控制条"],
		"l-live": ["Runs for real: the timer counts, and Pause, Options, Restart, Delete, Draw and Hide all work. Flip the three structures with the bar at the bottom of the page or the ← → keys. The bar never takes focus from what you're recording, and it is left out of the video.", "下面可以真的操作：计时在走，暂停、选项、重新开始、删除、画笔和隐藏都能点。用页面底部的切换条或 ← → 键换三种结构。控制条不会抢走被录内容的焦点，也不会出现在视频里。"],
		"t-states": ["Every state", "所有状态"],
		"l-states": ["These follow the variant and mode chosen above. Stop is the bar's one solid blue button. Red appears only on the recording dot, Delete and errors, amber only on paused and warnings. Studio shows a level only, because its microphone is an editable track and muted spans would bake in silence. Instant can mute. Everything else is the same in both modes.", "这些状态跟随上面选的方案和模式。停止是控制条上唯一的实色蓝按钮。红色只用在录制点、删除和错误上，琥珀色只用在暂停和警告上。工作室只显示音量，因为麦克风是可编辑的轨道，中途静音会把静音写死。即时可以静音。其余两种模式相同。"],
		"t-options": ["Recording options before you start", "开始前的录制选项"],
		"l-options": ["The compact window already holds source, camera, microphone and system audio, so the popover carries only what lives on the Recording settings page today, with the same names: quality, frame rate, countdown and what's captured. Open it with Tab and Enter, close it with Esc. M1 keeps the window at 386 px and always shows what's set; it replaces the item count, which the library shows anyway. Screenshot mode has no options.", "紧凑窗口已经有来源、摄像头、麦克风和系统声音，所以这个浮层只放设置“录制”页里的项目，名称完全一致：画质、帧率、倒计时和捕获内容。Tab 加 Enter 打开，Esc 关闭。M1 让窗口保持 386 px，并一直显示当前设置；它替换的是条目数，资源库里本来就有。截图模式没有这些选项。"],
		"t-keys": ["Keyboard", "键盘"],
		"l-keys": ["Global shortcuts work while you're in another app. The new focus shortcut reaches every other control: it moves focus to the bar, and Esc hands it back to the app you were recording. Inside the bar it follows the WAI-ARIA toolbar pattern. Ctrl+K stays out of the bar, as decided in Command palette; the palette in the main window still lists Stop, Pause and Restart.", "全局快捷键在其他应用里也能用。新增的聚焦快捷键覆盖其余所有按钮：它把焦点移到控制条，Esc 再还给被录的应用。控制条内部按 WAI-ARIA 工具栏模式。Ctrl+K 不进控制条，这是命令面板那张票定的；主窗口的命令面板仍列出停止、暂停和重新开始。"],
		"t-inv": ["Where everything went", "每个功能的去向"],
		"l-inv": ["Compared with our bar, upstream's latest GPUI bar and its last Tauri bar (removed in e4f4ca9bc), which had the recording settings menu the GPUI version left as a TODO.", "对照了我们的控制条、上游最新的 GPUI 控制条，以及上游最后一版 Tauri 控制条（在 e4f4ca9bc 删除）。Tauri 版有录制设置菜单，GPUI 版只留了 TODO。"],
		"t-add": ["Added from other products", "从其他产品加入"],
		"l-add": ["Open-source and popular recorders already have these. Your call on each; the first three are in the recommendation.", "这些开源和热门录屏软件已经有了。每项由你决定；前三项包含在推荐方案里。"],
		"t-ref": ["What this is built on", "这份设计的依据"],
		"l-ref": ["Our own decisions come first, then primary sources. The Zoom and Teams notes came from forum threads, so treat them as weaker.", "先看我们自己的决定，再看一手资料。Zoom 和 Teams 的内容来自论坛，可信度低一些。"],
	};
	for (const [id, pair] of Object.entries(text)) document.getElementById(id).innerHTML = L(...pair);
	table("keys", [["Action", "操作"], ["Key", "按键"], ["Where", "范围"]], [
		[["Stop", "停止"], keys("Ctrl", "Shift", "S"), ["Global, already exists", "全局，已有"]],
		[["Pause or resume", "暂停或继续"], keys("Ctrl", "Shift", "P"), ["Global, already exists", "全局，已有"]],
		[["Restart", "重新开始"], ["Not set", "未设置"], ["Global, already exists", "全局，已有"]],
		[["Focus recording controls", "聚焦录制控制条"], keys("Ctrl", "Shift", "Space"), ["Global, new. Registered only while recording", "全局，新增。只在录制时注册"]],
		[["Hide or show controls", "隐藏或显示控制条"], keys("Ctrl", "Shift", "H"), ["Global, new. Registered only while recording", "全局，新增。只在录制时注册"]],
		[["Mute", "静音"], ["Not set", "未设置"], ["Global, new. Instant only", "全局，新增。仅即时"]],
		[["Move between controls", "在按钮间移动"], `${keys("←")} ${keys("→")} ${keys("Home")} ${keys("End")}`, ["Inside the bar", "控制条内"]],
		[["Press the control", "按下按钮"], `${keys("Enter")} ${keys("Space")}`, ["Inside the bar", "控制条内"]],
		[["Close the popover, then leave the bar", "关闭浮层，再离开控制条"], keys("Esc"), ["Inside the bar", "控制条内"]],
	]);
	table("inv", [["Capability", "功能"], ["Today", "现状"], ["In this design", "这份设计"]], [
		[["Stop, elapsed time", "停止、计时"], ["Ours: one red button", "我们：同一个红色按钮"], ["Stop moves to the far end as the one solid blue button, so a click on the time can't end the take", "停止移到最后，是唯一的实色蓝按钮，点时间不会误停"]],
		[["Timer past an hour", "超过一小时的计时"], ["Ours shows 62:33; upstream fixed it", "我们显示 62:33；上游已修"], ["1:02:33, port upstream's format", "1:02:33，移植上游格式"]],
		[["Microphone level, Instant mute", "麦克风音量、即时静音"], ["Ours and upstream", "我们和上游都有"], ["Kept. The level fill is Klein blue, muted is amber", "保留。音量用克莱因蓝，静音用琥珀色"]],
		[["Pause, pending and unconfirmed", "暂停、等待中和未确认"], ["Ours", "我们有"], ["Kept; paused now shows the time too", "保留；暂停时同时显示时间"]],
		[["Restart, Delete", "重新开始、删除"], ["Windows dialog, which takes focus and can land in the video", "Windows 对话框，会抢焦点，还可能录进视频"], ["A card above the bar, using the confirmation pattern in the copy guide", "控制条上方的卡片，按文案规范的确认格式"]],
		[["Recording settings", "录制设置"], ["Ours and upstream GPUI: disabled gear", "我们和上游 GPUI：灰掉的齿轮"], ["Options popover", "录制选项浮层"]],
		[["Show camera preview", "显示摄像头预览"], ["Tauri had it, GPUI lost it", "Tauri 有，GPUI 丢了"], ["Switch in the popover", "浮层里的开关"]],
		[["Switch microphone or camera mid-take", "录制中换麦克风或摄像头"], ["Tauri had it, GPUI lost it", "Tauri 有，GPUI 丢了"], ["Popover. A device not used at the start stays off for this take", "浮层。开始时没用的设备本次保持关闭"]],
		[["Device disconnected warnings", "设备断开提示"], ["Tauri had them; not in our session code", "Tauri 有；我们的 session 代码里没有"], ["Warning button and card above the bar", "警告按钮和控制条上方的卡片"]],
		[["Error and low storage", "错误和空间不足"], ["Ours: banner above the bar", "我们：控制条上方的横幅"], ["Same card, now dismissible, and the warning button brings it back", "同样的卡片，现在可关闭，警告按钮可再打开"]],
		[["Window fits its issue", "窗口随提示调整大小"], ["Upstream fixed it; ours keeps a 150 px window that blocks clicks above the bar", "上游已修；我们的窗口固定 150 px，挡住控制条上方的点击"], ["Port upstream's fit", "移植上游做法"]],
		[["Countdown in the bar, cancel", "控制条内倒计时、取消"], ["Ours and Tauri", "我们和 Tauri 都有"], ["Kept here; the full-screen countdown belongs to Area picker and countdown", "这里保留；全屏倒计时归区域选择和倒计时那张票"]],
		[["Drag anywhere", "随意拖动"], ["Ours: handle on the right", "我们：手柄在右侧"], ["Handle on the left, position remembered", "手柄移到左侧，并记住位置"]],
		[["Quality link on the mode panel", "模式面板的画质链接"], ["Opens Settings", "打开设置"], ["Options popover, with a link to all settings", "录制选项浮层，内有全部设置入口"]],
		[["Maximum recording limit", "最长录制限制"], ["Tauri only", "仅 Tauri"], ["Dropped: an account limit", "去掉：这是账号限制"]],
	]);
	table("add", [["Feature", "功能"], ["Who has it", "谁有"], ["Cost", "代价"], ["Recommendation", "建议"]], [
		[["Hide controls", "隐藏控制条"], "Screen Studio, Screenity, Loom", ["Small", "小"], ["Add", "加入"]],
		[["Focus shortcut", "聚焦快捷键"], ["Needed for the keyboard floor", "键盘底线需要"], ["Small; the bar accepts focus only on this shortcut", "小；控制条只在这个快捷键下接受焦点"], ["Add", "加入"]],
		[["Remembered position", "记住位置"], "Screenity", ["Small", "小"], ["Add", "加入"]],
		[["Draw on screen", "屏幕画笔"], "Screenity, Loom", ["Medium: a new overlay window per display, and strokes are recorded into the video", "中：每个显示器一个新覆盖窗口，笔迹直接录进画面"], ["Your call", "你决定"]],
		[["Auto-stop after a set time", "定时自动停止"], "ShareX", ["Small; one row in the options popover", "小；选项浮层加一行"], ["Your call", "你决定"]],
		[["Click highlight in Instant", "即时录制的点击高亮"], "ShareX, Screenity", ["Medium: Instant has no editor, so it's baked in while capturing", "中：即时没有编辑器，要在采集时写入"], ["Your call", "你决定"]],
		[["Push to talk", "按住说话"], "Screenity", ["Small: reuses Instant mute", "小：复用即时静音"], ["Your call", "你决定"]],
		[["Chapter markers", "章节标记"], "OBS Studio", ["Medium: a new field in frameever.json and marks on the timeline", "中：frameever.json 加字段，时间轴加标记"], ["Your call", "你决定"]],
		[["Blur page elements", "模糊页面元素"], "Screenity", ["Reads web pages, so it can't work on a desktop recorder", "依赖网页结构，桌面录屏做不到"], ["Leave out", "不加"]],
	]);
	table("refs", [["Source", "来源"], ["What it decided here", "决定了什么"]], [
		['<a href="https://github.com/AniBullet/FrameEver/issues/28">Visual direction</a>', ["The chosen Klein Field bar is the starting point: time first, actions next, Stop last", "选定的克莱因场控制条是起点：时间在前，操作居中，停止在最后"]],
		['<a href="https://github.com/AniBullet/FrameEver/issues/54">Settings window design</a>, <code>docs/design/index.html</code>', ["Klein blue allowlist, neutral selection, border weights, no shadows, 20 px radius for recording controls and concentric inner radii", "蓝色白名单、中性选中、边框档位、无阴影、控制条 20 px 圆角和同心内圆角"]],
		['<a href="https://github.com/AniBullet/FrameEver/blob/main/docs/copy-style.md">Copy style</a>', ["Tooltips are a name plus a shortcut; confirmations are a question, a consequence, a verb and Cancel; errors start with Couldn't", "提示只写名称和快捷键；确认是问题、后果、动词和取消；错误用“无法”开头"]],
		['<a href="https://github.com/AniBullet/FrameEver/issues/53">Command palette</a>', ["Ctrl+K stays out of the bar", "Ctrl+K 不进控制条"]],
		['<a href="https://www.w3.org/WAI/ARIA/apg/patterns/toolbar/">WAI-ARIA toolbar pattern</a>', ["Arrow keys inside, one Tab stop", "内部用方向键，整条只占一个 Tab 位"]],
		["GPUI Kit", ["button, tooltip, kbd, popover, select, switch. The bar itself stays our own platform window, like the other overlays", "button、tooltip、kbd、popover、select、switch。控制条本身仍是我们自己的平台窗口，和其他覆盖层一样"]],
		[["Upstream Cap", "上游 Cap"], ["The settings menu and issue panel from its last Tauri bar; hours and window fit from its GPUI bar", "最后一版 Tauri 控制条的设置菜单和问题面板；GPUI 版的小时显示和窗口贴合"]],
		['<a href="https://www.screen.studio/guide/managing-recording-in-progress">Screen Studio</a>', ["Finish, Pause, Restart and Delete on one widget; hide it; left out of the video", "完成、暂停、重来、删除放在一个控件上；可隐藏；不录进视频"]],
		['<a href="https://screenity.io/help/recording/how-to-pause-or-resume-a-recording">Screenity</a>', ["Grip, remembered position, confirm before discarding, drawing tools", "抓手、记住位置、放弃前确认、画笔工具"]],
		['<a href="https://support.atlassian.com/loom/docs/hide-the-recording-controls/">Loom</a>', ["Keyboard use once the controls are hidden", "隐藏后用键盘操作"]],
		['<a href="https://github.com/ShareX/ShareX/blob/develop/ShareX/TaskSettings.cs">ShareX</a>', ["Fixed-length recording, mouse highlight", "固定时长录制、鼠标高亮"]],
		['<a href="https://community.zoom.com/t5/Zoom-Meetings/Keep-floating-meeting-controls-and-video-panel-from-hiding-part/td-p/37544">Zoom</a>, Microsoft Teams', ["Top-edge bar for variant C; complaints about bars that can't move or hide", "方案 C 的顶边控制条；用户抱怨不能移动或隐藏"]],
	]);
};

const renderAll = () => {
	renderLive();
	renderStates();
	renderOptions();
	copy();
};

const cycle = (step) => {
	const index = VARIANTS.findIndex(([id]) => id === live.variant);
	Object.assign(live, { variant: VARIANTS[(index + step + VARIANTS.length) % VARIANTS.length][0], pop: null, ask: null, tabOpen: false });
	renderLive();
	renderStates();
};

let folding;
const act = (a, value) => {
	if (a === "pause") live.paused = !live.paused;
	if (a === "stop") Object.assign(live, { sec: 0, paused: false, pop: null, draw: false });
	if (a === "mute") live.muted = !live.muted;
	if (a === "pop") Object.assign(live, { pop: !live.pop, open: null });
	if (a === "open-mic") live.open = live.open === "mic" ? null : "mic";
	if (a === "open-cam") live.open = live.open === "cam" ? null : "cam";
	if (a === "pick-mic") Object.assign(live, { mic: Number(value), open: null });
	if (a === "pick-cam") Object.assign(live, { cam: Number(value), open: null });
	if (a === "preview") live.preview = !live.preview;
	if (a === "hide") Object.assign(live, { hidden: true, pop: null });
	if (a === "show") live.hidden = false;
	if (a === "ask-restart") Object.assign(live, { ask: "restart", pop: null });
	if (a === "ask-delete") Object.assign(live, { ask: "delete", pop: null });
	if (a === "ask-no") live.ask = null;
	if (a === "ask-yes") Object.assign(live, { ask: null, sec: 0, paused: false });
	if (a === "draw") Object.assign(live, { draw: !live.draw, pop: null });
	if (a === "tool") live.tool = value;
	if (a === "colour") live.colour = Number(value);
	if (a === "undo-ink") { ink.pop(); renderInk(); }
	if (a === "clear-ink") { ink = []; renderInk(); }
	if (a === "dismiss") live.issueHidden = true;
	if (a === "issue") live.issueHidden = !live.issueHidden;
	if (a === "tab") {
		live.tabOpen = true;
		clearTimeout(folding);
		folding = setTimeout(() => { live.tabOpen = false; renderDock(); }, 2000);
	}
	renderDock();
};

document.addEventListener("click", (event) => {
	const sw = event.target.closest("[data-sw]");
	if (sw) return cycle(Number(sw.dataset.sw));
	const segment = event.target.closest("[data-seg] button");
	if (segment) {
		const name = segment.closest("[data-seg]").dataset.seg;
		const v = segment.dataset.v;
		if (name === "mode") { live.mode = v; live.muted = false; renderLive(); renderStates(); }
		if (name === "mic-state") { live.startedMic = v === "on"; renderLive(); }
		if (name === "issue") { live.issue = v === "none" ? null : v; live.issueHidden = false; renderLive(); }
		if (name === "place") { opt.place = v; opt.open = false; renderOptions(); }
		if (name === "opt-mode") { opt.mode = v; if (v === "screenshot") opt.open = false; renderOptions(); }
		if (["quality", "res", "fps", "countdown"].includes(name)) { opt[name] = v; renderOptions(); }
		return;
	}
	const toggle = event.target.closest("[data-opt]");
	if (toggle) { opt.open = !opt.open; renderOptions(); return; }
	const sw2 = event.target.closest("[data-o]");
	if (sw2) { opt[sw2.dataset.o] = !opt[sw2.dataset.o]; renderOptions(); return; }
	const action = event.target.closest("#screen [data-a]");
	if (action && !action.disabled) act(action.dataset.a, action.dataset.v);
});

document.addEventListener("keydown", (event) => {
	const inBar = event.target.closest?.("#screen");
	if (event.key === "Escape") {
		if (live.pop || live.ask) { Object.assign(live, { pop: null, ask: null }); renderDock(); }
		if (opt.open) { opt.open = false; renderOptions(); }
		return;
	}
	if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
	const step = event.key === "ArrowLeft" ? -1 : 1;
	if (inBar) {
		const list = [...screen.querySelectorAll(".rb button:not([disabled]):not([tabindex='-1'])")];
		const at = list.indexOf(event.target);
		if (at >= 0) { event.preventDefault(); list[Math.min(list.length - 1, Math.max(0, at + step))].focus(); }
		return;
	}
	if (!event.target.closest("input, textarea, [contenteditable]")) cycle(step);
});

screen.innerHTML = `${wall}<svg class="ink" viewBox="0 0 960 540" preserveAspectRatio="none" aria-hidden="true"></svg><div id="dock" class="dock bottom"></div><div class="hidden-chip" id="hidden-chip" hidden><span>${L("Controls hidden. The tray and Ctrl Shift H bring them back.", "控制条已隐藏。托盘或 Ctrl Shift H 可恢复。")}</span><button class="btn" data-a="show">${L("Show", "显示")}</button></div>`;

const inkLayer = screen.querySelector(".ink");
const inkPoint = (event) => {
	const box = inkLayer.getBoundingClientRect();
	return `${Math.round(((event.clientX - box.left) / box.width) * 960)},${Math.round(((event.clientY - box.top) / box.height) * 540)}`;
};
inkLayer.addEventListener("pointerdown", (event) => {
	ink.push({ points: [inkPoint(event)], colour: live.colour, tool: live.tool });
	inkLayer.setPointerCapture(event.pointerId);
});
inkLayer.addEventListener("pointermove", (event) => {
	if (!event.buttons || !ink.length) return;
	ink[ink.length - 1].points.push(inkPoint(event));
	renderInk();
});

setInterval(() => {
	if (live.paused) return;
	live.sec += 1;
	for (const node of screen.querySelectorAll("[data-time]")) node.textContent = fmt(live.sec);
}, 1000);
setInterval(() => {
	for (const bar of screen.querySelectorAll(".rb-meter b")) bar.style.width = `${live.paused ? 0 : 20 + Math.random() * 60}%`;
}, 140);

shell({ here: "recording-controls", depth: 1, sections: [["live", "Bar", "控制条"], ["states", "States", "状态"], ["options", "Options", "选项"], ["keyboard", "Keyboard", "键盘"], ["inventory", "Everything", "去向"], ["additions", "Additions", "新增"], ["references", "References", "参考"]] });
renderAll();
onChange(renderAll);
