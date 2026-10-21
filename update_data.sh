#!/bin/sh

set -eu

log() {
    printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*" >&2
}

cd /home/aro/aro
log 'Downloading YGOPRODeck card data'
wget -O cardinfo.php 'https://db.ygoprodeck.com/api/v7/cardinfo.php?misc=yes&format=genesys' >/dev/null 2>&1
jq . cardinfo.php > cards.json
log 'Updating card images'
/home/aro/update_images.sh
rm cardinfo.php
log 'Downloading YGOPRODeck set data'
wget https://db.ygoprodeck.com/api/v7/cardsets.php >/dev/null 2>&1
jq . cardsets.php > sets.json
rm cardsets.php

wiki_api='https://yugipedia.com/api.php'
wiki_query='[[Category:OCG/TCG cards]][[Actions::+]]'
wiki_limit=${WIKI_LIMIT:-1000}
wiki_pages=$(mktemp -d .wiki-pages.XXXXXXXX)
trap 'rm -rf "$wiki_pages"' 0
trap 'exit 1' HUP INT TERM

log "Downloading Yugipedia tags (batch limit=$wiki_limit)"
for wiki_range in '[[≤G]]' '[[≥G]][[≤M]]' '[[≥M]][[≤S]]' '[[≥S]]'; do
    wiki_offset=0
    while :; do
        page="$wiki_pages/$wiki_range-$wiki_offset.json"
        log "Yugipedia: $wiki_range offset=$wiki_offset — requesting batch"
        curl --fail --silent --show-error --retry 3 --connect-timeout 15 --max-time 90 --get "$wiki_api" \
            --user-agent 'aro-data-updater/1.0 (https://github.com/kageru/aro)' \
            --data-urlencode 'action=ask' \
            --data-urlencode 'format=json' \
            --data-urlencode 'api_version=3' \
            --data-urlencode "query=$wiki_query$wiki_range|?Actions|limit=$wiki_limit|offset=$wiki_offset" \
            --output "$page.part"
        jq -e --argjson offset "$wiki_offset" '
            if (.error == null and .errors == null and .query.errors == null
                and .query.meta.offset == $offset and (.query.results | type == "array")
                and all(.query.results[][]; (.printouts.Actions | type == "array")
                    and all(.printouts.Actions[]; .fulltext | type == "string"))
                and ((has("query-continue-offset") | not)
                    or (."query-continue-offset" | type == "number" and . > $offset)))
            then true else error("Invalid Yugipedia response or non-advancing pagination") end
        ' "$page.part" >/dev/null
        mv "$page.part" "$page"
        log "Yugipedia: $wiki_range offset=$wiki_offset — batch saved and validated"

        sleep 1
        wiki_offset=$(jq -r '."query-continue-offset" // empty' "$page")
        [ -n "$wiki_offset" ] || break
    done
    log "Yugipedia: $wiki_range complete"
done

log 'Merging Yugipedia batches'
jq -es '
    [.[].query.results[][] | {key: .fulltext, value: [.printouts.Actions[].fulltext]}]
    | from_entries | select(length > 0)
' "$wiki_pages"/*.json > "$wiki_pages/actions.tmp"
mv "$wiki_pages/actions.tmp" actions.json
log "Data update complete: $(jq 'length' actions.json) cards with tags"
