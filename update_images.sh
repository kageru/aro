#!/bin/sh

THUMB_PLACEHOLDER='9a2b1a2abad093cd8f923bf3620d1cd6'
FULL_PLACEHOLDER='d07d3b89fec1aebcacc9067a7807165b'
ALT_PLACEHOLDER='48091ea58d7e2a997433d6e3707e84a3'

p=$(pwd)
cd /srv/aro/static
cd thumb
md5sum * | grep -E "($THUMB_PLACEHOLDER|$ALT_PLACEHOLDER)" | awk '{print $2}' | while read i; do echo "deleting $i"; rm "$i"; done
echo "Deleted thumbnail placeholders"
cd ../full
md5sum * | grep -E "($FULL_PLACEHOLDER|$ALT_PLACEHOLDER)" | awk '{print $2}' | while read i; do echo "deleting $i"; rm "$i"; done
echo "Deleted full placeholders"
cd "$p"
for img in $(cat cards.json | rg 'image_url":' | cut -d/ -f6 | cut -d'"' -f1); do
        if [ ! -f "/srv/aro/static/full/$img" ]; then
                echo "Downloading missing full image for $img"
                wget "https://images.ygoprodeck.com/images/cards/$img" --quiet -O "/srv/aro/static/full/$img"
        fi
        if [ ! -f "/srv/aro/static/thumb/$img" ]; then
                echo "Downloading missing full image for $img"
                wget "https://images.ygoprodeck.com/images/cards_small/$img" --quiet -O "/srv/aro/static/thumb/$img"
        fi
done
