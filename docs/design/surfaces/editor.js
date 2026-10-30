const { L, icon, pick, shell, onChange } = FE;

Object.assign(window.FRAMEEVER_ICONS, {
	redo: '<path d="M21 7v6h-6"/><path d="M3 17a9 9 0 0 1 9-9 9 9 0 0 1 6 2.3l3 2.7"/>',
	crop: '<path d="M6 2v14a2 2 0 0 0 2 2h14"/><path d="M18 22V8a2 2 0 0 0-2-2H2"/>',
	proportions: '<rect width="20" height="16" x="2" y="4" rx="2"/><path d="M12 9v11"/><path d="M2 9h13a2 2 0 0 1 2 2v9"/>',
	"skip-back": '<polygon points="19 20 9 12 19 4 19 20"/><line x1="5" x2="5" y1="19" y2="5"/>',
	"skip-forward": '<polygon points="5 4 15 12 5 20 5 4"/><line x1="19" x2="19" y1="5" y2="19"/>',
	"zoom-in": '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/><path d="M11 8v6M8 11h6"/>',
	"zoom-out": '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/><path d="M8 11h6"/>',
	"mouse-pointer-2": '<path d="m4 4 7.07 17 2.51-7.39L21 11.07z"/>',
	captions: '<rect width="18" height="14" x="3" y="5" rx="2" ry="2"/><path d="M7 15h4M15 15h2M7 11h2M13 11h4"/>',
	box: '<path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"/><path d="m3.3 7 8.7 5 8.7-5"/><path d="M12 22V12"/>',
	layers: '<path d="m12 2 10 5-10 5L2 7z"/><path d="m2 12 10 5 10-5"/><path d="m2 17 10 5 10-5"/>',
	music: '<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>',
	mask: '<rect width="18" height="18" x="3" y="3" rx="2" stroke-dasharray="3 3"/>',
	bookmark: '<path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z"/>',
});

const glyphs = {
	minimize: '<svg viewBox="0 0 10 10"><path d="M0 5.5h10"/></svg>',
	maximize: '<svg viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" rx="1"/></svg>',
	close: '<svg viewBox="0 0 10 10"><path d="m0.5 0.5 9 9m0-9-9 9"/></svg>',
};
const scene =
	'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 180" preserveAspectRatio="xMidYMid slice" aria-hidden="true"><rect width="320" height="180" fill="#F2F1EC"/><rect width="60" height="180" fill="#D9D6CB"/><rect x="72" y="16" width="120" height="10" rx="2" fill="#2B3A55"/><rect x="72" y="36" width="210" height="6" rx="2" fill="#A9A79C"/><rect x="72" y="50" width="180" height="6" rx="2" fill="#A9A79C"/><rect x="72" y="72" width="104" height="72" rx="4" fill="#E2725B"/><rect x="186" y="72" width="104" height="72" rx="4" fill="#3E7CB1"/><rect x="10" y="20" width="40" height="6" rx="2" fill="#2B3A55"/><rect x="10" y="36" width="32" height="6" rx="2" fill="#A9A79C"/><rect x="10" y="52" width="36" height="6" rx="2" fill="#A9A79C"/></svg>';

const TOTAL = 222;
const fmt = (s) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

const sl = (k, en, zh, min, max, val, unit = "", step = 1) => ({ t: "sl", k, en, zh, min, max, val, unit, step });
const tg = (k, en, zh, val = false) => ({ t: "tg", k, en, zh, val });
const se = (k, en, zh, opts, val = 0) => ({ t: "se", k, en, zh, opts, val });
const sg = (k, opts, val = 0) => ({ t: "sg", k, opts, val });
const sw = (en, zh, color) => ({ t: "sw", en, zh, color });
const nt = (en, zh) => ({ t: "nt", en, zh });
const bt = (en, zh) => ({ t: "bt", en, zh });
const pd = (k, en, zh, val = 5) => ({ t: "pd", k, en, zh, val });
const ti = (k, opts, val = 0, four = false) => ({ t: "ti", k, opts, val, four });
const gr = (en, zh, rows, opt = {}) => ({ en, zh, rows, ...opt });
const O = (...pairs) => pairs;
const positions = O(["Top left", "左上"], ["Top centre", "上中"], ["Top right", "右上"], ["Bottom left", "左下"], ["Bottom centre", "下中"], ["Bottom right", "右下"]);
const fonts = O(["System sans-serif", "系统无衬线"], ["System serif", "系统衬线"], ["System monospace", "系统等宽"]);
const weights = O(["Normal", "常规"], ["Medium", "中等"], ["Bold", "粗体"]);
const corners = O(["Squircle", "超椭圆"], ["Rounded", "圆角"]);
const grades = O(["None", "无"], ["Cinematic", "电影"], ["Noir", "黑白"], ["Vintage", "复古"], ["Frost", "霜冷"], ["Golden", "金色"], ["Midnight", "午夜"], ["Vivid", "鲜艳"], ["Dreamy", "梦幻"]);
const colour = (p) =>
	gr("Color correction", "调色", [
		ti(`${p}grade`, grades),
		sl(`${p}grain`, "Grain", "颗粒", 0, 100, 0),
		...(p === "s" ? [tg("gradeCursor", "Apply to cursor", "应用到光标")] : []),
	], { shut: true, sub: gr("Fine-tune colors", "微调颜色", [
		sl(`${p}str`, "Strength", "强度", 0, 100, 100), sl(`${p}exp`, "Exposure", "曝光", -100, 100, 0), sl(`${p}con`, "Contrast", "对比度", -100, 100, 0),
		sl(`${p}sat`, "Saturation", "饱和度", -100, 100, 0), sl(`${p}tmp`, "Temperature", "色温", -100, 100, 0), sl(`${p}tnt`, "Tint", "色调", -100, 100, 0),
		sl(`${p}spl`, "Split tone", "分离色调", -100, 100, 0), sl(`${p}fad`, "Fade", "褪色", 0, 100, 0), sl(`${p}vig`, "Vignette", "暗角", 0, 100, 0),
	], { shut: true }) });

const bgBodies = {
	desktop: [bt("Import desktop background", "导入桌面背景")],
	wallpaper: [sg("wtheme", O(["macOS", "macOS"], ["Dark", "深色"], ["Blue", "蓝色"], ["Cities", "城市"], ["Purple", "紫色"], ["Orange", "橙色"])), { t: "swatches", n: 9 }],
	image: [bt("Choose or drop an image", "选择或拖入图片"), nt("JPG, PNG, GIF, WebP, BMP", "JPG、PNG、GIF、WebP、BMP")],
	color: [sw("Color", "颜色", "#FFFFFF"), { t: "swatches", n: 17, solid: true }],
	gradient: [{ t: "swatches", n: 18, grad: true }, sw("From", "起始色", "#4785FF"), sw("To", "结束色", "#FF4766"), sl("gAngle", "Angle", "角度", 0, 360, 90, "°"), sl("gNoise", "Noise", "噪点", 0, 100, 0), sl("gGrain", "Grain scale", "颗粒大小", 1, 100, 3), bt("Randomize", "随机")],
	animated: [{ t: "swatches", n: 12, grad: true }, sl("aMotion", "Motion", "运动", 0, 100, 40), { t: "stops" }, bt("Randomize", "随机"), bt("Save as template", "保存为模板"),
		gr("Fine-tune", "微调", [
			sg("aTab", O(["Flow", "流动"], ["Lighting", "光照"], ["Texture", "质感"], ["Colour", "色彩"])),
			sl("aDir", "Direction", "方向", 0, 360, 45, "°"), sl("aScale", "Flow scale", "流动尺度", 0.5, 5, 2, "", 0.1), sl("aStr", "Flow strength", "流动强度", 0, 100, 50), sl("aCurve", "Curvature", "曲率", 0, 100, 50), sl("aDet", "Detail", "细节", 1, 6, 3),
		], { shut: true })],
};

