import {
	createContext,
	createMemo,
	createSignal,
	onCleanup,
	onMount,
	type ParentProps,
	useContext,
} from "solid-js";
import { generalSettingsStore } from "~/store";
import { commands, events } from "~/utils/tauri";
import en from "./locales/en.json";
import ja from "./locales/ja.json";
import ko from "./locales/ko.json";
import zhCN from "./locales/zh-CN.json";

export type Language = "zh-CN" | "en" | "ja" | "ko";

const translations = {
	"zh-CN": zhCN,
	en: en,
	ja: ja,
	ko: ko,
} as const;

export type TranslationKey = keyof typeof zhCN;

const I18nContext = createContext<{
	t: (key: TranslationKey, params?: Record<string, string>) => string;
	language: () => Language;
	setLanguage: (lang: Language) => void;
}>();

export function I18nProvider(props: ParentProps) {
	const [language, setLanguageSignal] = createSignal<Language>("en");

	onMount(async () => {
		const settings = await generalSettingsStore.get();
		if (settings?.language) {
			setLanguageSignal(settings.language as Language);
			events.languageChanged.emit();
		} else {
			try {
				const systemLocale = await commands.getSystemLocale();
				const detectedLang = systemLocale as Language;
				setLanguageSignal(detectedLang);
				await generalSettingsStore.set({ language: detectedLang });
				events.languageChanged.emit();
			} catch {
				setLanguageSignal("en");
				events.languageChanged.emit();
			}
		}
	});

	const unlistenPromise = generalSettingsStore.listen(async () => {
		const settings = await generalSettingsStore.get();
		if (settings?.language) {
			const currentLang = language();
			if (settings.language !== currentLang) {
				setLanguageSignal(settings.language as Language);
			}
		}
	});

	onCleanup(() => {
		unlistenPromise.then((unlisten) => unlisten()).catch(() => {});
	});

	const setLanguage = async (lang: Language) => {
		setLanguageSignal(lang);
		await generalSettingsStore.set({ language: lang });
		events.languageChanged.emit();
	};

	const dict = createMemo(() => {
		return translations[language()];
	});

	const t = (key: TranslationKey, params?: Record<string, string>): string => {
		let s = dict()[key] ?? translations.en[key] ?? key;
		if (params && typeof s === "string") {
			for (const [k, v] of Object.entries(params)) {
				s = s.replace(new RegExp(`\\{${k}\\}`, "g"), v);
			}
		}
		return s;
	};

	return (
		<I18nContext.Provider value={{ t, language, setLanguage }}>
			{props.children}
		</I18nContext.Provider>
	);
}

export function useI18n() {
	const context = useContext(I18nContext);
	if (!context) {
		throw new Error("useI18n must be used within I18nProvider");
	}
	return context;
}
