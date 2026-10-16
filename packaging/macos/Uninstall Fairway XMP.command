#!/bin/sh
set -eu
APP='/Applications/Fairway XMP.app'
EXE="$APP/Contents/MacOS/fairway"
KEEP_ALL=no
case "${1:-}" in
  '') ;;
  --yes-keep-data) KEEP_ALL=yes ;;
  *) echo 'Usage: Uninstall Fairway XMP.command [--yes-keep-data]' >&2; exit 1 ;;
esac

if [ ! -x "$EXE" ]; then
  echo 'Fairway XMP is not installed in /Applications.'
  exit 1
fi
if ! "$EXE" --prepare-uninstall; then
  if [ "$KEEP_ALL" = yes ]; then echo 'Stop Fairway XMP and its services before uninstalling.' >&2; exit 1; fi
  /usr/bin/osascript -e 'display alert "Stop Fairway XMP before uninstalling" message "Open Fairway XMP, stop all services, and quit. Then run this uninstaller again."'
  exit 1
fi
if [ "$KEEP_ALL" = yes ]; then
  "$EXE" --remove-domains
  "$EXE" --remove-https-trust
  "$EXE" --remove-cli
  mkdir -p "$HOME/.Trash"
  TRASH_ROOT=$(mktemp -d "$HOME/.Trash/Fairway-XMP-removed.XXXXXX")
  # A writable /Applications can still restrict removal of root-owned entries.
  # Test the move itself; keep profile cleanup in the student's account and
  # request administrator access only if moving the application requires it.
  if ! /bin/mv "$APP" "$TRASH_ROOT/Fairway XMP.app"; then
    /usr/bin/sudo /bin/mv "$APP" "$TRASH_ROOT/Fairway XMP.app"
  fi
  echo 'Fairway XMP moved to Trash. Projects, databases and backups have been kept.'
  exit 0
fi
CONFIRM=$(/usr/bin/osascript -e 'button returned of (display dialog "Uninstall Fairway XMP and remove its command-line integration? Your projects and databases are kept unless you choose to delete them next." buttons {"Cancel", "Uninstall"} default button "Cancel")')
[ "$CONFIRM" = Uninstall ] || exit 0
"$EXE" --remove-domains
"$EXE" --remove-https-trust
"$EXE" --remove-cli
DATA=$(/usr/bin/osascript -e 'button returned of (display dialog "Delete Fairway XMP databases, backups, settings, and caches?" buttons {"Keep data", "Delete data"} default button "Keep data")')
if [ "$DATA" = 'Delete data' ]; then "$EXE" --remove-data; fi
COURSEWORK=$(/usr/bin/osascript -e 'button returned of (display dialog "Permanently delete every project in your fairway/www folder?" buttons {"Keep projects", "Delete projects"} default button "Keep projects")')
if [ "$COURSEWORK" = 'Delete projects' ]; then "$EXE" --remove-coursework; fi
# Finder requests authorization when the installed application is root-owned.
/usr/bin/osascript -e 'tell application "Finder" to delete POSIX file "/Applications/Fairway XMP.app"'
echo 'Fairway XMP uninstalled. Shared system runtimes have been kept.'
