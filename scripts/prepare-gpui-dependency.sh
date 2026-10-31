#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="$repo_root/../gpui-pre-patched"
pins_file="$repo_root/apps/desktop-gpui/patches/gpui-pre.pins"
patch_file="$repo_root/apps/desktop-gpui/patches/gpui-pre-windows.patch"
crates_url="${FRAMEEVER_CRATES_URL:-https://static.crates.io/crates}"
stamp_name=".frameever-prepared"

stamp="$(cat "$pins_file" "$patch_file" | sha256sum | cut -d' ' -f1)"

if [[ -e "$target_dir" ]]; then
	if [[ ! -f "$target_dir/$stamp_name" ]]; then
		echo "error: $target_dir exists but was not prepared by this script" >&2
		exit 1
	fi
	if [[ "$(cat "$target_dir/$stamp_name")" == "$stamp" ]]; then
		exit 0
	fi
fi

temporary_dir="$(mktemp -d "$target_dir.tmp.XXXXXX")"
cleanup() {
	rm -rf "$temporary_dir"
}
trap cleanup EXIT

git init --quiet "$temporary_dir"
git -C "$temporary_dir" config core.autocrlf false
while read -r name version checksum directory; do
	archive="$temporary_dir/$name.crate"
	curl --fail --silent --show-error --location --retry 3 \
		--output "$archive" "$crates_url/$name/$name-$version.crate"
	if [[ "$(sha256sum "$archive" | cut -d' ' -f1)" != "$checksum" ]]; then
		echo "error: $name $version does not match its pinned checksum" >&2
		exit 1
	fi
	mkdir -p "$temporary_dir/crates/$directory"
	tar -xzf "$archive" --strip-components=1 -C "$temporary_dir/crates/$directory"
	rm "$archive"
done < "$pins_file"
git -C "$temporary_dir" apply "$patch_file"
rm -rf "$temporary_dir/.git"
printf '%s' "$stamp" > "$temporary_dir/$stamp_name"

rm -rf "$target_dir"
mv "$temporary_dir" "$target_dir"
trap - EXIT