const TABS = {
	clips: { icon: "film", en: "Clips", zh: "片段" },
	background: { icon: "image", en: "Background", zh: "背景" },
	screen: { icon: "monitor", en: "Screen", zh: "画面" },
	camera: { icon: "video", en: "Camera", zh: "摄像头" },
	audio: { icon: "volume-2", en: "Audio", zh: "音频" },
	cursor: { icon: "mouse-pointer-2", en: "Cursor", zh: "光标" },
	keyboard: { icon: "keyboard", en: "Keyboard", zh: "按键" },
	captions: { icon: "captions", en: "Captions", zh: "字幕" },
	
	overlays: { icon: "layers", en: "Overlays", zh: "叠加" },
	scenes: { icon: "box", en: "Scenes", zh: "场景" },
};
const groups = {
	screen: () => [
		gr("Frame", "窗口外框", [ti("frame", O(["None", "无"], ["macOS", "macOS"], ["Windows", "Windows"], ["Browser", "浏览器"], ["MacBook", "MacBook"])), sg("frameTheme", O(["Light", "浅色"], ["Dark", "深色"])), { t: "input", en: "URL", zh: "网址", ph: "frameever.app" }]),
		gr("Layout", "布局", [sl("padding", "Padding", "内边距", 0, 40, 10, "%"), sl("rounding", "Rounded corners", "圆角", 0, 100, 8, "%"), se("corner", "Corner style", "圆角样式", corners), sl("motion", "Motion blur", "运动模糊", 0, 100, 100, "%")]),
		gr("Shadow", "阴影", [sl("shadow", "Shadow", "阴影", 0, 100, 74)], { sub: gr("Advanced shadow settings", "高级阴影设置", [sl("shS", "Size", "大小", 0, 100, 14), sl("shO", "Opacity", "不透明度", 0, 100, 68), sl("shB", "Blur", "模糊", 0, 100, 4)], { shut: true }) }),
		gr("Border", "描边", [tg("border", "Show border", "显示描边"), { when: "border", rows: [sl("bW", "Width", "宽度", 1, 20, 5), sw("Color", "颜色", "#000000"), sl("bO", "Opacity", "不透明度", 0, 100, 50)] }], { shut: true }),
		gr("MacBook notch", "MacBook 刘海", [tg("notch", "Show notch", "显示刘海"), { when: "notch", rows: [sl("nW", "Width", "宽度", 0, 40, 18), sl("nH", "Height", "高度", 0, 15, 4), sl("nP", "Position", "位置", 0, 100, 50)] }], { shut: true }),
		colour("s"),
	],
	camera: () => [
		gr("Placement", "位置", [pd("camPos", "Position", "位置"), tg("camHide", "Hide camera", "隐藏摄像头"), tg("camMirror", "Mirror camera", "镜像摄像头"), se("camBg", "Background", "背景", O(["Off", "关闭"], ["Light blur", "轻度模糊"], ["Heavy blur", "重度模糊"])), se("camShape", "Shape", "形状", O(["Square", "方形"], ["Source", "原始比例"]))]),
		gr("Size", "大小", [sl("camSize", "Size", "大小", 20, 80, 30, "%"), sl("camZoom", "Size during zoom", "缩放时的大小", 10, 60, 60, "%"), tg("camKeep", "Keep original size during zoom", "缩放时保持原大小")]),
		gr("Corners and shadow", "圆角和阴影", [sl("camR", "Rounded corners", "圆角", 0, 100, 100, "%"), se("camC", "Corner style", "圆角样式", corners), sl("camS", "Shadow", "阴影", 0, 100, 63)], { sub: gr("Advanced shadow settings", "高级阴影设置", [sl("cS", "Size", "大小", 0, 100, 34), sl("cO", "Opacity", "不透明度", 0, 100, 44), sl("cB", "Blur", "模糊", 0, 100, 11)], { shut: true }) }),
		colour("c"),
	],
	audio: () => [
		gr("Audio", "音频", [tg("mute", "Mute audio", "静音"), se("stereo", "Microphone stereo mode", "麦克风立体声", O(["Stereo", "立体声"], ["Mono L", "左声道"], ["Mono R", "右声道"])), sl("mic", "Microphone volume", "麦克风音量", -30, 10, 0, " dB", 0.1), sl("sys", "System audio volume", "系统音频音量", -30, 10, 0, " dB", 0.1)]),
		gr("Studio Sound", "录音棚音质", [tg("sound", "Studio Sound", "录音棚音质"), { when: "sound", rows: [sg("soundLevel", O(["Light", "轻度"], ["Balanced", "均衡"], ["Strong", "强力"]), 1), nt("Removes room echo and background noise.", "去除房间回声和背景噪声。"), tg("soundNew", "Use for new recordings", "用于新录制")] }]),
		gr("Sync", "同步", [{ t: "sync", en: "System audio", zh: "系统音频" }, { t: "sync", en: "Microphone", zh: "麦克风" }, { t: "sync", en: "Camera", zh: "摄像头" }], { shut: true }),
	],
	cursor: () => [
		gr("Cursor", "光标", [tg("cursor", "Show cursor", "显示光标", true), { when: "cursor", rows: [ti("curStyle", O(["Default", "默认"], ["Windows", "Windows"], ["macOS", "macOS"], ["macOS Tahoe", "macOS Tahoe"], ["Circle", "圆形"])), sl("curSize", "Size", "大小", 20, 300, 100, "%"), sl("curTilt", "Tilt", "倾斜", 0, 100, 15, "%"), tg("curSvg", "High quality SVG cursors", "高清 SVG 光标", true)] }]),
		gr("Movement", "移动", [ti("curMove", O(["Slow", "缓慢"], ["Smooth", "顺滑"], ["Mellow", "柔和"], ["Fast", "迅速"]), 2, true), tg("curSmooth", "Smooth movement", "平滑移动", true), { when: "curSmooth", rows: [sl("tens", "Tension", "张力", 1, 600, 470), sl("fric", "Friction", "摩擦", 0, 200, 70), sl("mass", "Mass", "质量", 0.1, 15, 3, "", 0.1)] }], { shut: true }),
		gr("Click ripple", "点击波纹", [tg("ripple", "Click ripple", "点击波纹"), { when: "ripple", rows: [sw("Color", "颜色", "#4785FF"), sl("rS", "Strength", "强度", 0, 100, 70, "%"), sl("rZ", "Size", "大小", 25, 300, 100, "%"), sl("rD", "Duration", "时长", 0.2, 1.5, 0.6, " s", 0.05)] }], { shut: true }),
		gr("Hide when idle", "空闲时隐藏", [tg("idle", "Hide when idle", "空闲时隐藏"), { when: "idle", rows: [sl("idleD", "Inactivity delay", "空闲延迟", 0.5, 5, 2, " s", 0.1)] }], { shut: true }),
	],
	keyboard: () => [
		gr("Keyboard", "按键", [tg("keys", "Show keyboard", "显示按键"), bt("Generate keyboard segments", "生成按键片段")]),
		gr("Style", "样式", [se("kFont", "Font family", "字体", fonts), sl("kSize", "Size", "大小", 12, 100, 50), se("kWeight", "Font weight", "字重", weights), sw("Text color", "文字颜色", "#FFFFFF"), sw("Background color", "背景颜色", "#000000"), sl("kBg", "Background opacity", "背景不透明度", 0, 100, 95, "%"), se("kPos", "Position", "位置", positions, 4)]),
		gr("Timing", "时间", [sl("kFade", "Fade duration", "淡入淡出时长", 0, 50, 15), sl("kLinger", "Linger duration", "停留时长", 0, 300, 80), sl("kGroup", "Grouping threshold", "合并阈值", 50, 1000, 500, " ms", 10)], { shut: true }),
		gr("What's shown", "显示内容", [tg("kMod", "Show modifiers", "显示修饰键", true), tg("kSpec", "Show special keys", "显示特殊键", true), tg("kUp", "Uppercase", "大写")], { shut: true }),
	],
	captions: () => [
		gr("Transcription", "转写", [se("model", "Model", "模型", O(["Recommended · ~640MB", "推荐 · ~640MB"], ["High accuracy · ~2.4GB", "高精度 · ~2.4GB"], ["Small · 466MB", "小 · 466MB"], ["Medium · 1.5GB", "中 · 1.5GB"])), se("lang", "Language", "语言", O(["Auto detect", "自动检测"], ["English", "English"], ["Chinese", "中文"])), bt("Regenerate captions", "重新生成字幕"), { t: "link", en: "Delete model", zh: "删除模型" }]),
		gr("Style", "样式", [ti("capPreset", O(["Classic", "经典"], ["Karaoke", "卡拉 OK"], ["Highlight", "高亮"], ["Pop", "跳动"], ["Minimal", "极简"])), se("capFont", "Font family", "字体", fonts), sl("capSize", "Size", "大小", 12, 100, 24), se("capWeight", "Font weight", "字重", weights, 2), tg("capUp", "Uppercase", "大写"), sw("Text color", "文字颜色", "#FFFFFF"), sw("Background color", "背景颜色", "#000000"), sl("capBg", "Background opacity", "背景不透明度", 0, 100, 90, "%")]),
		gr("Active word", "当前词", [tg("capWord", "Active word highlight", "当前词高亮"), { when: "capWord", rows: [se("capHl", "Highlight style", "高亮样式", O(["Color", "颜色"], ["Pill", "胶囊"])), sw("Highlight color", "高亮颜色", "#FFD400")] }], { shut: true }),
		gr("Position and motion", "位置和动画", [se("capPos", "Position", "位置", [["Manual", "手动"], ...positions], 5), se("capAnim", "Animation style", "动画样式", O(["None", "无"], ["Bounce", "弹跳"], ["Pop", "弹出"]), 1), sl("capFade", "Fade duration", "淡入淡出时长", 0, 50, 15)], { shut: true }),
	],
};
groups.background = () => [
	gr("Source", "来源", [sg("bgKind", O(["Desktop", "桌面"], ["Wallpaper", "壁纸"], ["Image", "图片"], ["Color", "纯色"], ["Gradient", "渐变"], ["Animated", "动态"]), 4), { t: "bgbody" }]),
	gr("Blur", "模糊", [sl("bgBlur", "Background blur", "背景模糊", 0, 100, 0)]),
];

