const { L, icon, pick, shell, onChange } = FE;

const KINDS = [
	["studio", "Studio recording", "工作室录制"],
	["instant", "Instant recording", "即时录制"],
	["audio", "Audio only", "仅音频"],
	["shot", "Screenshot", "截图"],
];
const STATES = [
	["ready", "Ready", "就绪"],
	["working", "Working", "进行中"],
	["done", "Done", "完成"],
	["failed", "Failed", "失败"],
];
const ITEM = {
	studio: {
		name: ["Product demo", "产品演示"],
		meta: "3:42",
		acts: [
			["pencil", "Edit", "编辑"],
			["download", "Export", "导出"],
		],
		more: [
			["folder-open", "Show in library", "在资源库中显示"],
			["trash", "Delete", "删除"],
		],
		working: ["Rendering video", "正在渲染视频"],
		done: ["Saved Product demo.mp4", "已保存 产品演示.mp4"],
		failed: ["Couldn't save the video", "无法保存视频"],
	},
	instant: {
		name: ["Bug report", "问题复现"],
		meta: "0:48 · 6 MB",
		acts: [
			["play", "Open", "打开"],
			["copy", "Copy", "复制"],
		],
		more: [
			["folder-open", "Show in folder", "在文件夹中显示"],
			["trash", "Delete", "删除"],
		],
		working: ["Copying", "正在复制"],
		done: ["Copied", "已复制"],
		failed: ["Couldn't copy", "无法复制"],
	},
	audio: {
		name: ["Voice note", "语音备忘"],
		meta: "1:05",
		acts: [
			["pencil", "Edit", "编辑"],
			["download", "Export audio", "导出音频"],
		],
		more: [
			["folder-open", "Show in library", "在资源库中显示"],
			["trash", "Delete", "删除"],
		],
		working: ["Exporting audio", "正在导出音频"],
		done: ["Saved Voice note.mp3", "已保存 语音备忘.mp3"],
		failed: ["Couldn't save the audio", "无法保存音频"],
	},
	shot: {
		name: ["Pricing page", "定价页面"],
		meta: "1920 × 1080",
		acts: [
			["pencil", "Edit", "编辑"],
			["copy", "Copy", "复制"],
		],
		more: [
			["folder-open", "Show in folder", "在文件夹中显示"],
			["trash", "Delete", "删除"],
		],
		working: ["Copying", "正在复制"],
		done: ["Copied", "已复制"],
		failed: ["Couldn't copy", "无法复制"],
	},
};
const s = { kind: "studio", st: "ready", stack: false };

const seg = (name, value, options) =>
	`<div class="seg" data-tool="${name}">${options.map(([v, en, zh]) => `<button class="${value === v ? "on" : ""}" data-v="${v}">${L(en, zh)}</button>`).join("")}</div>`;
const tool = (en, zh, html) =>
	`<div class="tool"><span>${L(en, zh)}</span>${html}</div>`;
