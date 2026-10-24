#!/usr/bin/env bash

set -euo pipefail


if [[ -t 1 ]] && command -v tput >/dev/null 2>&1 && [[ "$(tput colors 2>/dev/null || echo 0)" -ge 8 ]]; then
  BOLD=$(tput bold); DIM=$(tput dim); RESET=$(tput sgr0)
  BLUE=$(tput setaf 4); GREEN=$(tput setaf 2); YELLOW=$(tput setaf 3)
else
  BOLD=""; DIM=""; RESET=""; BLUE=""; GREEN=""; YELLOW=""
fi

TOTAL_STAGES=0

_STAGE_INDEX=0
ENV_FILE="${ENV_FILE:-.env}"
WRITTEN_ENV=()
WRITTEN_SECRET=()
SKIPPED=()

_clear() {
  [[ -t 1 ]] || return 0
  if command -v tput >/dev/null 2>&1 && tput clear 2>/dev/null; then return 0; fi
  printf '\033[2J\033[3J\033[H'
}

banner() {
  _clear
  printf '\n%s%s  %s%s\n' "$BOLD" "$BLUE" "$1" "$RESET"
  printf '%s  %s stages%s\n\n' "$DIM" "$TOTAL_STAGES" "$RESET"
  printf '%s  You drive the browser; this wizard tells you exactly what to do and\n' "$DIM"
  printf '  captures the values you copy back. Stop any time with Ctrl-C and re-run\n'
  printf '  later, since it remembers values already saved.%s\n' "$RESET"
  pause "Ready to start?"
}

stage() {
  _clear
  _STAGE_INDEX=$((_STAGE_INDEX + 1))
  printf '\n%s%s▸ Stage %s/%s · %s%s\n' \
    "$BOLD" "$BLUE" "$_STAGE_INDEX" "$TOTAL_STAGES" "$1" "$RESET"
}

say()  { printf '  %s\n' "$1"; }
step() { printf '  %s•%s %s\n' "$BLUE" "$RESET" "$1"; }
note() { printf '  %s%s%s\n' "$DIM" "$1" "$RESET"; }
warn() { printf '  %s⚠ %s%s\n' "$YELLOW" "$1" "$RESET"; }

open_url() {
  local url="$1"
  printf '  %s↗ opening%s %s\n' "$GREEN" "$RESET" "$url"
  { if   command -v wslview     >/dev/null 2>&1; then wslview "$url"
    elif command -v explorer.exe >/dev/null 2>&1; then explorer.exe "$url"
    elif command -v xdg-open    >/dev/null 2>&1; then xdg-open "$url"
    elif command -v open        >/dev/null 2>&1; then open "$url"
    else false; fi
  } >/dev/null 2>&1 || warn "couldn't open a browser, so visit it manually: $url"
}

pause() {
  printf '  %s%s%s ' "$DIM" "${1:-Press Enter to continue}" "$RESET"
  read -r _ || true
}

confirm() {
  local reply=""
  printf '  %s? %s [y/N] ' "$YELLOW" "$1"
  read -r reply || true
  [[ "$reply" =~ ^[Yy] ]]
}