const timing = [{ t: "kv", en: "Start", zh: "开始", v: "0:18.2" }, { t: "kv", en: "End", zh: "结束", v: "0:34.0" }, { t: "kv", en: "Duration", zh: "时长", v: "15.8 s" }];
const SEL = {
	zoom: { en: "Zoom", zh: "缩放", g: () => [gr("Zoom", "缩放", [sl("zA", "Amount", "倍数", 1, 4.5, 1.8, "×", 0.1), sg("zM", O(["Auto", "自动"], ["Manual", "手动"])), nt("Auto follows the cursor. Manual stays on one point.", "自动跟随光标，手动固定在一点。"), pd("zP", "Manual position", "手动位置", 1)])] },
	text: { en: "Text", zh: "文字", g: () => [
		gr("Content", "内容", [tg("tOn", "Enabled", "启用", true), { t: "input", en: "Text", zh: "文字", ph: "Product demo" }, sg("tAl", O(["Left", "左"], ["Centre", "中"], ["Right", "右"]), 1), tg("tIt", "Italic", "斜体"), tg("tAa", "AA", "AA")]),
		gr("Style presets", "样式预设", [sg("tPre", O(["Titles", "标题"], ["Lower thirds", "下方字幕条"], ["Callouts", "标注"], ["Statements", "陈述"], ["Code", "代码"])), { t: "swatches", n: 4, solid: true }]),
		gr("Type", "字体", [se("tF", "Font family", "字体", fonts), se("tW", "Weight", "字重", weights, 2), sl("tS", "Size", "大小", 8, 400, 64), sl("tL", "Line height", "行高", 0.8, 2, 1.2, "", 0.05), sl("tT", "Letter spacing", "字间距", -2, 20, 0), sw("Color", "颜色", "#FFFFFF"), tg("tGr", "Gradient", "渐变")]),
		gr("Effects", "效果", [se("tB", "Background", "背景", O(["None", "无"], ["Box", "方框"], ["Pill", "胶囊"], ["Highlight", "高亮"])), sl("tO", "Outline", "描边", 0, 12, 0), tg("tSh", "Shadow", "阴影"), tg("tGl", "Glow", "发光"), sl("tOp", "Opacity", "不透明度", 0, 100, 100, "%")], { shut: true }),
		gr("Animation", "动画", [sg("tIO", O(["In", "入场"], ["Out", "出场"])), ti("tAn", O(["None", "无"], ["Fade", "淡入"], ["Slide up", "上滑"], ["Slide down", "下滑"], ["Slide left", "左滑"], ["Slide right", "右滑"], ["Pop", "弹出"], ["Zoom", "缩放"], ["Bounce", "弹跳"], ["Wipe", "擦除"], ["Words", "逐词"], ["Letters", "逐字"], ["Tracking", "字距"], ["Typewriter", "打字机"]), 1, true), sl("tD", "Duration", "时长", 0, 3, 0.4, " s", 0.05)], { shut: true }),
		gr("Layout", "布局", [sg("tLy", O(["Overlay", "叠加"], ["Fullscreen", "全屏"])), sl("tTr", "Screen transition", "画面过渡", 0.1, 1.5, 0.5, " s", 0.05)], { shut: true }),
	] },
	captions: { en: "Caption", zh: "字幕", g: () => [gr("Caption", "字幕", [{ t: "input", en: "Text", zh: "文字", ph: "Now open the library" }, ...timing])] },
	keyboard: { en: "Keyboard", zh: "按键", g: () => [gr("Keyboard", "按键", [{ t: "input", en: "Keys", zh: "按键", ph: "Ctrl K" }, ...timing, sl("ksF", "Fade duration", "淡入淡出时长", 0, 50, 15)])] },
	audio: { en: "Audio", zh: "音频", g: () => [gr("Track", "音轨", [bt("Change track", "更换音轨"), tg("aOn", "Enabled", "启用", true), sl("aV", "Volume", "音量", -30, 12, -12, " dB", 0.5), sl("aI", "Fade in", "淡入", 0, 10, 1, " s", 0.1), sl("aOt", "Fade out", "淡出", 0, 10, 2, " s", 0.1)]), gr("Library", "音频库", [{ t: "list", items: O(["Lofi Beats", "Lofi Beats"], ["Raindrops", "Raindrops"], ["Sunday Mood", "Sunday Mood"], ["Good Night", "Good Night"], ["Empty Streets", "Empty Streets"], ["Study", "Study"], ["Cinematic", "Cinematic"], ["Hip Hop", "Hip Hop"], ["Cassette", "Cassette"], ["Smooth", "Smooth"]) }, bt("Import file", "导入文件")], { shut: true })] },
	mask: { en: "Mask", zh: "遮罩", g: () => [gr("Mask", "遮罩", [sg("mK", O(["Sensitive", "敏感信息"], ["Highlight", "高亮"])), tg("mOn", "Enabled", "启用", true), sg("mE", O(["Blur", "模糊"], ["Pixelate", "像素化"])), sl("mA", "Amount", "强度", 4, 80, 24), sl("mD", "Outside darkness", "外部暗度", 0, 100, 50, "%"), sl("mF", "Fade duration", "淡入淡出时长", 0, 100, 15)])] },
	scene: { en: "Scene", zh: "场景", g: () => [gr("Camera layout", "摄像头布局", [ti("scL", O(["Default", "默认"], ["Camera only", "仅摄像头"], ["Hide camera", "隐藏摄像头"], ["Split screen", "分屏"], ["Floating", "悬浮"]), 3), sl("scI", "Transition in", "入场过渡", 0, 2, 0.3, " s", 0.05), sl("scO", "Transition out", "出场过渡", 0, 2, 0.3, " s", 0.05)]), gr("Panes", "分屏内容", [sl("scZ", "Screen zoom", "画面缩放", 100, 300, 100, "%"), pd("scP", "Screen position", "画面位置", 1), sl("scC", "Camera zoom", "摄像头缩放", 100, 300, 100, "%"), pd("scQ", "Camera position", "摄像头位置", 1)])] },
	shot: { en: "3D shot", zh: "3D 镜头", g: () => [
		gr("Look", "外观", [bt("Play shot", "播放镜头"), sg("3k", O(["Moves", "运镜"], ["Angles", "角度"])), ti("3l", O(["Glide across", "横向滑过"], ["Drift down", "缓缓下移"], ["Rising sweep", "上升扫过"], ["Pull back", "后拉"], ["Top down", "俯视"], ["Tilt away", "倾斜离开"], ["Unfold", "展开"], ["Slide by", "滑过"])), nt("Sequences: Showcase, Product tour, Punch in", "镜头组：展示、产品导览、推近")]),
		gr("Pose", "姿态", [{ t: "orbit" }, sg("3p", O(["Start", "开始"], ["End", "结束"])), bt("Swap start and end", "交换开始和结束"), tg("3s", "Still shot", "静止镜头"), tg("3h", "Flip horizontally", "水平翻转"), tg("3v", "Flip vertically", "垂直翻转"), sl("3d", "Distance", "距离", 0.5, 10, 3, "", 0.1), sl("3r", "Roll", "滚转", -180, 180, 0, "°"), sl("3x", "Shift X", "X 偏移", -3, 3, 0, "", 0.1), sl("3y", "Shift Y", "Y 偏移", -3, 3, 0, "", 0.1)]),
		gr("Depth blur", "景深模糊", [tg("3b", "Depth blur", "景深模糊"), { when: "3b", rows: [sl("3a", "Amount", "强度", 0, 100, 40), se("3f", "Focus", "对焦", O(["Radial", "径向"], ["Directional", "方向"], ["Tilt shift", "移轴"]))] }], { shut: true }),
		gr("Timing and advanced", "时间和高级", [se("3m", "Motion style", "运动样式", O(["Linear", "线性"], ["Smooth", "平滑"], ["Ease in", "缓入"], ["Ease out", "缓出"]), 1), sl("3ei", "Ease in", "缓入", 0, 2, 0.5, " s", 0.05), sl("3eo", "Ease out", "缓出", 0, 2, 0.5, " s", 0.05), sl("3ln", "Lens", "镜头", 10, 100, 45), sl("3rx", "Rotate X", "X 旋转", -90, 90, 0, "°"), sl("3ry", "Rotate Y", "Y 旋转", -50, 50, 0, "°"), bt("Reset camera", "重置相机")], { shut: true }),
	] },
	image: { en: "Image", zh: "图片", g: () => [gr("Image", "图片", [{ t: "input", en: "Name", zh: "名称", ph: "Logo" }, bt("Center on canvas", "画布居中"), bt("Replace image", "替换图片"), tg("iOn", "Enabled", "启用", true), tg("iLock", "Lock aspect ratio", "锁定宽高比", true), tg("iH", "Flip horizontally", "水平翻转"), tg("iV", "Flip vertically", "垂直翻转"), sl("iR", "Rotation", "旋转", -180, 180, 0, "°"), sl("iC", "Rounding", "圆角", 0, 100, 0), sl("iO", "Opacity", "不透明度", 0, 100, 100, "%")])] },
	style: { en: "Style", zh: "样式", g: () => [gr("Style", "样式", [{ t: "input", en: "Name", zh: "名称", ph: "Style 1" }, tg("yOn", "Enabled", "启用", true), nt("Overrides settings for this stretch of the video.", "覆盖这一段视频的设置。"), tg("yB", "Background and frame", "背景和外框", true), tg("yC", "Camera", "摄像头"), tg("yR", "Cursor", "光标")]), gr("Screen crop", "画面裁剪", [{ t: "kv", en: "Left", zh: "左", v: "0" }, { t: "kv", en: "Top", zh: "上", v: "0" }, { t: "kv", en: "Width", zh: "宽", v: "2560" }, { t: "kv", en: "Height", zh: "高", v: "1440" }, bt("Reset crop", "重置裁剪")], { shut: true }), gr("Camera only", "仅摄像头", [tg("yO", "Background", "背景"), sl("yP", "Padding", "内边距", 0, 40, 10, "%")], { shut: true })] },
};

