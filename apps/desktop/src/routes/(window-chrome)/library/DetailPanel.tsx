import { Button } from "@cap/ui-solid";
import { Channel, convertFileSrc, invoke } from "@tauri-apps/api/core";
import { ask } from "@tauri-apps/plugin-dialog";
import { createEffect, createSignal, Show } from "solid-js";
import toast from "solid-toast";
import { useI18n } from "~/i18n";
import { commands, type FramesRendered, type LibraryItem } from "~/utils/tauri";
import IconLucideCheckCircle from "~icons/lucide/check-circle";
import IconLucideCopy from "~icons/lucide/copy";
import IconLucideEdit from "~icons/lucide/edit";
import IconLucideExternalLink from "~icons/lucide/external-link";
import IconLucideFolder from "~icons/lucide/folder";
import IconLucideImage from "~icons/lucide/image";
import IconLucideMinimize2 from "~icons/lucide/minimize-2";
import IconLucidePlay from "~icons/lucide/play";
import IconLucideTrash from "~icons/lucide/trash-2";
import IconLucideVideo from "~icons/lucide/video";
import IconLucideX from "~icons/lucide/x";
import {
	canEdit as canEditItem,
	canOpen as canOpenItem,
	canPlay as canPlayItem,
	editItem,
	openFolder as openFolderForItem,
	openWithDefaultApp,
} from "./library-actions";

type Props = {
	item: LibraryItem;
	onClose: () => void;
	onRefetch: () => void;
};

type VersionTab = "original" | "compressed";

