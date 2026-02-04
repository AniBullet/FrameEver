import type { Component } from "solid-js";
import { useI18n } from "~/i18n";

export const VersionBadge: Component = () => {
	const { t } = useI18n();

	return (
		<div class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-blue-50 dark:bg-blue-950/30 border border-blue-200/50 dark:border-blue-800/50">
			<span class="text-[10px] font-medium text-blue-700 dark:text-blue-400">
				{t("app.version.badge")}
			</span>
		</div>
	);
};