const clipSegs = [
	{ s: 0, e: 72, l: ["Clip 1", "片段 1"], speed: "1x" },
	{ s: 72, e: 130, l: ["Clip 2 · 2x", "片段 2 · 2x"], speed: "2x" },
	{ s: 130, e: 222, l: ["Clip 3", "片段 3"], speed: "1x" },
];
const words = [8, 22, 37, 52, 66, 84, 101, 118, 134, 149, 163, 180, 196, 208];
const TRACKS = [
	{ id: "video", icon: "video", en: "Video", zh: "视频", tool: "clips", big: true, segs: clipSegs },
	{ id: "zoom", icon: "zoom-in", en: "Zoom", zh: "缩放", tool: "zoom", segs: [{ s: 18, e: 34, l: ["1.8×", "1.8×"] }, { s: 96, e: 120, l: ["2.0×", "2.0×"] }] },
	{ id: "captions", icon: "captions", en: "Captions", zh: "字幕", tool: "captions", segs: words.map((s) => ({ s, e: s + 12, l: ["", ""] })) },
	{ id: "text", icon: "type", en: "Text", zh: "文字", tool: "overlays", segs: [{ s: 4, e: 14, l: ["Product demo", "Product demo"] }] },
	{ id: "audio", icon: "music", en: "Audio", zh: "音频", tool: "audio", segs: [{ s: 0, e: 222, l: ["Lofi Beats", "Lofi Beats"] }] },
	{ id: "keyboard", icon: "keyboard", en: "Keyboard", zh: "按键", tool: "keyboard", segs: [{ s: 40, e: 44, l: ["", ""] }, { s: 90, e: 94, l: ["", ""] }] },
	{ id: "style", icon: "sliders-horizontal", en: "Style", zh: "样式", tool: "scenes", segs: [{ s: 60, e: 100, l: ["Style 1", "样式 1"] }] },
	{ id: "image", icon: "image", en: "Image", zh: "图片", tool: "overlays", segs: [{ s: 0, e: 30, l: ["Logo", "Logo"] }] },
	{ id: "mask", icon: "mask", en: "Mask", zh: "遮罩", tool: "overlays", segs: [{ s: 140, e: 160, l: ["Sensitive", "敏感信息"] }] },
	{ id: "shot", icon: "box", en: "3D", zh: "3D", tool: "scenes", segs: [{ s: 170, e: 186, l: ["Glide across", "横向滑过"] }] },
	{ id: "scene", icon: "app-window", en: "Scene", zh: "场景", tool: "scenes", segs: [{ s: 150, e: 190, l: ["Split screen", "分屏"] }] },
];

const S = {
	variant: "A", tab: "background", sel: null, view: "edit", exStage: "form",
	menu: null, dialog: null, playing: false, t: 48, split: false, display: null, v: {}, shut: {}, shown: new Set(["video", "zoom", "captions", "text", "audio"]),
	zoom: 1, quality: 1, aspect: 0, preset: 0,
	ex: { dest: 0, format: 0, codec: 0, res: 1, fps: 1, quality: 3, optimize: false, adv: false, cursor: false, bpp: false },
};
const val = (c) => S.v[c.k] ?? c.val;
const shut = (g) => S.shut[g.en + g.zh] ?? Boolean(g.shut);
const display = () => S.display ?? "fit";
const percent = (value, min, max) => ((Number(value) - Number(min)) / (Number(max) - Number(min))) * 100;

const row = (c) => {
	if (c.when !== undefined) {
		return (S.v[c.when] ?? false) || (c.when === "cursor" && S.v.cursor === undefined) || (c.when === "curSmooth" && S.v.curSmooth === undefined) ? c.rows.map(row).join("") : "";
	}
	switch (c.t) {
		case "sl": {
			const v = val(c);
			return `<div class="f sl"><label for="r-${c.k}" title="${pick(c.en, c.zh)}">${L(c.en, c.zh)}</label><div class="ctl"><input type="range" id="r-${c.k}" style="--p:${percent(v, c.min, c.max)}%" data-range="${c.k}" data-unit="${c.unit}" min="${c.min}" max="${c.max}" step="${c.step}" value="${v}"><output>${v}${c.unit}</output></div></div>`;
		}
		case "tg":
			return `<div class="f"><span title="${pick(c.en, c.zh)}">${L(c.en, c.zh)}</span><div class="ctl"><button class="switch ${val(c) ? "on" : ""}" data-act="tg" data-k="${c.k}" data-d="${c.val ? 1 : 0}" aria-label="${pick(c.en, c.zh)}"></button></div></div>`;
		case "se": {
			const o = c.opts[val(c)];
			return `<div class="f"><span title="${pick(c.en, c.zh)}">${L(c.en, c.zh)}</span><div class="ctl"><button class="pick" data-act="cycle" data-k="${c.k}" data-n="${c.opts.length}" data-d="${c.val}"><span>${L(o[0], o[1])}</span>${icon("chevron-down")}</button></div></div>`;
		}
		case "sg":
			return `<div class="seg-row">${c.opts.map((o, i) => `<button class="${val(c) === i ? "on" : ""}" data-act="set" data-k="${c.k}" data-v="${i}">${L(o[0], o[1])}</button>`).join("")}</div>`;
		case "ti":
			return `<div class="tiles ${c.four ? "four" : ""}">${c.opts.map((o, i) => `<button class="tile ${val(c) === i ? "on" : ""}" data-act="set" data-k="${c.k}" data-v="${i}">${L(o[0], o[1])}</button>`).join("")}</div>`;
		case "sw":
			return `<div class="f"><span>${L(c.en, c.zh)}</span><div class="ctl"><span class="swatch"><i style="background:${c.color}"></i>${c.color}</span></div></div>`;
		case "nt":
			return `<p class="note">${L(c.en, c.zh)}</p>`;
		case "bt":
			return `<button class="line-btn">${L(c.en, c.zh)}</button>`;
		case "link":
			return `<button class="ib" style="align-self:flex-start">${L(c.en, c.zh)}</button>`;
		case "pd":
			return `<div class="f full"><span>${L(c.en, c.zh)}</span><div class="ctl"><div class="pad">${[0, 1, 2, 3, 4, 5].map((i) => `<button class="${val(c) === i ? "on" : ""}" data-act="set" data-k="${c.k}" data-v="${i}" aria-label="${i}"></button>`).join("")}</div></div></div>`;
		case "input":
			return `<div class="f"><span>${L(c.en, c.zh)}</span><div class="ctl"><div class="input" style="height:28px;width:100%"><input value="${c.ph}" aria-label="${c.en}"></div></div></div>`;
		case "kv":
			return `<div class="f"><span>${L(c.en, c.zh)}</span><div class="ctl tnum">${c.v}</div></div>`;
		case "sync":
			return `<div class="f"><span>${L(c.en, c.zh)}</span><div class="ctl tnum">0 ms <button class="ib">−10</button><button class="ib">+10</button></div></div>`;
		case "stops":
			return `<div class="f full"><span>${L("Colours, 2 to 5 stops", "颜色，2 到 5 个色标")}</span><div class="ctl"><div class="tile" style="width:100%;min-height:20px;padding:0;background:linear-gradient(90deg,#4785ff,#ff4766,#ffd400)"></div></div></div>`;
		case "orbit":
			return `<div class="f full"><span>${L("Drag to orbit", "拖动环绕")}</span><div class="ctl"><div class="tile" style="width:100%;min-height:96px">${icon("box", "lg")}</div></div></div>`;
		case "list":
			return `<div class="listing">${c.items.map((o) => `<button>${icon("music")}${o[0]}</button>`).join("")}</div>`;
		case "swatches": {
			const hues = ["#FFFFFF", "#1F3FD8", "#1E1F24", "#4B7F6B", "#E2725B", "#E0B04C", "#5B4B8A", "#3E7CB1", "#C98FA7", "#2B3A55", "#F2F1EC", "#264D47"];
			const n = Math.min(c.n, 12);
			return `<div class="tiles four">${Array.from({ length: n }, (_, i) => {
				const a = hues[i % hues.length];
				const b = hues[(i + 4) % hues.length];
				return `<button class="tile ${i === (S.v.sw ?? 1) ? "on" : ""}" data-act="swatch" data-a="${a}" data-b="${c.grad ? b : a}" data-i="${i}" style="min-height:34px;padding:3px"><i style="background:${c.grad ? `linear-gradient(135deg,${a},${b})` : a}"></i></button>`;
			}).join("")}</div>`;
		}
		case "bgbody": {
			const keys = ["desktop", "wallpaper", "image", "color", "gradient", "animated"];
			return body(bgBodies[keys[S.v.bgKind ?? 4]]);
		}
		default:
			return "";
	}
};
const body = (rows) => rows.map((r) => (r.rows && r.en ? group(r) : row(r))).join("");
const group = (g) =>
	`<div class="grp ${shut(g) ? "shut" : ""}"><button data-act="fold" data-g="${g.en + g.zh}" data-d="${g.shut ? 1 : 0}">${L(g.en, g.zh)}${icon("chevron-down")}</button>${
		shut(g) ? "" : `<div class="grp-body">${body(g.rows)}${g.sub ? group(g.sub) : ""}</div>`
	}</div>`;

