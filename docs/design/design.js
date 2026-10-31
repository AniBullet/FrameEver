window.FE = (() => {
	const tokens = window.FRAMEEVER_TOKENS;
	const icons = window.FRAMEEVER_ICONS;
	const defaults = { theme: "dark", lang: "zh", scale: "100" };
	const choices = { theme: ["dark", "light"], lang: ["en", "zh"], scale: ["100", "150", "200"] };
	const surfaces = [
		["main-window-library", "Main window and library", "主窗口和资源库", true],
		["settings", "Settings", "设置", true],
		["editor", "Editor", "编辑器", true],
		["screenshot-editor", "Screenshot editor", "截图编辑器", true],
		["recording-controls", "Recording controls", "录制控制条", true],
		["area-picker", "Area picker and countdown", "区域选择和倒计时", true],
		["tray", "Tray", "托盘", true],
		["camera-teleprompter", "Camera and teleprompter", "摄像头和提词器", true],
		["converter", "FrameEver Converter", "FrameEver Converter", false],
		["installer", "Installer", "安装器", true],
	];
	const state = { ...defaults, at: "" };

	const L = (en, zh) => `<span lang="en">${en}</span><span lang="zh">${zh}</span>`;
	const icon = (name, cls = "") => {
		if (!icons[name]) throw new Error(`No icon: ${name}`);
		return `<svg class="i ${cls}" viewBox="0 0 24 24" aria-hidden="true">${icons[name]}</svg>`;
	};
	const recordGlyph = () => '<span class="rec-glyph" aria-hidden="true"></span>';
	const pick = (en, zh) => (state.lang === "en" ? en : zh);

	const readHash = () => {
		const params = new URLSearchParams(location.hash.slice(1));
		for (const key of Object.keys(defaults)) {
			const value = params.get(key);
			state[key] = choices[key].includes(value) ? value : defaults[key];
		}
		state.at = params.get("at") ?? "";
	};
	const hashFor = (next = {}) => {
		const merged = { ...state, ...next };
		const params = new URLSearchParams();
		for (const key of Object.keys(defaults)) {
			if (merged[key] !== defaults[key]) params.set(key, merged[key]);
		}
		if (merged.at) params.set("at", merged.at);
		const text = params.toString();
		return text ? `#${text}` : "";
	};
	const write = (next) => {
		Object.assign(state, next);
		history.replaceState(null, "", hashFor() || location.pathname);
		apply();
	};
	const listeners = [];
	const apply = () => {
		const root = document.documentElement;
		root.dataset.theme = state.theme;
		root.dataset.lang = state.lang;
		root.lang = state.lang === "zh" ? "zh-CN" : "en";
		const main = document.querySelector("main");
		if (main) {
			main.style.zoom = Number(state.scale) / 100;
			main.style.setProperty("--zoom", Number(state.scale) / 100);
		}
		for (const button of document.querySelectorAll("[data-set]")) {
			const [key, value] = button.dataset.set.split(":");
			button.classList.toggle("on", state[key] === value);
		}
		for (const link of document.querySelectorAll("a[data-page]")) {
			link.href = `${link.dataset.page}${hashFor({ at: "" })}`;
		}
		for (const input of document.querySelectorAll("[data-ph-en]")) {
			input.placeholder = pick(input.dataset.phEn, input.dataset.phZh);
		}
		for (const listener of listeners) listener(state);
	};
	const seg = (key, options) =>
		`<div class="seg">${options.map(([value, label]) => `<button data-set="${key}:${value}">${label}</button>`).join("")}</div>`;

	const shell = ({ here, depth, sections = [] }) => {
		const prefix = "../".repeat(depth);
		const pages = [
			[`${prefix}index.html`, "index", "Tokens and components", "Token 和组件", true],
			...surfaces.map(([slug, en, zh, ready]) => [`${prefix}surfaces/${slug}.html`, slug, en, zh, ready]),
		];
		const nav = pages
			.map(([href, slug, en, zh, ready]) =>
				ready
					? `<a data-page="${href}" class="${slug === here ? "on" : ""}">${L(en, zh)}</a>`
					: `<span class="soon">${L(en, zh)}</span>`,
			)
			.join("");
		const toc = sections
			.map(([id, en, zh]) => `<a href="#" data-at="${id}">${L(en, zh)}</a>`)
			.join("");
		document.getElementById("shell").innerHTML = `
			<div class="page-bar">
				<h1>FrameEver ${L("design reference", "设计参考")}</h1>
				<span class="spacer"></span>
				${seg("theme", [["dark", L("Dark", "深色")], ["light", L("Light", "浅色")]])}
				${seg("lang", [["en", "English"], ["zh", "简体中文"]])}
				${seg("scale", [["100", "100%"], ["150", "150%"], ["200", "200%"]])}
			</div>
			<nav class="page-nav">${nav}</nav>
			${toc ? `<nav class="page-nav">${toc}</nav>` : ""}
			<div class="font-note" id="font-note">${L(
				"HarmonyOS Sans isn't on this machine, so Chinese falls back to a system font. Run pnpm fe setup.",
				"这台电脑上没有 HarmonyOS Sans，中文显示的是系统字体。运行 pnpm fe setup。",
			)}</div>`;
		document.addEventListener("click", (event) => {
			const set = event.target.closest("[data-set]");
			if (set) {
				const [key, value] = set.dataset.set.split(":");
				write({ [key]: value });
				return;
			}
			const at = event.target.closest("[data-at]");
			if (at) {
				event.preventDefault();
				write({ at: at.dataset.at });
				document.getElementById(at.dataset.at)?.scrollIntoView({ behavior: "auto" });
			}
		});
		window.addEventListener("hashchange", () => {
			readHash();
			apply();
		});
		document.fonts
			.load('16px "HarmonyOS Sans SC"', "帧刻")
			.then((faces) => {
				if (faces.length === 0) document.getElementById("font-note").classList.add("show");
			})
			.catch(() => document.getElementById("font-note").classList.add("show"));
		readHash();
		apply();
		if (state.at) document.getElementById(state.at)?.scrollIntoView({ behavior: "auto" });
	};

	const toLinear = ([l, c, h]) => {
		const rad = (h * Math.PI) / 180;
		const a = c * Math.cos(rad);
		const b = c * Math.sin(rad);
		const m1 = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3;
		const m2 = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3;
		const m3 = (l - 0.0894841775 * a - 1.291485548 * b) ** 3;
		return [
			4.0767416621 * m1 - 3.3077115913 * m2 + 0.2309699292 * m3,
			-1.2684380046 * m1 + 2.6097574011 * m2 - 0.3413193965 * m3,
			-0.0041960863 * m1 - 0.7034186147 * m2 + 1.707614701 * m3,
		];
	};
	const unit = (value) => Math.min(1, Math.max(0, value));
	const luminanceOf = (rgb) => 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
	const hex = (value) => {
		const gamma = (x) => {
			const y = unit(x);
			return Math.round((y <= 0.0031308 ? 12.92 * y : 1.055 * y ** (1 / 2.4) - 0.055) * 255);
		};
		return `#${toLinear(value)
			.map(gamma)
			.map((n) => n.toString(16).padStart(2, "0"))
			.join("")
			.toUpperCase()}`;
	};
	const oklch = (value) =>
		`oklch(${value[0]} ${value[1]} ${value[2]}${value[3] === undefined ? "" : ` / ${value[3]}`})`;
	const brandValue = (theme, ref) => {
		const [family, step] = ref.split(".");
		const steps = tokens.brand[theme][family];
		return Array.isArray(steps) ? steps[Number(step) - 1] : steps[step];
	};
	const value = (theme, name) => {
		if (name.includes(".")) return brandValue(theme, name);
		const token = [...tokens.semantic, ...tokens.componentVars].find((entry) => entry.name === name);
		return value(theme, token[theme]);
	};
	const color = (theme, name) => oklch(value(theme, name));
	const luminance = (theme, name) => {
		const fg = value(theme, name);
		const rgb = toLinear(fg).map(unit);
		if (fg[3] === undefined) return luminanceOf(rgb);
		const under = toLinear(value(theme, name === "fixed.badge" ? "preview" : "background")).map(unit);
		return luminanceOf(rgb.map((x, i) => x * fg[3] + under[i] * (1 - fg[3])));
	};
	const contrast = (theme, a, b) => {
		const x = luminance(theme, a);
		const y = luminance(theme, b);
		return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
	};
	const spring = (t) => {
		const { zeta, omega } = tokens.motion.move;
		const damped = omega * Math.sqrt(1 - zeta * zeta);
		return 1 - Math.exp(-zeta * omega * t) * (Math.cos(damped * t) + ((zeta * omega) / damped) * Math.sin(damped * t));
	};

	return { tokens, state, L, icon, recordGlyph, pick, shell, seg, hex, oklch, color, value, contrast, spring, onChange: (fn) => listeners.push(fn) };
})();
