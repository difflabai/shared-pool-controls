#!/usr/bin/env bash
# Checks that every cover is SATISFIED in at least one harness of its family,
# across the covers.tsv files run-kani.sh writes (family, description,
# status). CI runs the harnesses of a family in separate jobs and calls this
# once over all of their files.
#
# usage: tools/scripts/check-covers.sh covers.tsv...
set -uo pipefail
[ $# -gt 0 ] || { echo "usage: check-covers.sh covers.tsv..." >&2; exit 2; }
missing=$(cat "$@" | awk -F'\t' '{k=$1 "\t" $2; if ($3=="SATISFIED") s[k]=1; else if (!(k in s)) s[k]=s[k]+0}
                                 END {for (k in s) if (!s[k]) print k}')
if [ -n "$missing" ]; then
  echo "unsatisfied covers:"; echo "$missing"
  [ -n "${GITHUB_ACTIONS:-}" ] && echo "::error title=unsatisfied covers::$(echo "$missing" | tr '\t\n' ': ' )"
  exit 1
fi
echo "covers: $(cat "$@" | cut -f1,2 | sort -u | wc -l) checked, all satisfied"