const clipsPanel = () => `<div class="grp-body" style="padding-top:8px">
	<div class="listing">${clipSegs.map((c, i) => `<div class="clip-row"><span class="thumb">${scene}</span><span>${L(c.l[0], c.l[1])}<small class="tnum">${fmt(c.e - c.s)} · ${c.speed}</small></span></div>`).join("")}</div>
	<button class="line-btn">${L("Record a new clip", "录制新片段")}</button>
	<button class="line-btn">${L("Import an existing recording", "导入已有录制")}</button>
	<button class="line-btn">${L("Import MP4 video", "导入 MP4 视频")}</button>
	<p class="note">${L("Drag to reorder. Rename and delete are on each row.", "拖动排序。重命名和删除在每一行上。")}</p>
</div>`;
const panelBody = () => {
	if (S.sel) {
		const def = SEL[S.sel.track] ?? SEL.zoom;
		return def.g().map(group).join("");
	}
	if (S.tab === "clips") return clipsPanel();
	return groups[S.tab]().map(group).join("");
};
const panel = () => {
	const def = S.sel ? SEL[S.sel.track] ?? SEL.zoom : TABS[S.tab];
	const head = S.sel
		? `<b>${L(def.en, def.zh)}</b><button class="ib" data-act="delete-sel" title="${pick("Delete", "删除")}">${icon("trash")}</button><button class="ib" data-act="deselect">${L("Done", "完成")}</button>`
		: `<b>${L(def.en, def.zh)}</b>`;
	return `<div class="panel"><div class="panel-head">${head}</div><div class="panel-body" id="panel-body">${panelBody()}</div></div>`;
};

const titlebar = () => `<div class="titlebar">
	<span class="name">Product demo<em>.ever</em></span>
	<button class="ib" title="${pick("Show in Explorer", "在资源管理器中显示")}">${icon("folder")}</button>
	<button class="ib" data-act="dialog" data-d="delete" title="${pick("Delete recording", "删除录制")}">${icon("trash")}</button>
	<span class="grow"></span>
	<button class="ib" title="${pick("Undo", "撤销")} Ctrl+Z">${icon("undo")}</button>
	<button class="ib" title="${pick("Redo", "重做")} Ctrl+Y">${icon("redo")}</button>
	<span class="sep"></span>
	<button class="ib ${S.menu === "presets" ? "on" : ""}" data-act="menu" data-m="presets">${icon("bookmark")}${L("Presets", "预设")}${icon("chevron-down")}</button>
	<button class="export-btn" data-act="export">${L("Export", "导出")}</button>
	<span class="caption">${glyphs.minimize}</span><span class="caption">${glyphs.maximize}</span><span class="caption">${glyphs.close}</span>
</div>`;

const aspects = [["Auto", "自动"], ["Wide 16:9", "宽屏 16:9"], ["Vertical 9:16", "竖屏 9:16"], ["Square 1:1", "方形 1:1"], ["Classic 4:3", "经典 4:3"], ["Tall 3:4", "高屏 3:4"]];
const toolbar = () => `<div class="toolbar">
	<button class="ib ${S.menu === "aspect" ? "on" : ""}" data-act="menu" data-m="aspect">${icon("proportions")}${L(...aspects[S.aspect])}${icon("chevron-down")}</button>
	<button class="ib" data-act="dialog" data-d="crop">${icon("crop")}${L("Crop", "裁剪")}</button>
	<span class="grow"></span>
	<span class="lbl">${L("Preview", "预览")}</span>
	${`<div class="seg">${[["Full", "完整"], ["Half", "一半"], ["Quarter", "四分之一"]].map((o, i) => `<button class="${S.quality === i ? "on" : ""}" data-act="quality" data-v="${i}">${L(o[0], o[1])}</button>`).join("")}</div>`}
</div>`;
const canvas = () => `<div class="canvas" id="canvas"><div class="screen">${scene}</div><div class="bubble"></div><div class="cap-line">${L("Now open the library", "现在打开资源库")}</div></div>`;
const stage = () => `<div class="stage">${canvas()}</div>`;
const applyPreview = (root = document) => {
	const canvases = root.querySelectorAll(".canvas");
	const bgKind = S.v.bgKind ?? 4;
	const fill = bgKind === 3 ? S.v.c1 ?? "#1F3FD8" : `linear-gradient(135deg,${S.v.c1 ?? "#1F3FD8"},${S.v.c2 ?? "#C98FA7"})`;
	for (const node of canvases) {
		node.style.setProperty("--bg-fill", fill);
		node.style.setProperty("--pad", `${(S.v.padding ?? 10) * 0.9}%`);
		node.style.setProperty("--round", `${(S.v.rounding ?? 8) * 0.5}px`);
		node.style.setProperty("--shadow", (S.v.shadow ?? 74) / 200);
		node.style.setProperty("--cam", `${(S.v.camSize ?? 30) * 0.55}%`);
		const bubble = node.querySelector(".bubble");
		if (bubble) bubble.hidden = Boolean(S.v.camHide);
	}
};

const transport = () => `<div class="transport">
	<span class="clock grow"><b id="clock">${fmt(S.t)}</b> / ${fmt(TOTAL)}</span>
	<span class="mid">
		<button class="ib" data-act="seek" data-v="0" title="${pick("Jump to start", "跳到开头")}">${icon("skip-back")}</button>
		<button class="play" data-act="play" title="Space">${icon(S.playing ? "pause" : "play")}</button>
		<button class="ib" data-act="seek" data-v="${TOTAL}" title="${pick("Jump to end", "跳到结尾")}">${icon("skip-forward")}</button>
	</span>
	<span class="grow"></span>
</div>`;
const tlTools = () => `<div class="tl-tools">
	<button class="ib ${S.split ? "on" : ""}" data-act="split" title="S">${icon("scissors")}${S.split ? L("Done splitting", "完成分割") : L("Split", "分割")}</button>
	<button class="ib" data-act="delete-sel" title="Delete" ${S.sel ? "" : "disabled"}>${icon("trash")}${L("Delete", "删除")}</button>
	<span class="sep"></span>
	<button class="ib ${S.menu === "tracks" ? "on" : ""}" data-act="menu" data-m="tracks">${icon("plus")}${L("Add track", "添加轨道")}</button>
	<span class="grow"></span>
	<button class="ib" data-act="tlzoom" data-v="-1" title="Ctrl −">${icon("zoom-out")}</button>
	<input class="zoomer" type="range" min="1" max="6" step="0.5" value="${S.zoom}" style="--p:${percent(S.zoom, 1, 6)}%" data-range="__zoom" aria-label="${pick("Timeline zoom", "时间轴缩放")}">
	<button class="ib" data-act="tlzoom" data-v="1" title="Ctrl +">${icon("zoom-in")}</button>
</div>`;

const wave = (width = 900) => {
	const bars = [];
	for (let x = 0; x < width; x += 4) {
		const h = 3 + Math.abs(Math.sin(x * 0.037) * Math.cos(x * 0.011)) * 12;
		bars.push(`<rect x="${x}" y="${16 - h}" width="2" height="${h}" rx="1" fill="currentColor"/>`);
	}
	return `<svg class="wave" viewBox="0 0 ${width} 16" preserveAspectRatio="none" aria-hidden="true">${bars.join("")}</svg>`;
};
const laneSegs = (t) =>
	t.segs
		.map((seg, i) => {
			const on = S.sel && S.sel.track === t.id && S.sel.i === i;
			const tiny = (seg.e - seg.s) / TOTAL < 0.03;
			const fill = TRACK_FILL[t.id] ? `background:var(--track-${TRACK_FILL[t.id]});color:var(--white);` : "";
			const clip = t.id === "video";
			const body = clip
				? `<span class="strip"></span><span class="strip-wave">${wave()}</span><b class="chip">${L(seg.l[0], seg.l[1])}</b>`
				: `${t.id === "audio" ? wave() : ""}<b>${seg.l[0] ? L(seg.l[0], seg.l[1]) : ""}</b>`;
			return `<button class="seg-b ${on ? "on" : ""} ${tiny ? "tiny" : ""} ${clip ? "clip" : ""}" data-act="select" data-track="${t.id}" data-i="${i}" style="${fill}left:${(seg.s / TOTAL) * 100}%;width:calc(${((seg.e - seg.s) / TOTAL) * 100}% - 1px)">${body}</button>`;
		})
		.join("") + (t.id === "video" ? `<i class="tr-mark" style="left:${(clipSegs[1].s / TOTAL) * 100}%"></i>` : "");
const TRACK_FILL = { zoom: "zoom", captions: "text", text: "text", audio: "audio", keyboard: "input", style: "style", image: "input", mask: "mask", shot: "shot", scene: "audio" };
const trackList = () => {
	return TRACKS.filter((t) => S.shown.has(t.id));
};
const timeline = (height) => {
	const marks = [];
	for (let s = 0; s <= TOTAL; s += S.zoom >= 3 ? 10 : 30) marks.push(s);
	const rows = trackList()
		.map((t) => `<div class="tl-row ${t.big ? "big" : ""} ${t.id === "audio" ? "mid" : ""}"><span class="gutter">${icon(t.icon)}${L(t.en, t.zh)}</span><div class="lane" data-act="lane">${laneSegs(t)}</div></div>`)
		.join("");
	return `<div class="timeline ${S.split ? "split" : ""}" style="height:${height}px">
		${tlTools()}
		<div class="tl-scroll" id="tl-scroll"><div class="tl-inner" id="tl-inner" style="width:${S.zoom * 100}%">
			<div class="tl-row ruler"><span class="gutter"></span><div class="lane" data-act="lane">${marks.map((s) => `<span class="tick" style="left:${(s / TOTAL) * 100}%">${fmt(s)}</span>`).join("")}</div></div>
			${rows}
			<i class="playhead" id="playhead"></i>
		</div></div>
	</div>`;
};
const placePlayhead = () => {
	const inner = document.getElementById("tl-inner");
	const head = document.getElementById("playhead");
	if (!inner || !head) return;
	head.style.left = `calc(96px + (100% - 108px) * ${S.t / TOTAL})`;
	const clock = document.getElementById("clock");
	if (clock) clock.textContent = fmt(S.t);
};