const table = (id, head, list) => {
	const cell = (c) => (Array.isArray(c) ? L(...c) : c);
	document.getElementById(id).innerHTML =
		`<table class="inv"><thead><tr>${head.map((h) => `<th>${L(...h)}</th>`).join("")}</tr></thead><tbody>${list.map((r) => `<tr>${r.map((c) => `<td>${cell(c)}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
};
const text = (id, en, zh) => {
	document.getElementById(id).innerHTML = L(en, zh);
};

const thumb = (kind) =>
	kind === "audio"
		? `<div class="wave">${[40, 70, 100, 60, 85, 45, 75, 55, 90, 50].map((h) => `<i style="height:${h}%"></i>`).join("")}</div>`
		: `<div class="scene"></div>`;
const status = (item, st) => {
	if (st === "working")
		return `<div class="rc-status"><span class="grow">${L(...item.working)}</span><button class="btn ghost">${L("Cancel", "取消")}</button></div><div class="progress"><i style="width:42%"></i></div>`;
	if (st === "done")
		return `<div class="rc-status good">${icon("circle-check", "sm")}<span class="grow">${L(...item.done)}</span><button class="btn ghost">${L("Show in folder", "在文件夹中显示")}</button></div>`;
	if (st === "failed")
		return `<div class="rc-status bad">${icon("triangle-alert", "sm")}<span class="grow">${L(...item.failed)}</span><button class="btn ghost">${L("Try again", "重试")}</button></div>`;
	return "";
};
const card = (kind, st, small = false) => {
	const item = ITEM[kind];
	const title = pick("Drag to another app", "拖到其他应用");
	return `<div class="rc ${small ? "small" : ""}" role="status">
		<div class="rc-thumb" title="${title}">${thumb(kind)}${kind === "shot" ? "" : `<span class="rc-chip">${item.meta.split(" · ")[0]}</span>`}<button class="rc-close" title="${pick("Close", "关闭")}" aria-label="${pick("Close", "关闭")}">${icon("x", "sm")}</button></div>
		<div class="rc-body">
			<div class="rc-name"><b>${L(...item.name)}</b><span>${item.meta}</span></div>
			${
				st === "ready"
					? `<div class="rc-acts">${item.acts.map(([ic, en, zh]) => `<button class="btn">${icon(ic, "sm")}${L(en, zh)}</button>`).join("")}${item.more.map(([ic, en, zh]) => `<button class="btn ghost icon" title="${pick(en, zh)}" aria-label="${pick(en, zh)}">${icon(ic, "sm")}</button>`).join("")}</div>`
					: status(item, st)
			}
		</div>
	</div>`;
};

const paint = () => {
	document.getElementById("tools").innerHTML =
		`${tool("After", "来源", seg("kind", s.kind, KINDS))}${tool("State", "状态", seg("st", s.st, STATES))}${tool(
			"Cards",
			"卡片数",
			seg("stack", s.stack ? "y" : "n", [
				["n", "One", "一张"],
				["y", "Three", "三张"],
			]),
		)}`;
	document.getElementById("stage").innerHTML =
		`<div class="stack">${s.stack ? card("shot", "ready", true) + card("instant", "ready", true) : ""}${card(s.kind, s.st)}</div><div class="taskbar"></div>`;
	document.getElementById("state-grid").innerHTML = KINDS.flatMap(
		([kind, en, zh]) =>
			(kind === "studio" ? STATES : [STATES[0]]).map(
				([st, sen, szh]) =>
					`<figure>${card(kind, st)}<figcaption>${L(`${en} · ${sen}`, `${zh} · ${szh}`)}</figcaption></figure>`,
			),
	).join("");
};

const copy = () => {
	text("t-card", "Result card", "结果卡片");
	text(
		"l-card",
		"A small card in the corner of the display after a recording or a screenshot, when After a recording is set to Show card. 300 px wide, above the taskbar, left out of recordings.",
		"录制或截图结束后出现在显示器角落的小卡片，在“录制后”设为“显示卡片”时出现。宽 300 px，在任务栏上方，不会录进视频。",
	);
	text("t-states", "States", "状态");
	text(
		"l-states",
		"Each kind offers its two most likely next steps as buttons, then Show and Delete as icons. An action that takes time replaces the buttons with its progress, and then its result.",
		"每种来源把最可能的两个下一步做成按钮，后面是“显示”和“删除”两个图标。耗时的操作会把按钮换成进度，然后是结果。",
	);
	text("t-rules", "Behaviour", "行为");
	text("l-rules", "What a press cannot show here.", "这里按不出来的行为。");
	text("t-inv", "Where everything went", "每项功能的位置");
	text(
		"l-inv",
		"Upstream's recordings overlay, action by action.",
		"上游录制浮层的每个操作。",
	);
	text("t-ref", "References", "参考");
	text(
		"l-ref",
		"Where each choice came from. The products were compared from what is known of them, not opened again for this page.",
		"每个选择的出处。这些产品是凭已知情况比较的，没有为这一页重新打开。",
	);
	table(
		"rule-table",
		[
			["Behaviour", "行为"],
			["Rule", "规则"],
		],
		[
			[
				["When it appears", "何时出现"],
				[
					"Only when Settings › After a Studio recording, After an Instant recording or After a screenshot is Show card. The other choices keep what they do today",
					"只在设置里“工作室录制后”“即时录制后”或“截图后”选了“显示卡片”时出现。其他选项保持现在的行为",
				],
			],
			[
				["How long it stays", "停留多久"],
				[
					"8 seconds, like the library's saved notice. It stays while the pointer is over it, while an action runs, and after a failure until it is closed",
					"8 秒，和资源库的保存提示一样。指针悬停、操作进行中、失败后都会一直留着，直到关闭",
				],
			],
			[
				["Several cards", "多张卡片"],
				[
					"The newest is at the bottom with its thumbnail; up to two older ones stack above it without thumbnails. A fourth replaces the oldest",
					"最新的一张在最下面，带缩略图；上面最多叠两张不带缩略图的旧卡片。第四张出现时替换最旧的",
				],
			],
			[
				["Drag", "拖动"],
				[
					"Dragging the thumbnail drops the file into another app. A Studio recording has no single file until it is exported, so its thumbnail does not drag",
					"拖动缩略图可以把文件放进其他应用。工作室录制在导出之前没有单个文件，所以它的缩略图不能拖",
				],
			],
			[
				["Keyboard", "键盘"],
				[
					"The card never takes focus. Esc closes it only while it has focus from a click or Tab",
					"卡片不抢焦点。只有在点击或 Tab 让它获得焦点时，Esc 才关闭它",
				],
			],
			[
				["Where it sits", "位置"],
				[
					"The display that was recorded, bottom right, 16 px from the work area's edges. During another recording it is hidden from the capture like the recording bar",
					"在被录制的那个显示器右下角，距工作区边缘 16 px。再次录制时和录制条一样不会被录进去",
				],
			],
			[
				["Delete", "删除"],
				[
					"Moves the recording to the Recycle Bin and turns the card into Deleted with Undo, for the same 8 seconds",
					"把录制移到回收站，卡片变成“已删除”和“撤销”，同样停留 8 秒",
				],
			],
		],
	);
	table(
		"inv-table",
		[
			["Upstream", "上游"],
			["Here", "这里"],
		],
		[
			[
				["Open", "打开"],
				[
					"Open, for an Instant recording (plays it in the library's preview). A Studio recording has Edit instead",
					"“打开”，用于即时录制（在资源库预览里播放）。工作室录制用“编辑”",
				],
			],
			[
				["Edit", "编辑"],
				[
					"Edit: the editor for a Studio recording, the screenshot editor for a screenshot",
					"“编辑”：工作室录制打开编辑器，截图打开截图编辑器",
				],
			],
			[
				["Copy to clipboard", "复制到剪贴板"],
				[
					"Copy, for an Instant recording and a screenshot. A Studio recording is copied from the editor's export page",
					"“复制”，用于即时录制和截图。工作室录制在编辑器的导出页复制",
				],
			],
			[
				["Export", "导出"],
				[
					"Export, for a Studio recording: renders with the last export settings into the storage folder and shows progress on the card. Export audio for an audio only recording",
					"“导出”，用于 工作室录制：按上次的导出设置渲染到存储文件夹，进度显示在卡片上。仅音频录制是“导出音频”",
				],
			],
			[
				["Close", "关闭"],
				["The close button on the thumbnail", "缩略图上的关闭按钮"],
			],
			[
				["Retry", "重试"],
				["Try again, on a failed action", "操作失败后的“重试”"],
			],
			[
				[
					"Create shareable link, Copy shareable link",
					"创建分享链接、复制分享链接",
				],
				[
					"Not in FrameEver: there is no upload and no account",
					"帧刻没有：不上传，也没有账户",
				],
			],
			[
				[
					"Show in overlay (after-recording choice)",
					"在浮层中显示（录制后选项）",
				],
				[
					"Show card, in the three After… rows of Settings",
					"“显示卡片”，在设置的三个“…后”行里",
				],
			],
			[
				["Not upstream", "上游没有"],
				[
					"Show in library or Show in folder, Delete with Undo, dragging the thumbnail out, stacking",
					"“在资源库中显示”或“在文件夹中显示”、带撤销的删除、拖出缩略图、叠放",
				],
			],
		],
	);
	table(
		"ref-table",
		[
			["Choice", "选择"],
			["From", "出处"],
		],
		[
			[
				["A card with a thumbnail and actions", "带缩略图和操作的卡片"],
				[
					"Upstream Cap's recordings overlay (dbaae65b3)",
					"上游 Cap 的录制浮层（dbaae65b3）",
				],
			],
			[
				["Stays a few seconds, longer on hover", "停留几秒，悬停时不消失"],
				[
					"ShareX's after-capture toast; the screenshot thumbnail in macOS",
					"ShareX 的截图后提示；macOS 的截图缩略图",
				],
			],
			[
				["Drag the thumbnail into another app", "把缩略图拖进其他应用"],
				[
					"macOS screenshot thumbnail; Flameshot's pin and drag",
					"macOS 截图缩略图；Flameshot 的贴图和拖动",
				],
			],
			[
				["Progress and result on the card itself", "进度和结果显示在卡片上"],
				["Upstream Cap; Kap's export window", "上游 Cap；Kap 的导出窗口"],
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
	const name = option.closest("[data-tool]").dataset.tool;
	if (name === "stack") s.stack = option.dataset.v === "y";
	else s[name] = option.dataset.v;
	paint();
});

shell({
	here: "result-card",
	depth: 1,
	sections: [
		["card", "Card", "卡片"],
		["states", "States", "状态"],
		["behaviour", "Behaviour", "行为"],
		["inventory", "Where everything went", "功能位置"],
		["references", "References", "参考"],
	],
});
renderAll();
onChange(renderAll);
