window.FE = (() => {
	const tokens = window.FRAMEEVER_TOKENS;
	const icons = window.FRAMEEVER_ICONS;
	const defaults = { theme: "dark", lang: "zh", scale: "100" };
	const choices = {
		theme: ["dark", "light"],
		lang: ["en", "zh"],
		scale: ["100", "150", "200"],
	};
	const surfaces = [
		["main-window-library", "Main window and library", "主窗口和资源库", true],
		["settings", "Settings", "设置", true],
		["editor", "Editor", "编辑器", true],
		["screenshot-editor", "Screenshot editor", "截图编辑器", true],
		["recording-controls", "Recording controls", "录制控制条", true],
		["area-picker", "Area picker and countdown", "区域选择和倒计时", true],
		["tray", "Tray", "托盘", true],
		["camera-teleprompter", "Camera and teleprompter", "摄像头和提词器", true],
		["converter", "FrameEver Converter", "FrameEver Converter", true],
		["installer", "Installer", "安装器", true],
	];
	const state = { ...defaults, at: "" };

	const L = (en, zh) =>
		`<span lang="en">${en}</span><span lang="zh">${zh}</span>`;
	const icon = (name, cls = "") => {
		if (!icons[name]) throw new Error(`No icon: ${name}`);
		return `<svg class="i ${cls}" viewBox="0 0 24 24" aria-hidden="true">${icons[name]}</svg>`;
	};
	const recordGlyph = () =>
		'<span class="rec-glyph" aria-hidden="true"></span>';
	const stroke = (d, ink, width, move, delay, time, extra = {}) => ({
		d,
		ink,
		width,
		move,
		delay,
		time,
		...extra,
	});
	const doodles = {
		FRAME: {
			view: [120, 104],
			strokes: [
				stroke(
					"M 24 14 L 96 14 Q 104 14 104 22 L 104 74 Q 104 82 96 82 L 24 82 Q 16 82 16 74 L 16 22 Q 16 14 24 14 Z",
					"link",
					2.5,
					"fade",
					100,
					400,
					{ dash: "2 4.2" },
				),
				stroke(
					"M 60 37 C 66 37 71 42 71 48 C 71 54 66 59 60 59 C 54 59 49 54 49 48 C 49 42 54 37 60 37 Z",
					"recording",
					0,
					"pop",
					450,
					450,
					{ fill: true },
				),
			],
		},
		TICK: {
			view: [24, 24],
			strokes: [
				stroke("M 4 13 L 9.5 18 L 20 6", "success-text", 3, "draw", 200, 300),
			],
		},
		ALL_SET: {
			view: [120, 104],
			strokes: [
				stroke(
					"M 60 12 C 84 12 102 30 102 52 C 102 75 84 92 60 92 C 35 92 18 74 18 52 C 18 31 34 14 56 12.5 C 62 12 68 13 73 15",
					"muted-foreground",
					3,
					"draw",
					450,
					750,
				),
				stroke("M 38 54 L 54 70 L 84 34", "success-text", 5.5, "draw", 80, 500),
			],
		},
		FLOW: {
			view: [120, 104],
			strokes: [
				stroke(
					"M 20 20 L 50 20 Q 56 20 56 26 L 56 44 Q 56 50 50 50 L 20 50 Q 14 50 14 44 L 14 26 Q 14 20 20 20 Z",
					"foreground",
					3.5,
					"draw",
					60,
					550,
				),
				stroke(
					"M 72 56 L 100 56 Q 106 56 106 62 L 106 80 Q 106 86 100 86 L 72 86 Q 66 86 66 80 L 66 62 Q 66 56 72 56 Z",
					"foreground",
					3.5,
					"draw",
					420,
					550,
				),
				stroke("M 34 52 C 34 72 44 72 62 71", "link", 3, "draw", 800, 350),
			],
		},
		LENS: {
			view: [120, 104],
			strokes: [
				stroke(
					"M 52 16 C 68 16 82 30 82 46 C 82 62 68 76 52 76 C 36 76 22 62 22 46 C 22 30 36 16 52 16 Z",
					"foreground",
					4,
					"draw",
					60,
					700,
				),
				stroke("M 74 68 L 98 92", "link", 5, "draw", 650, 300),
			],
		},
		UP_ARROW: {
			view: [34, 44],
			strokes: [
				stroke("M 12 40 C 6 30 8 16 22 6", "link", 3, "draw", 200, 450),
				stroke("M 12 6 L 23 5 L 22 16", "link", 3, "draw", 600, 250),
			],
		},
		UNDERLINE: {
			view: [132, 12],
			strokes: [
				stroke("M 4 8 C 34 4 78 3 128 6", "link", 3.5, "draw", 350, 650),
			],
		},
	};
	let doodleCount = 0;
	const doodle = (name, { width, still = false }) => {
		const { view, strokes } = doodles[name];
		const id = `doodle-${++doodleCount}`;
		const paths = strokes.map((s) => {
			const paint = s.fill
				? `fill="var(--${s.ink})" stroke="none"`
				: `fill="none" stroke="var(--${s.ink})" stroke-width="${s.width}" stroke-linecap="round" stroke-linejoin="round"`;
			const dash = s.dash
				? `stroke-dasharray="${s.dash}"`
				: s.move === "draw"
					? 'pathLength="1"'
					: "";
			return `<path class="${s.move}" d="${s.d}" ${paint} ${dash} style="--delay:${s.delay}ms;--time:${s.time}ms"/>`;
		});
		return `<svg class="doodle ${still ? "still" : ""}" width="${width}" height="${Math.round((width * view[1]) / view[0])}" viewBox="0 0 ${view[0]} ${view[1]}" aria-hidden="true"><defs><filter id="${id}" x="-10" y="-10" width="${view[0] + 20}" height="${view[1] + 20}" filterUnits="userSpaceOnUse"><feTurbulence type="fractalNoise" baseFrequency="0.05" numOctaves="1" seed="1"/><feDisplacementMap in="SourceGraphic" scale="2.5" xChannelSelector="R" yChannelSelector="G"/></filter></defs><g filter="url(#${id})">${paths.join("")}</g></svg>`;
	};
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
			[
				`${prefix}index.html`,
				"index",
				"Tokens and components",
				"Token 和组件",
				true,
			],
			...surfaces.map(([slug, en, zh, ready]) => [
				`${prefix}surfaces/${slug}.html`,
				slug,
				en,
				zh,
				ready,
			]),
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
				${seg("theme", [
					["dark", L("Dark", "深色")],
					["light", L("Light", "浅色")],
				])}
				${seg("lang", [
					["en", "English"],
					["zh", "简体中文"],
				])}
				${seg("scale", [
					["100", "100%"],
					["150", "150%"],
					["200", "200%"],
				])}
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
				document
					.getElementById(at.dataset.at)
					?.scrollIntoView({ behavior: "auto" });
			}
		});
		window.addEventListener("hashchange", () => {
			readHash();
			apply();
		});
		document.fonts
			.load('16px "HarmonyOS Sans SC"', "帧刻")
			.then((faces) => {
				if (faces.length === 0)
					document.getElementById("font-note").classList.add("show");
			})
			.catch(() => document.getElementById("font-note").classList.add("show"));
		readHash();
		apply();
		if (state.at)
			document.getElementById(state.at)?.scrollIntoView({ behavior: "auto" });
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
	const luminanceOf = (rgb) =>
		0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
	const hex = (value) => {
		const gamma = (x) => {
			const y = unit(x);
			return Math.round(
				(y <= 0.0031308 ? 12.92 * y : 1.055 * y ** (1 / 2.4) - 0.055) * 255,
			);
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
		const token = [...tokens.semantic, ...tokens.componentVars].find(
			(entry) => entry.name === name,
		);
		return value(theme, token[theme]);
	};
	const color = (theme, name) => oklch(value(theme, name));
	const luminance = (theme, name) => {
		const fg = value(theme, name);
		const rgb = toLinear(fg).map(unit);
		if (fg[3] === undefined) return luminanceOf(rgb);
		const under = toLinear(
			value(theme, name === "fixed.badge" ? "preview" : "background"),
		).map(unit);
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
		return (
			1 -
			Math.exp(-zeta * omega * t) *
				(Math.cos(damped * t) +
					((zeta * omega) / damped) * Math.sin(damped * t))
		);
	};

	return {
		tokens,
		state,
		L,
		icon,
		recordGlyph,
		doodle,
		pick,
		shell,
		seg,
		hex,
		oklch,
		color,
		value,
		contrast,
		spring,
		onChange: (fn) => listeners.push(fn),
	};
})();