_existing() {
  [[ -f "$ENV_FILE" ]] || return 1
  local line value sq="'\\''"; line=$(grep -E "^${1}=" "$ENV_FILE" | tail -n1) || return 1
  value="${line#*=}"
  if [[ "$value" == \'*\' ]]; then value="${value:1:${#value}-2}"; value="${value//"$sq"/\'}"
  elif [[ "$value" == \"*\" ]]; then
    value="${value:1:${#value}-2}"
    [[ "$value" == *[\\\$\"]* ]] && return 1
  fi
  printf '%s' "$value"
}

ask() {
  local key="$1" prompt="$2" current input s=$'\001' e=$'\002' p rc=0
  current=$(_existing "$key" || true)
  p="  $s$BOLD$e$prompt$s$RESET$e "
  [[ -n "$current" ]] && p+="$s$DIM$e[Enter keeps current]$s$RESET$e "
  read -e -r -p "$p" input || rc=1
  if (( rc )) && [[ -z "$input" ]]; then return 1; fi
  [[ -z "$input" && -n "$current" ]] && input="$current"
  printf -v "$key" '%s' "$input"
}

ask_secret() {
  local key="$1" prompt="$2" current input rc=0
  current=$(_existing "$key" || true)
  if [[ -n "$current" ]]; then
    printf '  %s%s%s %s[Enter keeps current]%s ' "$BOLD" "$prompt" "$RESET" "$DIM" "$RESET"
  else
    printf '  %s%s%s ' "$BOLD" "$prompt" "$RESET"
  fi
  read -rs input || rc=1
  printf '\n'
  if (( rc )) && [[ -z "$input" ]]; then return 1; fi
  [[ -z "$input" && -n "$current" ]] && input="$current"
  printf -v "$key" '%s' "$input"
}

write_env() {
  local key="$1" value="$2" tmp sq="'\\''"
  [[ -e "$ENV_FILE" ]] || (umask 077 && : > "$ENV_FILE")
  tmp=$(mktemp)
  grep -vE "^${key}=" "$ENV_FILE" > "$tmp" || true
  printf "%s='%s'\n" "$key" "${value//\'/$sq}" >> "$tmp"
  cat "$tmp" > "$ENV_FILE"
  rm -f "$tmp"
  printf -v "$key" '%s' "$value"
  WRITTEN_ENV+=("$key")
  printf '  %s✓ wrote%s %s → %s\n' "$GREEN" "$RESET" "$key" "$ENV_FILE"
}

set_secret() {
  local name="$1" value="$2"
  if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    if printf '%s' "$value" | gh secret set "$name" >/dev/null 2>&1; then
      WRITTEN_SECRET+=("$name")
      printf '  %s✓ set%s GitHub secret %s\n' "$GREEN" "$RESET" "$name"
      return
    fi
  fi
  SKIPPED+=("GitHub secret $name (set it manually: gh secret set $name)")
  warn "skipped GitHub secret $name: gh not ready; set it later"
}

set_var() {
  local name="$1" value="$2"
  if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    if gh variable set "$name" --body "$value" >/dev/null 2>&1; then
      printf '  %s✓ set%s GitHub variable %s\n' "$GREEN" "$RESET" "$name"
      return
    fi
  fi
  SKIPPED+=("GitHub variable $name")
  warn "skipped GitHub variable $name, gh not ready; set it later"
}

finish() {
  _clear
  printf '\n%s%s  ✓ Setup complete%s\n' "$BOLD" "$GREEN" "$RESET"
  (( ${#WRITTEN_ENV[@]} ))    && note "wrote ${#WRITTEN_ENV[@]} value(s) to $ENV_FILE: ${WRITTEN_ENV[*]}"
  (( ${#WRITTEN_SECRET[@]} )) && note "set ${#WRITTEN_SECRET[@]} GitHub secret(s): ${WRITTEN_SECRET[*]}"
  if (( ${#SKIPPED[@]} )); then
    printf '\n'; warn "still to do by hand:"
    for s in "${SKIPPED[@]}"; do note "  - $s"; done
  fi
  printf '\n'
}


run_wizard() {

TOTAL_STAGES=5

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
KEY_DIR="${FRAMEEVER_KEY_DIR:-$HOME/.frameever}"
KEY_FILE="$KEY_DIR/updater.key"
REPO="AniBullet/FrameEver"
export GH_REPO="$REPO"

banner "FrameEver updater signing key"

stage "Check tools"
say "The key is generated by the Tauri CLI and stored in GitHub with the gh CLI."
if ! pnpm --dir apps/desktop-gpui exec tauri --version >/dev/null 2>&1; then
  warn "The Tauri CLI is not installed. Run 'pnpm install' in $REPO_ROOT, then re-run this wizard."
  exit 1
fi
step "Tauri CLI: $(pnpm --dir apps/desktop-gpui exec tauri --version)"
until gh auth status >/dev/null 2>&1; do
  warn "gh is not signed in."
  step "In another terminal run: gh auth login"
  pause "Press Enter once gh is signed in"
done
step "gh is signed in; secrets go to $REPO"
pause

stage "Choose the key password"
say "The private key is encrypted with this password. CI needs both to sign installers."
note "Use a long random password and keep it in your password manager."
while true; do
  ask_secret KEY_PASSWORD "Key password:"
  ask_secret KEY_PASSWORD_AGAIN "Repeat it:"
  if [[ -z "$KEY_PASSWORD" ]]; then
    warn "The password can't be empty."
  elif [[ "$KEY_PASSWORD" != "$KEY_PASSWORD_AGAIN" ]]; then
    warn "The passwords don't match."
  else
    break
  fi
done

stage "Generate the key pair"
mkdir -p "$KEY_DIR"
FORCE=()
if [[ -f "$KEY_FILE" ]]; then
  warn "A key already exists at $KEY_FILE."
  note "Replacing it means every installed copy built with the old public key can no longer update."
  if confirm "Replace the existing key?"; then
    FORCE=(--force)
  else
    say "Keeping the existing key; checking that the password unlocks it."
  fi
fi
if [[ ${#FORCE[@]} -gt 0 || ! -f "$KEY_FILE" ]]; then
  pnpm --dir apps/desktop-gpui exec tauri signer generate --ci -p "$KEY_PASSWORD" -w "$KEY_FILE" "${FORCE[@]}" >/dev/null
fi
if [[ ! -s "$KEY_FILE" || ! -s "$KEY_FILE.pub" ]]; then
  warn "Key generation failed; nothing was written to $KEY_DIR."
  exit 1
fi
PROBE="$(mktemp)"
printf 'frameever' > "$PROBE"
if ! pnpm --dir apps/desktop-gpui exec tauri signer sign -f "$KEY_FILE" -p "$KEY_PASSWORD" "$PROBE" >/dev/null 2>&1; then
  rm -f "$PROBE" "$PROBE.sig"
  warn "The password does not unlock $KEY_FILE. Re-run the wizard with that key's password."
  exit 1
fi
rm -f "$PROBE" "$PROBE.sig"
printf '  %s✓ private key%s %s\n' "$GREEN" "$RESET" "$KEY_FILE"
printf '  %s✓ public key%s  %s\n' "$GREEN" "$RESET" "$KEY_FILE.pub"
pause

stage "Back up the key"
say "If the private key or its password is lost, installed copies can never update again."
step "Copy $KEY_FILE into your password manager (as a file attachment or secure note)."
step "Save the key password in the same entry."
note "Never commit the private key. The public key is not secret."
until confirm "Is the key and its password backed up?"; do
  warn "Back it up before continuing."
done

stage "Store the key in GitHub"
say "The release workflow signs installers with the secrets and builds the public key into the app."
set_secret TAURI_SIGNING_PRIVATE_KEY "$(cat "$KEY_FILE")"
set_secret TAURI_SIGNING_PRIVATE_KEY_PASSWORD "$KEY_PASSWORD"
set_var FRAMEEVER_UPDATER_PUBKEY "$(cat "$KEY_FILE.pub")"
open_url "https://github.com/$REPO/settings/secrets/actions"
step "Check that both secrets and the variable are listed on the Secrets and Variables pages."
pause

finish
say "Next: push a version tag (for example: pnpm release 3.0.0); the release workflow builds, signs and publishes it."
printf '\n'
}

run_wizard "$@"; exit
