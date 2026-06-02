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
import IconLucideLoader from "~icons/lucide/loader-2";
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
		!hasCompressed() &&
		!props.item.isCompressing;

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

	const handleDeleteOriginal = async () => {
		const hasProject = !!props.item.capProjectPath;
		const message = hasProject
			? t("library.detail.confirmDeleteOriginalWithProject")
			: t("library.detail.confirmDeleteOriginal");

		if (!(await ask(message))) return;

		try {
			if (hasProject && props.item.capProjectPath) {
				await commands.deleteLibraryItem(props.item.capProjectPath);
			}
			if (props.item.exportedFilePath) {
				await commands.deleteLibraryItem(props.item.exportedFilePath);
			}
			if (hasCompressed()) {
				setActiveVersion("compressed");
			}
			props.onRefetch();
			if (!hasCompressed()) {
				props.onClose();
			}
		} catch (error) {
			toast.error(t("library.detail.deleteFailedInUse"));
		}
	};

	const handleDeleteCompressed = async () => {
		if (!(await ask(t("library.detail.confirmDeleteCompressed")))) return;

		try {
			if (props.item.compressedFilePath) {
				await commands.deleteLibraryItem(props.item.compressedFilePath);
			}
			if (hasOriginal()) {
				setActiveVersion("original");
			}
			props.onRefetch();
			if (!hasOriginal()) {
				props.onClose();
			}
		} catch (error) {
			toast.error(t("library.detail.deleteFailedInUse"));
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
				await commands.deleteLibraryItem(props.item.capProjectPath);
			}
			if (props.item.exportedFilePath) {
				await commands.deleteLibraryItem(props.item.exportedFilePath);
			}
			if (props.item.compressedFilePath) {
				await commands.deleteLibraryItem(props.item.compressedFilePath);
			}
			props.onRefetch();
			props.onClose();
		} catch (error) {
			toast.error(t("library.detail.deleteFailedInUse"));
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

	const isCompressing = () => !!props.item.isCompressing;

	return (
		<div class="w-96 bg-gray-2 border-l border-gray-4 flex flex-col">
			<div class="h-11 border-b border-gray-4 flex items-center justify-between px-4">
				<span class="text-sm font-semibold text-gray-12">
					{t("library.detail.title")}
				</span>
				<button
					type="button"
					onClick={props.onClose}
					class="p-1 rounded-md hover:bg-gray-3 transition-colors"
					aria-label={t("library.detail.close")}
				>
					<IconLucideX class="size-3.5 text-gray-11" />
				</button>
			</div>

			<Show when={imageExists() && (previewSrc() || videoPreviewSrc())}>
				<div class="px-4 pt-4 pb-2 shrink-0">
					<div class="aspect-video w-full bg-gray-3 rounded-lg overflow-hidden">
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
					</div>
				</div>
			</Show>

			<div class="flex-1 overflow-auto custom-scroll px-4 pb-4 space-y-4">
				<Show when={hasBothVersions() || isCompressing()}>
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
						<Show
							when={!isCompressing() || hasCompressed()}
							fallback={
								<div class="flex-1 px-3 py-2 rounded-md text-xs font-medium text-orange-10 flex items-center justify-center gap-1.5">
									<IconLucideLoader class="size-3 animate-spin" />
									{t("library.detail.compressing")}
								</div>
							}
						>
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
						</Show>
					</div>
				</Show>

				<div class="space-y-3">
					<p class="text-base font-semibold text-gray-12 break-words leading-snug">
						{props.item.name}
					</p>

					<div class="rounded-lg bg-gray-3 overflow-hidden divide-y divide-gray-4">
						<div class="flex items-center justify-between px-3 py-2.5">
							<span class="text-sm text-gray-10">
								{t("library.detail.type")}
							</span>
							<span class="text-sm font-medium text-gray-12">
								{props.item.itemType === "video"
									? t("library.type.video")
									: t("library.type.screenshot")}
							</span>
						</div>

						<div class="flex items-center justify-between px-3 py-2.5">
							<span class="text-sm text-gray-10">
								{t("library.detail.status")}
							</span>
							<span class="flex items-center gap-1.5 text-sm font-medium">
								<Show when={props.item.status === "editing"}>
									<IconLucideEdit class="size-3.5 text-orange-500 shrink-0" />
									<span class="text-orange-500">{statusLabel()}</span>
								</Show>
								<Show
									when={
										props.item.status === "exported" ||
										props.item.status === "exportedNoSource"
									}
								>
									<IconLucideCheckCircle class="size-3.5 text-green-500 shrink-0" />
									<span class="text-green-500">{statusLabel()}</span>
								</Show>
							</span>
						</div>

						<Show when={formattedDate()}>
							<div class="flex items-center justify-between px-3 py-2.5">
								<span class="text-sm text-gray-10">
									{t("library.detail.createdAt")}
								</span>
								<span class="text-sm font-medium text-gray-12">
									{formattedDate()}
								</span>
							</div>
						</Show>

						<Show when={!hasBothVersions() && formattedSize()}>
							<div class="flex items-center justify-between px-3 py-2.5">
								<span class="text-sm text-gray-10">
									{t("library.detail.fileSize")}
								</span>
								<span class="text-sm font-medium text-gray-12">
									{activeVersion() === "compressed"
										? formattedCompressedSize()
										: formattedSize()}
								</span>
							</div>
						</Show>
					</div>
				</div>
			</div>

			<div class="border-t border-gray-4 p-3 space-y-1.5">
				<Show when={canEdit()}>
					<Button
						variant="primary"
						size="sm"
						class="w-full"
						onClick={handleEdit}
					>
						<IconLucideEdit class="size-3.5 shrink-0" />
						{t("library.detail.edit")}
					</Button>
				</Show>

				<div class="grid grid-cols-4 gap-1">
					<button
						type="button"
						onClick={canPlay() ? handlePlay : handleOpen}
						disabled={!canPlay() && !canOpen()}
						class="flex flex-col items-center gap-1 py-2.5 rounded-lg transition-colors text-gray-11 enabled:hover:text-gray-12 enabled:hover:bg-gray-3 disabled:opacity-25 disabled:cursor-default"
					>
						<Show
							when={canPlay()}
							fallback={<IconLucideExternalLink class="size-4" />}
						>
							<IconLucidePlay class="size-4" />
						</Show>
						<span class="text-[10px] leading-none">
							{canPlay() ? t("library.detail.play") : t("library.detail.open")}
						</span>
					</button>
					<button
						type="button"
						onClick={handleCopy}
						disabled={!activeFilePath()}
						class="flex flex-col items-center gap-1 py-2.5 rounded-lg transition-colors text-gray-11 enabled:hover:text-gray-12 enabled:hover:bg-gray-3 disabled:opacity-25 disabled:cursor-default"
					>
						<IconLucideCopy class="size-4" />
						<span class="text-[10px] leading-none">
							{t("library.detail.copy")}
						</span>
					</button>
					<button
						type="button"
						onClick={handleCompress}
						disabled={!canCompress()}
						class="flex flex-col items-center gap-1 py-2.5 rounded-lg transition-colors text-gray-11 enabled:hover:text-gray-12 enabled:hover:bg-gray-3 disabled:opacity-25 disabled:cursor-default"
					>
						<IconLucideMinimize2 class="size-4" />
						<span class="text-[10px] leading-none">
							{t("library.detail.compress")}
						</span>
					</button>
					<button
						type="button"
						onClick={handleOpenFolder}
						class="flex flex-col items-center gap-1 py-2.5 rounded-lg text-gray-11 hover:text-gray-12 hover:bg-gray-3 transition-colors"
					>
						<IconLucideFolder class="size-4" />
						<span class="text-[10px] leading-none">
							{t("library.detail.openFolder")}
						</span>
					</button>
				</div>

				<div class="flex items-center gap-1 pt-1">
					<Show when={hasBothVersions()}>
						<button
							type="button"
							onClick={handleDeleteOriginal}
							class="flex-1 flex items-center justify-center gap-1 py-1.5 rounded-md text-[11px] text-gray-11 hover:text-red-10 hover:bg-red-3 transition-colors"
						>
							<IconLucideTrash class="size-3" />
							{t("library.detail.deleteOriginal")}
						</button>
						<span class="text-gray-6">|</span>
						<button
							type="button"
							onClick={handleDeleteCompressed}
							disabled={isCompressing()}
							class="flex-1 flex items-center justify-center gap-1 py-1.5 rounded-md text-[11px] text-gray-11 hover:text-red-10 hover:bg-red-3 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
						>
							<IconLucideTrash class="size-3" />
							{t("library.detail.deleteCompressed")}
						</button>
						<span class="text-gray-6">|</span>
					</Show>
					<button
						type="button"
						onClick={handleDeleteAll}
						class="flex-1 flex items-center justify-center gap-1 py-1.5 rounded-md text-[11px] text-red-10 hover:bg-red-3 transition-colors font-medium"
					>
						<IconLucideTrash class="size-3" />
						{hasBothVersions()
							? t("library.detail.deleteAll")
							: t("library.detail.delete")}
					</button>
				</div>
			</div>
		</div>
	);
}
