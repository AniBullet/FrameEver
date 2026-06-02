import { Show } from "solid-js";
import { useI18n } from "~/i18n";
import { Input } from "~/routes/editor/ui";
import IconLucideAlignJustify from "~icons/lucide/align-justify";
import IconLucideFolderOpen from "~icons/lucide/folder-open";
import IconLucideGrid3x3 from "~icons/lucide/grid-3x3";
import IconLucideList from "~icons/lucide/list";
import IconLucideMinimize2 from "~icons/lucide/minimize-2";
import IconLucideRefreshCw from "~icons/lucide/refresh-cw";
import IconLucideSearch from "~icons/lucide/search";
import IconLucideTrash from "~icons/lucide/trash-2";

type ViewMode = "grid" | "list" | "compact";

type Props = {
	searchQuery: string;
	onSearchChange: (query: string) => void;
	viewMode: ViewMode;
	onViewModeChange: (mode: ViewMode) => void;
	onRefresh: () => void;
	onOpenLibraryFolder: () => void;
	selectedCount: number;
	onBatchDelete?: () => void;
	onBatchCompress?: () => void;
};

export default function Toolbar(props: Props) {
	const { t } = useI18n();

	return (
		<div class="h-14 border-b border-gray-4 flex items-center px-5 bg-gray-1 gap-4">
			<div class="relative flex-1 max-w-md">
				<IconLucideSearch class="absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none size-4 text-gray-10" />
				<Input
					type="search"
					class="pl-10 h-9 w-full"
					value={props.searchQuery}
					onInput={(e) => props.onSearchChange(e.currentTarget.value)}
					onKeyDown={(e) => {
						if (e.key === "Escape" && props.searchQuery) {
							e.preventDefault();
							props.onSearchChange("");
						}
					}}
					placeholder={t("library.toolbar.search")}
					autoCapitalize="off"
					autocorrect="off"
					autocomplete="off"
					spellcheck={false}
				/>
			</div>

			<Show when={props.selectedCount > 0}>
				<div class="flex items-center gap-1 px-2 py-1 rounded-lg bg-gray-3 border border-gray-4">
					<span class="text-xs text-gray-11 px-1">
						{t("library.batch.selected", {
							count: String(props.selectedCount),
						})}
					</span>
					<Show when={props.onBatchCompress}>
						<button
							type="button"
							onClick={() => props.onBatchCompress?.()}
							class="p-1.5 rounded-md hover:bg-gray-4 transition-colors text-gray-11 hover:text-gray-12"
							title={t("library.detail.compress")}
						>
							<IconLucideMinimize2 class="size-3.5" />
						</button>
					</Show>
					<button
						type="button"
						onClick={() => props.onBatchDelete?.()}
						class="p-1.5 rounded-md hover:bg-red-3 transition-colors text-gray-11 hover:text-red-11"
						title={t("library.batch.delete")}
					>
						<IconLucideTrash class="size-3.5" />
					</button>
				</div>
			</Show>

			<div class="flex items-center gap-2">
				<div class="flex items-center bg-gray-3 rounded-lg p-1">
					<button
						type="button"
						onClick={() => props.onViewModeChange("grid")}
						class={`p-2 rounded transition-colors ${
							props.viewMode === "grid"
								? "bg-gray-5 text-gray-12"
								: "text-gray-10 hover:text-gray-12"
						}`}
						title={t("library.toolbar.view.grid")}
					>
						<IconLucideGrid3x3 class="size-4" />
					</button>
					<button
						type="button"
						onClick={() => props.onViewModeChange("list")}
						class={`p-2 rounded transition-colors ${
							props.viewMode === "list"
								? "bg-gray-5 text-gray-12"
								: "text-gray-10 hover:text-gray-12"
						}`}
						title={t("library.toolbar.view.list")}
					>
						<IconLucideList class="size-4" />
					</button>
					<button
						type="button"
						onClick={() => props.onViewModeChange("compact")}
						class={`p-2 rounded transition-colors ${
							props.viewMode === "compact"
								? "bg-gray-5 text-gray-12"
								: "text-gray-10 hover:text-gray-12"
						}`}
						title={t("library.toolbar.view.compact")}
					>
						<IconLucideAlignJustify class="size-4" />
					</button>
				</div>

				<button
					type="button"
					onClick={props.onRefresh}
					class="p-2 rounded-lg hover:bg-gray-3 transition-colors text-gray-10 hover:text-gray-12"
					title={t("library.toolbar.refresh")}
				>
					<IconLucideRefreshCw class="size-4" />
				</button>
				<button
					type="button"
					onClick={props.onOpenLibraryFolder}
					class="p-2 rounded-lg hover:bg-gray-3 transition-colors text-gray-10 hover:text-gray-12"
					title={t("library.toolbar.openFolder")}
				>
					<IconLucideFolderOpen class="size-4" />
				</button>
			</div>
		</div>
	);
}
