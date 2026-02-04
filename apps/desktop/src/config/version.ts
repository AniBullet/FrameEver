// Cap aniBullet 版本配置
export const VERSION_INFO = {
	edition: "aniBullet",
	fullName: "Cap - aniBullet",
	tagline: "专业录屏工具",
	features: ["无限制录制", "本地存储", "AI 智能字幕", "高质量导出"],
	colors: {
		primary: "#3b82f6",
		accent: "#8b5cf6",
	},
} as const;

export type VersionInfo = typeof VERSION_INFO;
