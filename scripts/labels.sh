#!/usr/bin/env bash
# Create or correct the issue labels this repository uses. Running it again
# changes nothing.
#
#   scripts/labels.sh

set -euo pipefail
cd "$(dirname "$0")/.."

# name|colour|description
LABELS='finding|fbca04|Something a session saw, to sort
question|d876e3|A choice for the owner (with a recommendation)
needs-pc|0e8a16|Needs the owner'"'"'s Windows PC
retest|c5def5|Re-test list of rows whose behaviour changed
vote|5319e7|Open for votes
answered|0075ca|The owner answered
lane:win|ededed|From the x64 machine
lane:arm|ededed|From the ARM64 machine
lane:cloud|ededed|From a cloud session'

repo=$(git remote get-url origin | sed -E 's#\.git$##; s#.*[:/]([^/:]+/[^/]+)$#\1#')
while IFS='|' read -r name color desc; do
    gh label create "$name" --color "$color" --description "$desc" --repo "$repo" --force >/dev/null
    echo "label: $name"
done <<< "$LABELS"
