#!/usr/bin/env bash
# Refresh the block lists bundled with the installer (brief §5).
#
# Run by a person before a release. The program itself never runs this:
# it only downloads lists when the user presses "actualizar listas".
#
# Output: crates/lists/data/<id>.txt plus MANIFEST.json (url, date, sha256, entries).
# Only lists whose licence allows redistribution in a program that is sold
# (plan Hogar) are fetched here; see docs/LISTS.md.
set -euo pipefail

cd "$(dirname "$0")/../crates/lists/data"

sha() { shasum -a 256 "$1" | cut -d' ' -f1; }
now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# EasyPrivacy (GPL-3.0-or-later / CC BY-SA 3.0). Keep the header and only the
# rules that name a whole domain (`||domain^` with no options): those are the
# only ones a DNS resolver can apply. Path, cosmetic and option rules are dropped.
curl -sSfL "https://easylist.to/easylist/easyprivacy.txt" -o easyprivacy.raw
awk 'BEGIN { header = 1 }
     /^!/ { if (header) print; next }
     /^\|\|[A-Za-z0-9.-]+\^$/ { header = 0; print }' easyprivacy.raw > easyprivacy.txt
rm -f easyprivacy.raw

# Peter Lowe's ad server list, hosts format. Redistribution allowed by the author.
curl -sSfL "https://pgl.yoyo.org/adservers/serverlist.php?hostformat=hosts&showintro=0&mimetype=plaintext" -o peterlowe.txt

count_abp()   { grep -cE '^\|\|' "$1" || true; }
count_hosts() { grep -cE '^(127\.0\.0\.1|0\.0\.0\.0) ' "$1" || true; }

cat > MANIFEST.json <<JSON
{
  "fetched": "$now",
  "lists": [
    {
      "id": "easyprivacy",
      "file": "easyprivacy.txt",
      "url": "https://easylist.to/easylist/easyprivacy.txt",
      "sha256": "$(sha easyprivacy.txt)",
      "entries": $(count_abp easyprivacy.txt)
    },
    {
      "id": "peterlowe",
      "file": "peterlowe.txt",
      "url": "https://pgl.yoyo.org/adservers/serverlist.php?hostformat=hosts&showintro=0&mimetype=plaintext",
      "sha256": "$(sha peterlowe.txt)",
      "entries": $(count_hosts peterlowe.txt)
    }
  ]
}
JSON
echo "bundled lists refreshed at $now"
cat MANIFEST.json
