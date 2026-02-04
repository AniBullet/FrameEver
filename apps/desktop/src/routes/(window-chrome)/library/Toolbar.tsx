import { useI18n } from "~/i18n";
import { Input } from "~/routes/editor/ui";
import IconLucideAlignJustify from "~icons/lucide/align-justify";
import IconLucideGrid3x3 from "~icons/lucide/grid-3x3";
import IconLucideList from "~icons/lucide/list";
import IconLucideRefreshCw from "~icons/lucide/refresh-cw";
import IconLucideSearch from "~icons/lucide/search";

type ViewMode = "grid" | "list" | "compact";

type Props = {
	searchQuery: string;
	onSearchChange: (query: string) => void;
	viewMode: ViewMode;
	onViewModeChange: (mode: ViewMode) => void;
	onRefresh: () => void;
};

export default function Toolbar(props: Props) {
	const { t } = useI18n();

	return (
		<div class="h-16 border-b border-gray-4 flex items-center px-6 bg-gray-1 gap-4">
			<div class="relative flex-1 max-w-md">
				<IconLucideSearch class="absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none size-4 text-gray-10" />
				<Input
					type="search"
					class="pl-10 h-10 w-full"
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
						title="网格视图"
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
						title="列表视图"
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
						title="紧凑视图"
					>
						<IconLucideAlignJustify class="size-4" />
					</button>
				</div>

				<button
					type="button"
					onClick={props.onRefresh}
					class="p-2 rounded-lg hover:bg-gray-3 transition-colors text-gray-10 hover:text-gray-12"
					title="刷新"
				>
					<IconLucideRefreshCw class="size-4" />
				</button>
			</div>
		</div>
	);
}