const rail = () => `<div class="rail left icons">${["background", "screen", "camera", "audio", "cursor", "keyboard", "captions", "|", "clips"]
	.map((id) => {
		if (id === "|") return '<span class="hr"></span>';
		const tab = TABS[id];
		return `<button class="${!S.sel && S.tab === id ? "on" : ""}" data-act="tab" data-v="${id}" title="${pick(tab.en, tab.zh)}" aria-label="${pick(tab.en, tab.zh)}">${icon(tab.icon)}</button>`;
	})
	.join("")}</div>`;
const layout = () => `<div class="ed-body"><div class="col">
	<div class="row-top">
		<div class="player">${toolbar()}${stage()}${transport()}</div>
		${rail()}${panel()}
	</div>
	<div class="grip"></div>${timeline(260)}
</div></div>`;

const optGroup = (key, titleEn, titleZh, opts, disabled = []) => `<h5>${L(titleEn, titleZh)}</h5><div class="opts">${opts
	.map((o, i) => (disabled.includes(i) ? "" : `<button class="${S.ex[key] === i ? "on" : ""}" data-act="ex" data-k="${key}" data-v="${i}">${L(o[0], o[1])}</button>`))
	.join("")}</div>`;
const exportForm = () => {
	const gif = S.ex.format === 1;
	return `<div class="ex-form">
		${optGroup("dest", "Destination", "保存到", [["File", "文件"], ["Clipboard", "剪贴板"]])}
		${optGroup("format", "Format", "格式", [["MP4", "MP4"], ["GIF", "GIF"]])}
		${gif ? "" : optGroup("codec", "Video codec", "视频编码", [["H.264", "H.264"], ["HEVC", "HEVC"], ["AV1", "AV1"]])}
		${gif ? "" : `<p class="note">${S.ex.codec === 0 ? L("Plays almost everywhere.", "几乎到处都能播放。") : S.ex.codec === 1 ? L("Smaller files. Most recent devices play it.", "文件更小，较新的设备都能播放。") : L("Smallest files. Takes longer to encode.", "文件最小，编码更慢。")}</p>`}
		${optGroup("res", "Resolution", "分辨率", [["720p", "720p"], ["1080p", "1080p"], ["4K", "4K"]], gif ? [2] : [])}
		${optGroup("fps", "Frame rate", "帧率", gif ? [["10", "10"], ["15", "15"], ["20", "20"], ["25", "25"], ["30", "30"]] : [["15", "15"], ["30", "30"], ["60", "60"]])}
		${optGroup("quality", "Quality", "画质", [["Potato", "土豆"], ["Web", "网页"], ["Social", "社交"], ["Maximum", "最高"]])}
		<div class="row-sw"><span>${L("Optimize file size", "优化文件大小")}</span><button class="switch ${S.ex.optimize ? "on" : ""}" data-act="ext" data-k="optimize" aria-label="optimize"></button></div>
		<div class="grp ${S.ex.adv ? "" : "shut"}" style="margin:6px -14px 0;border-bottom:0;border-top:1px solid var(--border)"><button data-act="ext" data-k="adv">${L("Advanced", "高级")}${icon("chevron-down")}</button>
		${S.ex.adv ? `<div class="grp-body"><div class="row-sw"><span>${L("Export cursor only", "仅导出光标")}</span><button class="switch ${S.ex.cursor ? "on" : ""}" data-act="ext" data-k="cursor" aria-label="cursor"></button></div>${row(sl("bpp", "Bits per pixel", "每像素比特数", 0.02, 0.5, 0.15, "", 0.01))}</div>` : ""}</div>
	</div>`;
};
const stats = (two) => {
	const size = [24, 64, 310][S.ex.res] * [0.5, 1, 1.6][S.ex.fps % 3];
	return `<div class="stats ${two ? "two" : ""}">
		<div><small>${L("Duration", "时长")}</small><b>${fmt(TOTAL)}</b></div>
		<div><small>${L("Output", "输出")}</small><b>${["1280 × 720", "1920 × 1080", "3840 × 2160"][S.ex.res]}</b></div>
		<div><small>${L("Estimated size", "预计大小")}</small><b>${Math.round(size)} MB</b></div>
		<div><small>${L("Export time", "导出时间")}</small><b>${["0:40", "1:12", "3:05"][S.ex.res]}</b></div>
	</div>`;
};
const exportStateBody = () => {
	switch (S.exStage) {
		case "progress":
			return `<div class="ex-state"><h4>${L("Rendering video", "正在渲染视频")}</h4><div class="progress"><i id="ex-bar" style="width:${S.exPct ?? 34}%"></i></div><p class="tnum">3,204 / 13,320 ${L("frames", "帧")}</p><div class="acts"><button class="ex-sec" data-act="exst" data-v="cancel">${L("Cancel", "取消")}</button></div></div>`;
		case "cancel":
			return `<div class="ex-state"><h4>${L("Cancel export?", "取消导出？")}</h4><p>${L("The frames rendered so far are discarded.", "已渲染的帧会被丢弃。")}</p><div class="acts"><button class="ex-sec" data-act="exst" data-v="progress">${L("Keep exporting", "继续导出")}</button><button class="btn danger" data-act="exst" data-v="form">${L("Cancel export", "取消导出")}</button></div></div>`;
		case "done":
			return `<div class="ex-state"><span class="done-bar"></span><h4>${L("Export complete", "导出完成")}</h4><p>Product demo.mp4 · 64 MB</p><div class="acts"><button class="ex-sec" data-act="close-export">${L("Back to editor", "返回编辑器")}</button><button class="ex-cta" style="flex:none;padding:0 14px">${L("Open file", "打开文件")}</button></div></div>`;
		case "failed":
			return `<div class="ex-state">${icon("triangle-alert", "big bad")}<h4>${L("Couldn't finish the export", "导出没有完成")}</h4><p>${L("The disk is full. Free up some space, then export again.", "磁盘已满。腾出一些空间后再导出。")}</p><div class="acts"><button class="ex-sec">${L("Details", "详情")}</button><button class="ex-cta" style="flex:none;padding:0 14px" data-act="exst" data-v="form">${L("Back to export", "返回导出")}</button></div></div>`;
		default:
			return "";
	}
};
const cta = () => `<div class="ex-foot"><button class="ex-cta" data-act="exst" data-v="progress">${S.ex.dest === 0 ? L("Export to file", "导出到文件") : L("Export to clipboard", "导出到剪贴板")}</button></div>`;
const exportPage = () => `<div class="ex-page">
	<div class="titlebar"><button class="ib" data-act="close-export">${icon("chevron-left")}${L("Back to editor", "返回编辑器")}</button><b style="font-size:var(--text-body);font-weight:600;margin-left:6px">${L("Export", "导出")}</b><span class="grow"></span><span class="caption">${glyphs.minimize}</span><span class="caption">${glyphs.maximize}</span><span class="caption">${glyphs.close}</span></div>
	<div class="ex-body"><div class="ex-left"><div class="canvas-wrap">${canvas()}</div>${stats(false)}</div>
	<div class="ex-side">${S.exStage === "form" ? exportForm() + cta() : exportStateBody()}</div></div>
</div>`;
const menus = {
	aspect: () => `<div class="pop menu" style="left:8px;top:84px;width:200px">${aspects.map((a, i) => `<button data-act="aspect" data-v="${i}"><span class="grow">${L(a[0], a[1])}</span>${S.aspect === i ? icon("check") : ""}</button>`).join("")}</div>`,
	presets: () => `<div class="pop menu" style="right:110px;top:44px;width:288px"><button data-act="close"><span class="grow">${L("Default", "默认")}</span>${icon("check")}</button><button data-act="close"><span class="grow">${L("Tutorial look", "教程风格")}</span>${icon("settings")}</button><hr><button data-act="close">${icon("plus")}${L("Create new preset", "新建预设")}</button></div>`,
	tracks: () => `<div class="pop menu" style="right:96px;bottom:264px"><div class="tracks">${TRACKS.filter((t) => t.id !== "video" && t.id !== "zoom")
		.map((t) => `<button class="${S.shown.has(t.id) ? "on" : ""}" data-act="toggle-track" data-v="${t.id}">${icon(t.icon)}${L(t.en, t.zh)}</button>`)
		.join("")}</div></div>`,
	clip: () => {
		const c = clipSegs[S.menuClip ?? 0];
		return `<div class="pop menu" style="left:300px;bottom:150px;width:300px">
			<div class="t">${L(c.l[0], c.l[1])} · ${fmt(c.e - c.s)}</div>
			<div class="t">${L("Speed", "速度")}</div><div class="speed">${["0.25x", "0.5x", "1x", "1.5x", "2x", "4x", "8x"].map((s) => `<button class="${c.speed === s ? "on" : ""}" data-act="speed" data-v="${s}">${s}</button>`).join("")}</div>
			<div class="t">${L("Audio", "音频")}</div><div class="box">${row(sl("clipVol", "Volume", "音量", 0, 200, 100, "%"))}<div class="seg-row">${[["Mute", "静音"], ["Keep pitch", "保持音调"], ["Match speed", "跟随速度"]].map((o, i) => `<button class="${(S.v.clipAud ?? 1) === i ? "on" : ""}" data-act="set" data-k="clipAud" data-v="${i}">${L(o[0], o[1])}</button>`).join("")}</div></div>
			<div class="t">${L("Transition from previous clip", "与上一片段的转场")}</div><div class="box"><div class="seg-row">${[["None", "无"], ["Cross fade", "交叉淡化"], ["Fade through black", "黑场过渡"]].map((o, i) => `<button class="${(S.v.clipTr ?? 1) === i ? "on" : ""}" data-act="set" data-k="clipTr" data-v="${i}">${L(o[0], o[1])}</button>`).join("")}</div>${row(sl("clipTrD", "Duration", "时长", 0.1, 2, 0.5, " s", 0.05))}</div>
			<div class="t">${L("Cursor", "光标")}</div><div class="box">${row(tg("clipCur", "Hide cursor", "隐藏光标"))}</div><hr>
			<button data-act="close">${icon("scissors")}<span class="grow">${L("Split at playhead", "在播放头处分割")}</span><span class="kbd">C</span></button>
			<button data-act="close">${L("Merge with previous clip", "与上一片段合并")}</button>
			<button data-act="close">${L("Merge with next clip", "与下一片段合并")}</button>
			<button data-act="close">${icon("pencil")}${L("Rename clip", "重命名片段")}</button>
			<button class="danger" data-act="close">${icon("trash")}<span class="grow">${L("Delete clip", "删除片段")}</span><span class="kbd">Del</span></button>
		</div>`;
	},
};
const dialogs = {
	delete: () => `<div class="scrim"><div class="dialog"><h3>${L("Delete recording?", "删除录制？")}</h3><p>${L("It goes to the Recycle Bin. Exports stay where they are.", "它会进入回收站，导出的文件不受影响。")}</p><div class="actions"><button class="btn" data-act="close">${L("Keep", "保留")}</button><button class="btn danger" data-act="close">${L("Delete recording", "删除录制")}</button></div></div></div>`,
	crop: () => `<div class="scrim"><div class="dialog" style="width:560px"><h3>${L("Crop", "裁剪")}</h3><div class="thumb">${scene}</div><div class="crop-stage">${["Free", "1:1", "2:1", "3:2", "4:3", "9:16", "16:9", "16:10", "21:9"].map((r, i) => `<span class="tag ${i === 0 ? "on" : ""}">${r === "Free" ? L("Free", "自由") : r}</span>`).join("")}</div><div class="actions"><button class="btn" data-act="close">${L("Cancel", "取消")}</button><button class="btn primary" data-act="close">${L("Save", "保存")}</button></div></div></div>`,
};

