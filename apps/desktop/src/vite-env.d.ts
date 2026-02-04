/// <reference types="vinxi/types/client" />

interface ImportMetaEnv {
	readonly VITE_SERVER_URL: string;
	// more env variables...
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
