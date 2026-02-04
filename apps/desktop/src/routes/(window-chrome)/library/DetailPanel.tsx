import { Button } from "@cap/ui-solid";
import { convertFileSrc } from "@tauri-apps/api/core";
import { ask } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import * as shell from "@tauri-apps/plugin-shell";
import { cx } from "cva";
import { createSignal, Show } from "solid-js";
import { useI18n } from "~/i18n";
import { commands, type LibraryItem } from "~/utils/tauri";
import IconLucideCheckCircle from "~icons/lucide/check-circle";
import IconLucideEdit from "~icons/lucide/edit";
import IconLucideExternalLink from "~icons/lucide/external-link";
import IconLucideFolder from "~icons/lucide/folder";
import IconLucidePlay from "~icons/lucide/play";
import IconLucideTrash from "~icons/lucide/trash-2";
import IconLucideX from "~icons/lucide/x";

type Props = {
	item: LibraryItem;
	onClose: () => void;
	onRefetch: () => void;
};

export default function DetailPanel(props: Props) {
	const { t } = useI18n();
	const [imageExists, setImageExists] = createSignal(true);

	const thumbnailSrc = () => {
		if (props.item.thumbnailPath) {
			return `${convertFileSrc(props.item.thumbnailPath)}?t=${Date.now()}`;
		}
		if (props.item.exportedFilePath && props.item.itemType === "screenshot") {
			return convertFileSrc(props.item.exportedFilePath);
		}
		return "";
	};

	const canEdit = () => {
		return props.item.capProjectPath !== null;
	};

	const canPlay = () => {
		return (
			props.item.exportedFilePath !== null && props.item.itemType === "video"
		);
	};

	const handleEdit = () => {
		if (props.item.capProjectPath) {
			if (props.item.itemType === "video") {
				commands.showWindow({
					Editor: { project_path: props.item.capProjectPath },
				});
			} else {
				commands.showWindow({
					ScreenshotEditor: { path: props.item.capProjectPath },
				});
			}
		}
	};

	const handlePlay = () => {
		if (props.item.exportedFilePath) {
			shell.open(props.item.exportedFilePath);
		}
	};

	const handleOpenFolder = () => {
		if (props.item.capProjectPath) {
			revealItemInDir(props.item.capProjectPath);
		} else if (props.item.exportedFilePath) {
			revealItemInDir(props.item.exportedFilePath);
		}
	};

	const handleDelete = async () => {
		const message =
			props.item.itemType === "video"
				? t("library.detail.confirmDeleteRecording")
				: t("library.detail.confirmDeleteScreenshot");

		if (!(await ask(message))) return;

		try {
			const pathToDelete =
				props.item.capProjectPath || props.item.exportedFilePath;
			if (pathToDelete) {
				await commands.deleteLibraryItem(pathToDelete);
			}
			props.onRefetch();
			props.onClose();
		} catch (error) {
			console.error("Failed to delete item:", error);
		}
	};

	const statusLabel = () => {
		switch (props.item.status) {
			case "editing":
				return t("library.status.editing");
			case "exported":
				return t("library.status.exported");
			case "exportedNoSource":
				return t("library.status.exportedNoSource");
		}
	};

	return (
		<div class="w-96 bg-gray-2 border-l border-gray-4 flex flex-col">
			<div class="h-16 border-b border-gray-4 flex items-center justify-between px-4">
				<h3 class="font-semibold text-gray-12">{t("library.detail.title")}</h3>
				<button
					type="button"
					onClick={props.onClose}
					class="p-2 rounded-lg hover:bg-gray-3 transition-colors"
					aria-label={t("library.detail.close")}
				>
					<IconLucideX class="size-4 text-gray-11" />
				</button>
			</div>

			<div class="flex-1 overflow-auto custom-scroll p-4 space-y-4">
				<div class="aspect-video w-full bg-gray-3 rounded-lg overflow-hidden">
					<Show
						when={imageExists() && thumbnailSrc()}
						fallback={<div class="w-full h-full bg-gray-4" />}
					>
						<img
							src={thumbnailSrc()}
							alt={props.item.name}
							class="w-full h-full object-cover"
							onError={() => setImageExists(false)}
						/>
					</Show>
				</div>

				<div class="space-y-3">
					<div>
						<label class="text-xs font-semibold text-gray-11 uppercase tracking-wider">
							{t("library.detail.name")}
						</label>
						<p class="mt-1 text-sm text-gray-12 break-words">
							{props.item.name}
						</p>
					</div>

					<div>
						<label class="text-xs font-semibold text-gray-11 uppercase tracking-wider">
							{t("library.detail.type")}
						</label>
						<p class="mt-1 text-sm text-gray-11">
							{props.item.itemType === "video"
								? t("library.type.video")
								: t("library.type.screenshot")}
						</p>
					</div>

					<div>
						<label class="text-xs font-semibold text-gray-11 uppercase tracking-wider">
							{t("library.detail.status")}
						</label>
						<div class="mt-1 flex items-center gap-2">
							<Show when={props.item.status === "editing"}>
								<IconLucideEdit class="size-4 text-orange-500" />
							</Show>
							<Show
								when={
									props.item.status === "exported" ||
									props.item.status === "exportedNoSource"
								}
							>
								<IconLucideCheckCircle class="size-4 text-green-500" />
							</Show>
							<span class="text-sm text-gray-11">{statusLabel()}</span>
						</div>
					</div>

					<Show when={props.item.capProjectPath}>
						<div>
							<label class="text-xs font-semibold text-gray-11 uppercase tracking-wider">
								{t("library.detail.projectPath")}
							</label>
							<p class="mt-1 text-xs text-gray-10 break-all font-mono">
								{props.item.capProjectPath}
							</p>
						</div>
					</Show>

					<Show when={props.item.exportedFilePath}>
						<div>
							<label class="text-xs font-semibold text-gray-11 uppercase tracking-wider">
								{t("library.detail.exportedPath")}
							</label>
							<p class="mt-1 text-xs text-gray-10 break-all font-mono">
								{props.item.exportedFilePath}
							</p>
						</div>
					</Show>
				</div>
			</div>

			<div class="border-t border-gray-4 p-4 space-y-2">
				<Show when={canEdit()}>
					<Button
						variant="primary"
						size="md"
						class="w-full"
						onClick={handleEdit}
					>
						<IconLucideEdit class="size-4" />
						{t("library.detail.edit")}
					</Button>
				</Show>

				<Show when={canPlay()}>
					<Button
						variant="primary"
						size="md"
						class="w-full"
						onClick={handlePlay}
					>
						<IconLucidePlay class="size-4" />
						{t("library.detail.play")}
					</Button>
				</Show>

				<Button
					variant="gray"
					size="md"
					class="w-full"
					onClick={handleOpenFolder}
				>
					<IconLucideFolder class="size-4" />
					{t("library.detail.openFolder")}
				</Button>

				<Button
					variant="destructive"
					size="md"
					class="w-full"
					onClick={handleDelete}
				>
					<IconLucideTrash class="size-4" />
					{t("library.detail.delete")}
				</Button>
			</div>
		</div>
	);
}