const renderViewRow = () => {
	const stateOpts = [["form", "Options", "选项"], ["progress", "Progress", "进度"], ["cancel", "Cancel", "取消"], ["done", "Done", "完成"], ["failed", "Failed", "失败"]];
	document.getElementById("view-row").innerHTML = `
		<div class="seg">${[["fit", "Whole window", "整窗"], ["stacked", "Phone detail", "手机细看"]].map(([v, en, zh]) => `<button class="${display() === v ? "on" : ""}" data-pact="display" data-v="${v}">${L(en, zh)}</button>`).join("")}</div>
		${S.view === "export" ? `<div class="seg">${stateOpts.map(([v, en, zh]) => `<button class="${S.exStage === v ? "on" : ""}" data-pact="exst" data-v="${v}">${L(en, zh)}</button>`).join("")}</div>` : ""}`;
};
const fit = () => {
	const wrap = document.getElementById("ed-fit");
	const ed = document.getElementById("ed");
	const stageEl = document.getElementById("stage");
	if (!wrap || !ed || display() === "stacked") return;
	const scale = Math.min(1, stageEl.clientWidth / 1300);
	ed.style.transform = `scale(${scale})`;
	ed.style.transformOrigin = "top left";
	wrap.style.width = `${1275 * scale}px`;
	wrap.style.height = `${800 * scale}px`;
};
const render = () => {
	const prior = document.getElementById("panel-body")?.scrollTop ?? 0;
	const priorTl = document.getElementById("tl-scroll")?.scrollTop ?? 0;
	const stacked = display() === "stacked";
	document.getElementById("stage").innerHTML = `<div class="ed-fit" id="ed-fit"><div class="ed ${stacked ? "stacked" : ""}" id="ed">
		${titlebar()}${layout()}
		${S.view === "export" ? exportPage() : ""}
		${S.menu ? menus[S.menu]() : ""}
		${S.dialog ? dialogs[S.dialog]() : ""}
	</div></div>`;
	const panelNode = document.getElementById("panel-body");
	if (panelNode) panelNode.scrollTop = prior;
	const tl = document.getElementById("tl-scroll");
	if (tl) tl.scrollTop = priorTl;
	applyPreview();
	placePlayhead();
	renderViewRow();
	fit();
	const root = document.documentElement;
	root.dataset.lang = FE.state.lang;
};

const act = (el, event) => {
	const a = el.dataset.act;
	const d = el.dataset;
	switch (a) {
		case "tab":
			S.tab = d.v;
			S.sel = null;
			S.view = "edit";
			break;
		case "select": {
			if (S.split) return;
			if (d.track === "video") {
				S.sel = { track: "video", i: Number(d.i) };
				S.menu = "clip";
				S.menuClip = Number(d.i);
				break;
			}
			S.sel = { track: d.track, i: Number(d.i) };
			S.menu = null;
			event.stopPropagation();
			break;
		}
		case "deselect":
		case "delete-sel":
			S.sel = null;
			break;
		case "lane":
			S.sel = null;
			S.menu = null;
			break;
		case "tg": {
			const cur = S.v[d.k] ?? d.d === "1";
			S.v[d.k] = !cur;
			break;
		}
		case "cycle": {
			const cur = S.v[d.k] ?? Number(d.d);
			S.v[d.k] = (cur + 1) % Number(d.n);
			break;
		}
		case "set":
			S.v[d.k] = Number(d.v);
			break;
		case "swatch":
			S.v.c1 = d.a;
			S.v.c2 = d.b;
			S.v.sw = Number(d.i);
			break;
		case "fold": {
			const cur = S.shut[d.g] ?? d.d === "1";
			S.shut[d.g] = !cur;
			break;
		}
		case "menu":
			S.menu = S.menu === d.m ? null : d.m;
			break;
		case "aspect":
			S.aspect = Number(d.v);
			S.menu = null;
			break;
		case "quality":
			S.quality = Number(d.v);
			break;
		case "close":
			S.menu = null;
			S.dialog = null;
			break;
		case "dialog":
			S.dialog = d.d;
			break;
		case "toggle-track":
			if (S.shown.has(d.v)) S.shown.delete(d.v);
			else S.shown.add(d.v);
			break;
		case "speed":
			clipSegs[S.menuClip].speed = d.v;
			break;
		case "seek":
			S.t = Number(d.v);
			S.playing = false;
			break;
		case "play":
			S.playing = !S.playing;
			if (S.playing) tick();
			break;
		case "split":
			S.split = !S.split;
			break;
		case "tlzoom":
			S.zoom = Math.min(6, Math.max(1, S.zoom + Number(d.v)));
			break;
		case "export":
			S.view = "export";
			S.exStage = "form";
			S.menu = null;
			S.sel = null;
			break;
		case "close-export":
			S.view = "edit";
			S.exStage = "form";
			break;
		case "ex":
			S.ex[d.k] = Number(d.v);
			break;
		case "ext":
			S.ex[d.k] = !S.ex[d.k];
			break;
		case "exst":
			setStage(d.v);
			break;
		default:
			return;
	}
	render();
};
let exTimer = null;
const setStage = (stage) => {
	clearInterval(exTimer);
	S.exStage = stage;
	if (stage === "progress") {
		S.exPct = S.exPct && S.exPct < 100 ? S.exPct : 8;
		exTimer = setInterval(() => {
			S.exPct += 6;
			const bar = document.getElementById("ex-bar");
			if (bar) bar.style.width = `${S.exPct}%`;
			if (S.exPct >= 100) {
				clearInterval(exTimer);
				S.exPct = 0;
				S.exStage = "done";
				render();
			}
		}, 260);
	}
};
let last = 0;
const tick = (now) => {
	if (!S.playing) return;
	if (now && last) {
		S.t += (now - last) / 1000;
		if (S.t >= TOTAL) {
			S.t = 0;
		}
		placePlayhead();
	}
	last = now ?? 0;
	requestAnimationFrame(tick);
};
const pact = (el) => {
	const d = el.dataset;
	if (d.pact === "display") S.display = d.v;
	if (d.pact === "exst") setStage(d.v);
	render();
};

document.addEventListener("click", (event) => {
	const p = event.target.closest("[data-pact]");
	if (p) return pact(p);
	const el = event.target.closest("[data-act]");
	if (el) return act(el, event);
	if (S.menu && !event.target.closest(".pop")) {
		S.menu = null;
		render();
	}
});
document.addEventListener("input", (event) => {
	const el = event.target;
	const key = el.dataset?.range;
	if (!key) return;
	const value = Number(el.value);
	if (key === "__zoom") {
		S.zoom = value;
		el.style.setProperty("--p", `${percent(value, 1, 6)}%`);
		document.getElementById("tl-inner").style.width = `${value * 100}%`;
		return;
	}
	S.v[key] = value;
	el.style.setProperty("--p", `${percent(el.value, el.min, el.max)}%`);
	el.nextElementSibling.textContent = `${value}${el.dataset.unit}`;
	applyPreview();
});
document.addEventListener("keydown", (event) => {
	if (event.target.closest?.("input,textarea,[contenteditable]")) return;
	if (event.key === " ") {
		event.preventDefault();
		S.playing = !S.playing;
		if (S.playing) tick();
		render();
	}
	if (event.key === "s") {
		S.split = !S.split;
		render();
	}
	if (event.key === "Escape") {
		if (S.dialog || S.menu) {
			S.dialog = null;
			S.menu = null;
		} else if (S.split) S.split = false;
		else S.sel = null;
		render();
	}
});
window.addEventListener("resize", fit);

