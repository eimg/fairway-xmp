#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
"$ROOT/bin/fairway" shutdown
"$ROOT/bin/fairway" clear-domains
"$ROOT/bin/fairway" clear-https-trust
"$ROOT/bin/fairway" remove-cli
"$ROOT/bin/fairway" shutdown
PREFIX=$(dirname -- "$(dirname -- "$ROOT")")
if [ -f "$PREFIX/.fairway-installation" ] && [ -d "$PREFIX/releases" ] && [ -L "$PREFIX/current" ]; then
  case "$ROOT" in "$PREFIX"/releases/*) rm -rf -- "$PREFIX" ;; *) echo 'Installation layout mismatch; files preserved.' >&2; exit 1 ;; esac
else
  printf 'Shell integration removed. Remove this extracted directory manually: %s\n' "$ROOT"
fi
printf 'Fairway XMP removed. Your projects, database files and backups have been preserved.\n'