export default function DetailPanel(props: Props) {
	const { t } = useI18n();
	const [imageExists, setImageExists] = createSignal(true);
	const [activeVersion, setActiveVersion] =
		createSignal<VersionTab>("original");

	createEffect(() => {
		const item = props.item;
		if (!item.compressedFilePath && activeVersion() === "compressed") {
			setActiveVersion("original");
		}
		if (
			!item.exportedFilePath &&
			activeVersion() === "original" &&
			item.compressedFilePath
		) {
			setActiveVersion("compressed");
		}
		setImageExists(true);
	});

	const hasCompressed = () => !!props.item.compressedFilePath;
	const hasOriginal = () => !!props.item.exportedFilePath;
	const hasBothVersions = () => hasOriginal() && hasCompressed();

	const activeFilePath = () => {
		if (activeVersion() === "compressed" && props.item.compressedFilePath) {
			return props.item.compressedFilePath;
		}
		return props.item.exportedFilePath;
	};

	const previewSrc = () => {
		if (props.item.itemType === "screenshot") {
			const path = activeFilePath();
			if (!path) return "";
			return convertFileSrc(path);
		}
		if (props.item.thumbnailPath) {
			return `${convertFileSrc(props.item.thumbnailPath)}?t=${Date.now()}`;
		}
		return "";
	};

	const videoPreviewSrc = () => {
		if (props.item.itemType !== "video") return "";
		if (props.item.thumbnailPath) return "";
		const path = activeFilePath();
		if (!path) return "";
		return convertFileSrc(path);
	};

	const canEdit = () => canEditItem(props.item);
	const canPlay = () => canPlayItem(props.item) && !!activeFilePath();
	const canOpen = () => canOpenItem(props.item) && !!activeFilePath();
	const canCompress = () =>
		(props.item.exportedFilePath?.endsWith(".mp4") ?? false) &&
		!hasCompressed();

	const handleEdit = () => editItem(props.item);
	const handlePlay = () => {
		const path = activeFilePath();
		if (path) openWithDefaultApp(path);
	};
	const handleOpen = () => {
		const path = activeFilePath();
		if (path) openWithDefaultApp(path);
	};
	const handleOpenFolder = () => openFolderForItem(props.item);

	const handleCopy = async () => {
		const path = activeFilePath();
		if (!path) return;
		try {
			if (props.item.itemType === "video") {
				await commands.copyVideoToClipboard(path);
			} else {
				await commands.copyScreenshotToClipboard(path);
			}
			toast.success(t("library.detail.copied"));
		} catch (e) {
			toast.error(String(e));
		}
	};

	const handleCompress = () => {
		const inputPath = props.item.exportedFilePath;
		if (!inputPath) return;

		const progress = new Channel<FramesRendered>((e) => {
			if (e.totalFrames > 0) {
				const pct = Math.round((e.renderedCount / e.totalFrames) * 100);
				toast.loading(`${t("library.detail.compress")}... ${pct}%`, {
					id: "compress-progress",
				});
			}
		});

		toast.loading(`${t("library.detail.compress")}...`, {
			id: "compress-progress",
		});

		invoke("compress_video", { inputPath, crf: 23, progress })
			.then(() => {
				toast.success(t("library.detail.compressComplete"), {
					id: "compress-progress",
				});
				props.onRefetch();
			})
			.catch((e) => {
				toast.error(`${t("library.detail.compressFailed")}: ${e}`, {
					id: "compress-progress",
				});
			});
	};

	const canDeleteSingleVersion = () => {
		if (!hasBothVersions()) return false;
		if (activeVersion() === "original" && props.item.capProjectPath)
			return false;
		return true;
	};

	const handleDeleteVersion = async () => {
		const isOriginal = activeVersion() === "original";
		const message = isOriginal
			? t("library.detail.confirmDeleteOriginal")
			: t("library.detail.confirmDeleteCompressed");

		if (!(await ask(message))) return;

		try {
			const pathToDelete = isOriginal
				? props.item.exportedFilePath
				: props.item.compressedFilePath;

			if (pathToDelete) {
				await commands.deleteLibraryItem(pathToDelete).catch(() => {});
			}

			setActiveVersion(isOriginal ? "compressed" : "original");
			props.onRefetch();
		} catch (error) {
			console.error("Failed to delete:", error);
		}
	};

	const handleDeleteAll = async () => {
		const message =
			props.item.itemType === "video"
				? t("library.detail.confirmDeleteRecording")
				: t("library.detail.confirmDeleteScreenshot");

		if (!(await ask(message))) return;

		try {
			if (props.item.capProjectPath) {
				await commands
					.deleteLibraryItem(props.item.capProjectPath)
					.catch(() => {});
			}
			if (props.item.exportedFilePath) {
				await commands
					.deleteLibraryItem(props.item.exportedFilePath)
					.catch(() => {});
			}
			if (props.item.compressedFilePath) {
				await commands
					.deleteLibraryItem(props.item.compressedFilePath)
					.catch(() => {});
			}
			props.onRefetch();
			props.onClose();
		} catch (error) {
			console.error("Failed to delete item:", error);
		}
	};

	const formattedDate = () => {
		const ts = props.item.createdAt;
		if (!ts) return "";
		const d = new Date(ts * 1000);
		const now = Date.now();
		const diffMs = now - d.getTime();
		const diffMin = Math.floor(diffMs / 60000);
		const diffHour = Math.floor(diffMs / 3600000);
		const diffDay = Math.floor(diffMs / 86400000);

		if (diffMin < 1) return t("library.time.justNow");
		if (diffMin < 60)
			return t("library.time.minutesAgo", { count: String(diffMin) });
		if (diffHour < 24)
			return t("library.time.hoursAgo", { count: String(diffHour) });
		if (diffDay === 1) return t("library.time.yesterday");
		if (diffDay < 7)
			return t("library.time.daysAgo", { count: String(diffDay) });
		return d.toLocaleDateString();
	};

	const formatBytes = (bytes: number | null | undefined) => {
		if (!bytes) return null;
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
		if (bytes < 1024 * 1024 * 1024)
			return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
		return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
	};

	const formattedSize = () => formatBytes(props.item.fileSize);
	const formattedCompressedSize = () =>
		formatBytes(props.item.compressedFileSize);

	const statusLabel = () => {
		if (
			props.item.itemType === "screenshot" &&
			props.item.status === "exportedNoSource"
		) {
			return t("library.status.exported");
		}
		switch (props.item.status) {
			case "editing":
				return t("library.status.editing");
			case "exported":
				return t("library.status.exported");
			case "exportedNoSource":
				return t("library.status.exportedNoSource");
		}
	};

	const deleteVersionLabel = () => {
		return activeVersion() === "original"
			? t("library.detail.deleteOriginal")
			: t("library.detail.deleteCompressed");
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
						when={imageExists() && (previewSrc() || videoPreviewSrc())}
						fallback={
							<div class="w-full h-full flex items-center justify-center">
								{props.item.itemType === "video" ? (
									<IconLucideVideo class="size-12 text-gray-8" />
								) : (
									<IconLucideImage class="size-12 text-gray-8" />
								)}
							</div>
						}
					>
						<Show
							when={!videoPreviewSrc()}
							fallback={
								<video
									src={videoPreviewSrc()}
									preload="metadata"
									muted
									class="w-full h-full object-cover"
									onError={() => setImageExists(false)}
								/>
							}
						>
							<img
								src={previewSrc()}
								alt={props.item.name}
								class="w-full h-full object-cover"
								onError={() => setImageExists(false)}
							/>
						</Show>
					</Show>
				</div>

				<Show when={hasBothVersions()}>
					<div class="flex bg-gray-3 rounded-lg p-1 gap-1">
						<button
							type="button"
							onClick={() => setActiveVersion("original")}
							class={`flex-1 px-3 py-2 rounded-md text-xs font-medium transition-all ${
								activeVersion() === "original"
									? "bg-gray-1 text-gray-12 shadow-sm"
									: "text-gray-11 hover:text-gray-12"
							}`}
						>
							{t("library.detail.original")}
							<Show when={formattedSize()}>
								<span class="ml-1.5 text-gray-10">{formattedSize()}</span>
							</Show>
						</button>
						<button
							type="button"
							onClick={() => setActiveVersion("compressed")}
							class={`flex-1 px-3 py-2 rounded-md text-xs font-medium transition-all ${
								activeVersion() === "compressed"
									? "bg-gray-1 text-gray-12 shadow-sm"
									: "text-gray-11 hover:text-gray-12"
							}`}
						>
							{t("library.detail.compressed")}
							<Show when={formattedCompressedSize()}>
								<span class="ml-1.5 text-gray-10">
									{formattedCompressedSize()}
								</span>
							</Show>
						</button>
					</div>
				</Show>

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

					<Show when={formattedDate()}>
						<div>
							<label class="text-xs font-semibold text-gray-11 uppercase tracking-wider">
								{t("library.detail.createdAt")}
							</label>
							<p class="mt-1 text-sm text-gray-11">{formattedDate()}</p>
						</div>
					</Show>

					<Show when={!hasBothVersions() && formattedSize()}>
						<div>
							<label class="text-xs font-semibold text-gray-11 uppercase tracking-wider">
								{t("library.detail.fileSize")}
							</label>
							<p class="mt-1 text-sm text-gray-11">
								{activeVersion() === "compressed"
									? formattedCompressedSize()
									: formattedSize()}
							</p>
						</div>
					</Show>
				</div>
			</div>

			<div class="border-t border-gray-4 p-4 space-y-3">
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

				<div class="flex gap-2">
					<Show when={canPlay()}>
						<Button
							variant="primary"
							size="md"
							class="flex-1"
							onClick={handlePlay}
						>
							<IconLucidePlay class="size-4" />
							{t("library.detail.play")}
						</Button>
					</Show>
					<Show when={canOpen()}>
						<Button
							variant="primary"
							size="md"
							class="flex-1"
							onClick={handleOpen}
						>
							<IconLucideExternalLink class="size-4" />
							{t("library.detail.open")}
						</Button>
					</Show>
					<Show when={activeFilePath()}>
						<Button
							variant="gray"
							size="md"
							class="flex-1"
							onClick={handleCopy}
						>
							<IconLucideCopy class="size-4" />
							{t("library.detail.copyFile")}
						</Button>
					</Show>
				</div>

				<div class="flex gap-2">
					<Button
						variant="gray"
						size="md"
						class="flex-1"
						onClick={handleOpenFolder}
					>
						<IconLucideFolder class="size-4" />
						{t("library.detail.openFolder")}
					</Button>
					<Show when={canCompress()}>
						<Button
							variant="gray"
							size="md"
							class="flex-1"
							onClick={handleCompress}
						>
							<IconLucideMinimize2 class="size-4" />
							{t("library.detail.compress")}
						</Button>
					</Show>
				</div>

				<div class="flex gap-2 pt-1 border-t border-gray-4">
					<Show when={canDeleteSingleVersion()}>
						<button
							type="button"
							onClick={handleDeleteVersion}
							class="flex-1 flex items-center justify-center gap-1.5 py-2 rounded-lg text-xs text-red-10 hover:bg-red-3 transition-colors"
						>
							<IconLucideTrash class="size-3.5" />
							{deleteVersionLabel()}
						</button>
					</Show>
					<button
						type="button"
						onClick={handleDeleteAll}
						class="flex-1 flex items-center justify-center gap-1.5 py-2 rounded-lg text-xs text-red-10 hover:bg-red-3 transition-colors"
					>
						<IconLucideTrash class="size-3.5" />
						{t("library.detail.delete")}
					</button>
				</div>
			</div>
		</div>
	);
}