const table = (id, cls, rows) => {
	document.getElementById(id).innerHTML = `<table class="${cls}"><tbody>${rows.map((r) => `<tr>${r.map((c, i) => `<td class="${i ? "" : "dim"}">${Array.isArray(c) ? L(c[0], c[1]) : c}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
};
const renderDocs = () => {
	const heads = {
		"t-live": ["Editor", "编辑器"],
		"l-live": ["The chosen layout: preview and inspector on top, a full-width timeline below, export on its own page. Press a segment to swap the inspector, a video clip for its menu, Export for the export page.", "选定的布局：上面是预览和检查器，下面是全宽时间轴，导出是整页。按段落切换检查器，按视频片段打开菜单，按“导出”进入导出页。"],
		"t-layout": ["Layout", "布局"],
		"l-layout": ["Sizes at the default 1275 × 800 window.", "默认 1275 × 800 窗口下的尺寸。"],
		"t-timeline": ["Timeline", "时间轴"],
		"l-timeline": ["Modelled on 剪映 and OpenCut: tools above the tracks, height by importance, a colour per segment type. The playhead stays white.", "参考剪映和 OpenCut：工具在轨道上方，高度按重要程度分配，每类段落一种颜色。播放头保持白色。"],
		"t-states": ["Other states", "其他状态"],
		"l-states": ["What the window shows outside normal editing.", "正常编辑之外，窗口显示什么。"],
		"t-inv": ["Where everything went", "每项功能的位置"],
		"l-inv": ["Every current capability keeps a place. Implementation tickets check against this list.", "现有功能都有位置，实现工单按这张表核对。"],
	};
	for (const [id, text] of Object.entries(heads)) document.getElementById(id).innerHTML = L(text[0], text[1]);
	table("layout-table", "doc", [
		[["Title bar", "标题栏"], ["44 high: name, show in Explorer, delete, undo, redo, presets, Export, window controls", "44 高：名称、在资源管理器中显示、删除、撤销、重做、预设、导出、窗口按钮"]],
		[["Preview toolbar", "预览工具栏"], ["40 high: aspect ratio, crop, preview quality", "40 高：宽高比、裁剪、预览画质"]],
		[["Preview", "预览"], ["Flexible, about 674 × 379 at 16:9", "弹性，16:9 时约 674 × 379"]],
		[["Transport", "播放条"], ["44 high, under the preview and centred on it: clock, jump to start, play, jump to end", "44 高，在预览下方并与预览居中：时间码、跳到开头、播放、跳到结尾"]],
		[["Inspector", "检查器"], ["46 icon rail plus a 340 panel, about 445 of content height", "46 宽图标栏加 340 宽面板，内容高度约 445"]],
		[["Timeline", "时间轴"], ["Full width, 260 by default, resized by the grip above it", "全宽，默认 260 高，用上方的手柄调整"]],
	]);
	table("timeline-table", "doc", [
		[["Toolbar", "工具栏"], ["36 high: split, delete, add track, then timeline zoom on the right", "36 高：分割、删除、添加轨道，右侧是时间轴缩放"]],
		[["Video track", "视频轨"], ["58 high: filmstrip above an 18 px waveform, name chip top left, diamond at a transition", "58 高：上方画面条，下方 18 px 波形，左上角名称，转场处有菱形标记"]],
		[["Audio track", "音频轨"], ["44 high with a waveform", "44 高，带波形"]],
		[["Thin tracks", "细轨道"], ["26 high: zoom, captions, text, keyboard, style, image, mask, 3D, scene", "26 高：缩放、字幕、文字、按键、样式、图片、遮罩、3D、场景"]],
		[["Segment colours", "段落颜色"], ["The track_* tokens, with white text at 4.5:1 or better", "track_* token，白字对比度不低于 4.5:1"]],
		[["Selected", "选中"], ["2 px foreground edge with a trim bar at each end", "2 px 前景色描边，两端有修剪手柄"]],
		[["Gutter", "轨道名"], ["96 wide: icon and track name", "96 宽：图标和轨道名"]],
	]);
	const states = [
		[["Loading", "加载中"], ["Skeletons for preview, inspector and timeline. Input is blocked; closing still works.", "预览、检查器和时间轴显示骨架。输入被阻止，仍可关闭。"]],
		[["Preparing", "准备中"], ["A veil covers the timeline past the playable frontier. Playback and seeking work once ready.", "可播放范围之后的时间轴盖一层遮罩，就绪后可以播放和拖动。"]],
		[["Couldn't open", "打不开"], ["\"Couldn't open this recording. Its project files may be damaged.\" with Details and Export logs.", "“打不开这个录制，项目文件可能已损坏。”并提供详情和导出日志。"]],
		[["Saving", "保存"], ["Autosave, no indicator. Closing flushes first; a failed flush keeps the window open and says the recording is retained.", "自动保存，不显示状态。关闭前先写入；写入失败时窗口保持打开，并提示录制已保留。"]],
		[["Empty tracks", "空轨道"], ["Each lane says how to add its first segment. Zoom offers Add zooms automatically; 3D offers Auto scene. No clips yet draws TICK.", "每条轨道说明如何添加第一段。缩放提供自动添加，3D 提供自动场景。“还没有片段”画 TICK。"]],
		[["Export finished", "导出完成"], ["Blue completion bar with ALL_SET, Open file and Back to editor.", "蓝色完成横条加 ALL_SET，打开文件和返回编辑器。"]],
	];
	document.getElementById("states-grid").innerHTML = states.map(([h, b]) => `<div class="card"><h3>${L(h[0], h[1])}</h3><p>${L(b[0], b[1])}</p></div>`).join("");
	table("inv", "doc", [
		[["Source: desktop, wallpaper, image, color, gradient, animated", "来源：桌面、壁纸、图片、纯色、渐变、动态"], ["Background", "背景"]],
		[["Background blur", "背景模糊"], ["Background", "背景"]],
		[["Frame style, theme, URL or window title", "外框样式、主题、网址或窗口标题"], ["Screen (was a preview toolbar popover)", "画面（原在预览工具栏弹窗）"]],
		[["Padding, rounding, corner style, motion blur, shadow, border, notch, color correction", "内边距、圆角、圆角样式、运动模糊、阴影、描边、刘海、调色"], ["Screen (was Background)", "画面（原在背景）"]],
		[["Camera, audio, cursor, keyboard, captions settings", "摄像头、音频、光标、按键、字幕设置"], ["Their own tabs, unchanged", "各自页面，不变"]],
		[["Clips: list, reorder, rename, delete, record a new clip, import", "片段：列表、排序、重命名、删除、录制新片段、导入"], ["Clips tab (was a header toggle)", "片段页（原在标题栏开关）"]],
		[["Zoom, text, caption, keyboard, audio, mask, scene, 3D shot, image, style segments", "缩放、文字、字幕、按键、音频、遮罩、场景、3D 镜头、图片、样式段落"], ["Inspector when selected", "选中后在检查器"]],
		[["Audio library", "音频库"], ["Inspector, from an audio segment", "检查器，从音频段落进入"]],
		[["Clip speed, volume, mute, pitch, hide cursor, split, merge, rename, delete", "片段速度、音量、静音、音调、隐藏光标、分割、合并、重命名、删除"], ["Clip menu", "片段菜单"]],
		[["Clip transitions: add, change, remove", "片段转场：添加、更改、删除"], ["Clip menu, ported from upstream", "片段菜单，从上游移植"]],
		[["Aspect ratio, crop, preview quality", "宽高比、裁剪、预览画质"], ["Preview toolbar", "预览工具栏"]],
		[["Play, jump to start or end, clock", "播放、跳到首尾、时间码"], ["Transport; Home and End ported from upstream", "播放条；Home 和 End 从上游移植"]],
		[["Split, delete, add track, timeline zoom", "分割、删除、添加轨道、时间轴缩放"], ["Timeline toolbar (was the transport)", "时间轴工具栏（原在播放条）"]],
		[["Canvas boxes, snapping guides, lock messages, arrow-key nudging", "画布选框、吸附参考线、锁定提示、方向键微调"], ["Preview, unchanged", "预览，不变"]],
		[["Trim, move, range select, gutter reorder, ruler scrub, minimap, snapping, follow playback", "修剪、移动、范围选择、轨道拖动排序、标尺拖动、小地图、吸附、跟随播放"], ["Timeline, unchanged", "时间轴，不变"]],
		[["Presets", "预设"], ["Title bar", "标题栏"]],
		[["Export options, estimates, progress, cancel, finished, failed", "导出选项、估算、进度、取消、完成、失败"], ["Export page", "导出页"]],
		[["Turbo export", "Turbo 导出"], ["macOS only upstream; added with the macOS work", "上游仅 macOS，随 macOS 工作加入"]],
		[["Disabled Captions pill in the header", "标题栏禁用的字幕胶囊"], ["Removed: it never did anything", "移除：它没有任何作用"]],
	]);
};
shell({ here: "editor", depth: 1, sections: [["live", "Editor", "编辑器"], ["layout", "Layout", "布局"], ["timeline", "Timeline", "时间轴"], ["states", "Other states", "其他状态"], ["inventory", "Where everything went", "功能位置"]] });
document.documentElement.style.setProperty("--filmstrip", `url("data:image/svg+xml,${encodeURIComponent(scene)}")`);
renderDocs();
onChange(() => {
	renderDocs();
	render();
});
render();
