import { A, type RouteSectionProps } from "@solidjs/router";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import * as shell from "@tauri-apps/plugin-shell";
import { check as checkForUpdates } from "@tauri-apps/plugin-updater";
import "@total-typescript/ts-reset/filter-boolean";
import {
	createResource,
	createSignal,
	For,
	onMount,
	Show,
	Suspense,
} from "solid-js";
import { CapErrorBoundary } from "~/components/CapErrorBoundary";
import { useI18n } from "~/i18n";

const WINDOW_SIZE = { width: 700, height: 540 } as const;

export default function Settings(props: RouteSectionProps) {
	const { t } = useI18n();
	const [version] = createResource(() => getVersion());
	const [updateAvailable, setUpdateAvailable] = createSignal<string | null>(
		null,
	);

	onMount(() => {
		const currentWindow = getCurrentWindow();

		currentWindow.setSize(
			new LogicalSize(WINDOW_SIZE.width, WINDOW_SIZE.height),
		);

		setTimeout(async () => {
			try {
				const update = await checkForUpdates();
				if (update) {
					setUpdateAvailable(update.version);
				}
			} catch (e) {
				console.error("自动检查更新失败:", e);
			}
		}, 2000);
	});

	return (
		<div class="flex-1 flex flex-row divide-x divide-gray-3 text-[0.875rem] leading-[1.25rem] overflow-y-hidden">
			<div class="flex flex-col h-full bg-gray-2">
				<ul class="min-w-[12rem] h-full p-[0.625rem] space-y-1 text-gray-12">
					<For
						each={[
							{
								href: "general",
								name: t("settings.general"),
								icon: IconCapSettings,
							},
							{
								href: "hotkeys",
								name: t("settings.hotkeys"),
								icon: IconCapHotkeys,
							},
							{
								href: "experimental",
								name: t("settings.experimental"),
								icon: IconCapSettings,
							},
							{
								href: "feedback",
								name: t("settings.feedback"),
								icon: IconLucideMessageSquarePlus,
							},
						].filter(Boolean)}
					>
						{(item) => (
							<li>
								<A
									href={item.href}
									activeClass="bg-gray-5 pointer-events-none"
									class="rounded-lg h-[2rem] hover:bg-gray-3 text-[13px] px-2 flex flex-row items-center gap-[0.375rem] transition-colors"
								>
									<item.icon class="opacity-60 size-4" />
									<span>{item.name}</span>
								</A>
							</li>
						)}
					</For>
				</ul>
				<div class="p-[0.625rem] text-left flex flex-col">
					<Show when={updateAvailable()}>
						{(newVersion) => (
							<A
								href="general"
								class="mb-2 px-2 py-1.5 rounded-lg bg-blue-500/10 border border-blue-500/30 hover:bg-blue-500/20 transition-colors"
							>
								<div class="text-xs font-medium text-blue-11 flex items-center gap-1.5">
									<IconLucideSparkles class="size-3.5" />
									<span>
										{t("app.update.available")} {newVersion()}
									</span>
								</div>
							</A>
						)}
					</Show>
					<Show when={version()}>
						{(v) => (
							<div class="mb-2 text-xs text-gray-11 flex flex-col items-start gap-0.5">
								<span>v{v()}</span>
								<button
									type="button"
									class="text-gray-11 hover:text-gray-12 underline transition-colors"
									onClick={() =>
										shell.open(
											"https://github.com/AniBullet/Cap-aniBullet/releases",
										)
									}
								>
									{t("settings.versions")}
								</button>
							</div>
						)}
					</Show>
				</div>
			</div>
			<div class="overflow-y-hidden flex-1 animate-in">
				<CapErrorBoundary>
					<Suspense>{props.children}</Suspense>
				</CapErrorBoundary>
			</div>
		</div>
	);
}
