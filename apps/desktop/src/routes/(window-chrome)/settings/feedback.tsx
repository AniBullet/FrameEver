import { Button } from "@cap/ui-solid";
import { getVersion } from "@tauri-apps/api/app";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { type as ostype } from "@tauri-apps/plugin-os";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import { createResource, createSignal, For, Show } from "solid-js";
import toast from "solid-toast";
import { useI18n } from "~/i18n";

import { commands, type SystemDiagnostics } from "~/utils/tauri";

async function fetchDiagnostics(): Promise<SystemDiagnostics | null> {
	try {
		return await commands.getSystemDiagnostics();
	} catch (e) {
		console.error("Failed to fetch diagnostics:", e);
		return null;
	}
}

export default function FeedbackTab() {
	const { t } = useI18n();
	const [exportingLogs, setExportingLogs] = createSignal(false);
	const [diagnostics] = createResource(fetchDiagnostics);

	const handleExportLogs = async () => {
		setExportingLogs(true);
		try {
			const diagnosticsData = diagnostics();
			if (!diagnosticsData) {
				toast.error("No diagnostics data available");
				return;
			}

			const version = await getVersion();
			const os = ostype();
			const timestamp = new Date()
				.toISOString()
				.replace(/:/g, "-")
				.split(".")[0];

			const logContent = `Cap aniBullet - System Diagnostics
Generated: ${new Date().toLocaleString()}
Version: ${version}
OS: ${os}

${JSON.stringify(diagnosticsData, null, 2)}`;

			const filePath = await save({
				defaultPath: `cap-diagnostics-${timestamp}.txt`,
				filters: [
					{
						name: "Text File",
						extensions: ["txt"],
					},
				],
			});

			if (filePath) {
				await writeTextFile(filePath, logContent);
				toast.success(t("feedback.debug.logs.success"));
			}
		} catch (error) {
			toast.error(t("feedback.debug.logs.error"));
			console.error("Failed to export diagnostics:", error);
		} finally {
			setExportingLogs(false);
		}
	};

	return (
		<div class="flex flex-col w-full h-full">
			<div class="flex-1 custom-scroll">
				<div class="p-4 space-y-4">
					<div class="flex flex-col pb-4 border-b border-gray-2">
						<h2 class="text-lg font-medium text-gray-12">
							{t("feedback.title")}
						</h2>
						<p class="text-sm text-gray-10">{t("feedback.description")}</p>
					</div>

					<div class="pt-2">
						<h3 class="text-sm font-medium text-gray-12 mb-2">
							{t("feedback.project.home")}
						</h3>
						<p class="text-sm text-gray-10 mb-3">
							{t("feedback.project.home.description")}
						</p>
						<Button
							onClick={() =>
								openUrl("https://github.com/AniBullet/Cap-aniBullet")
							}
							size="md"
							variant="gray"
						>
							{t("feedback.project.home.button")}
						</Button>
					</div>

					<div class="pt-6 border-t border-gray-2">
						<h3 class="text-sm font-medium text-gray-12 mb-2">
							{t("feedback.original.community")}
						</h3>
						<p class="text-sm text-gray-10 mb-3">
							{t("feedback.original.community.description")}
						</p>
						<Button
							onClick={() => openUrl("https://cap.link/discord")}
							size="md"
							variant="gray"
						>
							{t("feedback.original.community.button")}
						</Button>
					</div>

					<div class="pt-6 border-t border-gray-2">
						<h3 class="text-sm font-medium text-gray-12 mb-2">
							{t("feedback.debug.logs")}
						</h3>
						<p class="text-sm text-gray-10 mb-3">
							{t("feedback.debug.logs.description")}
						</p>
						<Button
							onClick={handleExportLogs}
							size="md"
							variant="gray"
							disabled={exportingLogs()}
						>
							{exportingLogs()
								? t("feedback.debug.logs.exporting")
								: t("feedback.debug.logs.button")}
						</Button>
					</div>

					<div class="pt-6 border-t border-gray-2">
						<h3 class="text-sm font-medium text-gray-12 mb-3">
							{t("feedback.system.title")}
						</h3>
						<Show
							when={!diagnostics.loading && diagnostics()}
							fallback={
								<p class="text-sm text-gray-10">
									{t("feedback.system.loading")}
								</p>
							}
						>
							{(diag) => {
								const d = diag() as Record<string, unknown>;
								const osVersion =
									"macosVersion" in d
										? (d.macosVersion as { displayName: string } | null)
										: "windowsVersion" in d
											? (d.windowsVersion as { displayName: string } | null)
											: null;
								const captureSupported =
									"screenCaptureSupported" in d
										? (d.screenCaptureSupported as boolean)
										: "graphicsCaptureSupported" in d
											? (d.graphicsCaptureSupported as boolean)
											: false;
								return (
									<div class="space-y-3 text-sm">
										<Show when={osVersion}>
											{(ver) => (
												<div class="space-y-1">
													<p class="text-gray-11 font-medium">
														{t("feedback.system.os")}
													</p>
													<p class="text-gray-10 bg-gray-2 px-2 py-1.5 rounded font-mono text-xs">
														{ver().displayName}
													</p>
												</div>
											)}
										</Show>

										<div class="space-y-1">
											<p class="text-gray-11 font-medium">
												{t("feedback.system.capture.support")}
											</p>
											<div class="flex gap-2 flex-wrap">
												<span
													class={`px-2 py-1 rounded text-xs ${
														captureSupported
															? "bg-green-500/20 text-green-400"
															: "bg-red-500/20 text-red-400"
													}`}
												>
													{t("feedback.system.capture.label")}:{" "}
													{captureSupported
														? t("feedback.system.supported")
														: t("feedback.system.notSupported")}
												</span>
											</div>
										</div>

										<Show when={(d.availableEncoders as string[])?.length > 0}>
											<div class="space-y-1">
												<p class="text-gray-11 font-medium">
													{t("feedback.system.encoders")}
												</p>
												<div class="flex gap-1.5 flex-wrap">
													<For each={d.availableEncoders as string[]}>
														{(encoder) => (
															<span class="px-2 py-1 bg-gray-2 rounded text-xs text-gray-10 font-mono">
																{encoder}
															</span>
														)}
													</For>
												</div>
											</div>
										</Show>
									</div>
								);
							}}
						</Show>
					</div>
				</div>
			</div>
		</div>
	);
}
