import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { commands, type LibraryItem } from "~/utils/tauri";

export function openWithDefaultApp(path: string) {
	invoke("open_path_with_default_app", { path }).catch((e) =>
		console.error("open_path_with_default_app failed:", e),
	);
}

export function canPlay(item: LibraryItem) {
	return item.exportedFilePath !== null && item.itemType === "video";
}

export function canOpen(item: LibraryItem) {
	return item.exportedFilePath !== null && item.itemType === "screenshot";
}

export function canEdit(item: LibraryItem) {
	return item.capProjectPath !== null;
}

export function executePrimaryAction(item: LibraryItem): boolean {
	if (canPlay(item) && item.exportedFilePath) {
		openWithDefaultApp(item.exportedFilePath);
		return true;
	}
	if (canOpen(item) && item.exportedFilePath) {
		openWithDefaultApp(item.exportedFilePath);
		return true;
	}
	if (canEdit(item) && item.capProjectPath) {
		if (item.itemType === "video") {
			commands.showWindow({
				Editor: { project_path: item.capProjectPath },
			});
		} else {
			commands.showWindow({
				ScreenshotEditor: { path: item.capProjectPath },
			});
		}
		return true;
	}
	return false;
}

export function openFolder(item: LibraryItem) {
	const isExported =
		item.status === "exported" || item.status === "exportedNoSource";
	if (isExported && item.exportedFilePath) {
		revealItemInDir(item.exportedFilePath);
	} else if (item.capProjectPath) {
		revealItemInDir(item.capProjectPath);
	} else if (item.exportedFilePath) {
		revealItemInDir(item.exportedFilePath);
	}
}

export function editItem(item: LibraryItem) {
	if (!item.capProjectPath) return;
	if (item.itemType === "video") {
		commands.showWindow({
			Editor: { project_path: item.capProjectPath },
		});
	} else {
		commands.showWindow({
			ScreenshotEditor: { path: item.capProjectPath },
		});
	}
}
