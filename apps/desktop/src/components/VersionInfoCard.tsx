import { Button } from "@cap/ui-solid";
import { getVersion } from "@tauri-apps/api/app";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import {
	type Component,
	createResource,
	createSignal,
	onMount,
	Show,
} from "solid-js";
import { useI18n } from "~/i18n";
import { commands } from "~/utils/tauri";

export const VersionInfoCard: Component = () => {
	const { t } = useI18n();
	const [version] = createResource(() => getVersion());
	const [checking, setChecking] = createSignal(false);
	const [updateInfo, setUpdateInfo] = createSignal<any>(null);
	const [error, setError] = createSignal<string | null>(null);
	const [autoChecked, setAutoChecked] = createSignal(false);

	const handleCheckUpdate = async () => {
		setChecking(true);
		setError(null);
		setUpdateInfo(null);

		try {
			const update = await commands.checkGithubRelease();

			if (update) {
				setUpdateInfo(update);
			} else {
				setError(t("app.update.latest"));
				setTimeout(() => setError(null), 3000);
			}
		} catch (e) {
			console.error("更新检查失败:", e);
			setError(t("app.update.error"));
			setTimeout(() => setError(null), 3000);
		} finally {
			setChecking(false);
		}
	};

	const handleOpenDownload = async () => {
		const info = updateInfo();
		if (info?.download_url) {
			await openUrl(info.download_url);
		}
	};

	onMount(() => {
		setTimeout(async () => {
			if (!autoChecked()) {
				setAutoChecked(true);
				try {
					const update = await commands.checkGithubRelease();
					if (update) {
						setUpdateInfo(update);
					}
				} catch (e) {
					console.error("自动检查更新失败（已静默）:", e);
				}
			}
		}, 3000);
	});

	return (
		<div class="p-4 rounded-lg bg-gray-2 dark:bg-gray-3 border border-gray-4 dark:border-gray-6">
			<div class="flex items-center justify-between gap-4">
				<div class="flex items-center gap-2">
					<div class="text-lg">🎬</div>
					<div>
						<h3 class="text-base font-semibold text-gray-12">
							{t("app.version.card.title")}
						</h3>
						<p class="text-xs text-gray-11 mt-0.5">
							{t("app.version.label")} {version() || "..."} •{" "}
							{t("app.version.card.subtitle")}
						</p>
					</div>
				</div>
				<div class="flex items-center gap-2 shrink-0">
					<Show when={error()}>
						<span class="text-xs text-gray-11">{error()}</span>
					</Show>
					<Show when={updateInfo()}>
						{(info) => (
							<>
								<span class="text-xs text-blue-11">
									{t("app.update.available")} {info().version}
								</span>
								<Button
									size="sm"
									variant="primary"
									onClick={handleOpenDownload}
								>
									{t("app.update.download")}
								</Button>
							</>
						)}
					</Show>
					<Show when={!updateInfo() && !error()}>
						<Button
							size="sm"
							variant="gray"
							disabled={checking()}
							onClick={handleCheckUpdate}
						>
							{checking() ? t("app.update.checking") : t("app.update.check")}
						</Button>
					</Show>
				</div>
			</div>
		</div>
	);
};
