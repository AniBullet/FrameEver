import {
	createContext,
	createEffect,
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

type TranslationKey = keyof typeof zhCN;

const I18nContext = createContext<{
	t: (key: TranslationKey) => string;
	language: () => Language;
	setLanguage: (lang: Language) => void;
}>();

export function I18nProvider(props: ParentProps) {
	const [language, setLanguageSignal] = createSignal<Language>("zh-CN");

	onMount(async () => {
		const settings = await generalSettingsStore.get();
		if (settings?.language) {
			console.log("[I18n] Initial language from store:", settings.language);
			setLanguageSignal(settings.language as Language);
			events.languageChanged.emit();
		} else {
			try {
				const systemLocale = await commands.getSystemLocale();
				console.log("[I18n] Detected system locale:", systemLocale);
				const detectedLang = systemLocale as Language;
				setLanguageSignal(detectedLang);
				await generalSettingsStore.set({ language: detectedLang });
				events.languageChanged.emit();
			} catch (error) {
				console.error("[I18n] Failed to detect system locale:", error);
				setLanguageSignal("en");
				events.languageChanged.emit();
			}
		}
	});

	createEffect(() => {
		const interval = setInterval(async () => {
			const settings = await generalSettingsStore.get();
			if (settings?.language) {
				const currentLang = language();
				if (settings.language !== currentLang) {
					console.log(
						"[I18n] Language changed in store:",
						settings.language,
						"current:",
						currentLang,
					);
					setLanguageSignal(settings.language as Language);
				}
			}
		}, 500);

		onCleanup(() => clearInterval(interval));
	});

	const setLanguage = async (lang: Language) => {
		console.log("[I18n] setLanguage called:", lang);
		setLanguageSignal(lang);
		await generalSettingsStore.set({ language: lang });
		events.languageChanged.emit();
	};

	const dict = createMemo(() => {
		const lang = language();
		console.log("[I18n] Dictionary memo recomputed for language:", lang);
		return translations[lang];
	});

	const t = (key: TranslationKey): string => {
		return dict()[key] ?? translations["zh-CN"][key] ?? key;
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
