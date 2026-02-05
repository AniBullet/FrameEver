import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { type } from "@tauri-apps/plugin-os";
import { createResource, Show } from "solid-js";
import { createStore } from "solid-js/store";
import { useI18n } from "~/i18n";
import { generalSettingsStore } from "~/store";
import { commands, type GeneralSettingsStore } from "~/utils/tauri";
import { ToggleSettingItem } from "./Setting";

export default function ExperimentalSettings() {
	const [store] = createResource(() => generalSettingsStore.get());

	return (
		<Show when={store.state === "ready" && ([store()] as const)}>
			{(store) => <Inner initialStore={store()[0] ?? null} />}
		</Show>
	);
}

function Inner(props: { initialStore: GeneralSettingsStore | null }) {
	const { t } = useI18n();
	const [settings, setSettings] = createStore<GeneralSettingsStore>(
		props.initialStore ?? {
			hideDockIcon: false,
			enableNotifications: true,
			enableNativeCameraPreview: false,
			autoZoomOnClicks: false,
			custom_cursor_capture2: true,
		},
	);

	const handleChange = async <K extends keyof typeof settings>(
		key: K,
		value: (typeof settings)[K],
	) => {
		console.log(`Handling settings change for ${key}: ${value}`);

		setSettings(key as keyof GeneralSettingsStore, value);
		generalSettingsStore.set({ [key]: value });
		if (key === "enableNativeCameraPreview") {
			await commands.setCameraInput(null, true);
			try {
				const cameraWindow = await WebviewWindow.getByLabel("camera");
				await cameraWindow?.close();
			} catch (error) {
				console.error("Failed to close camera window", error);
			}
		}
	};

	return (
		<div class="flex flex-col h-full custom-scroll">
			<div class="p-4 space-y-4">
				<div class="flex flex-col pb-4 border-b border-gray-2">
					<h2 class="text-lg font-medium text-gray-12">
						{t("experimental.title")}
					</h2>
					<p class="text-sm text-gray-10">{t("experimental.description")}</p>
				</div>
				<div class="space-y-3">
					<h3 class="text-sm text-gray-12 w-fit">
						{t("experimental.section.recording")}
					</h3>
					<div class="px-3 rounded-xl border divide-y divide-gray-3 border-gray-3 bg-gray-2">
						<ToggleSettingItem
							label={t("experimental.cursor.capture.label")}
							description={t("experimental.cursor.capture.description")}
							value={!!settings.custom_cursor_capture2}
							onChange={(value) =>
								handleChange("custom_cursor_capture2", value)
							}
						/>
						{type() !== "windows" && (
							<ToggleSettingItem
								label={t("experimental.native.camera.label")}
								description={t("experimental.native.camera.description")}
								value={!!settings.enableNativeCameraPreview}
								onChange={(value) =>
									handleChange("enableNativeCameraPreview", value)
								}
							/>
						)}
						<ToggleSettingItem
							label={t("experimental.auto.zoom.label")}
							description={t("experimental.auto.zoom.description")}
							value={!!settings.autoZoomOnClicks}
							onChange={(value) => {
								handleChange("autoZoomOnClicks", value);
								setTimeout(
									() => window.scrollTo({ top: 0, behavior: "instant" }),
									5,
								);
							}}
						/>
					</div>
				</div>
			</div>
		</div>
	);
}
