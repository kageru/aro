#!/bin/sh

cd /home/aro/aro
wget -O cardinfo.php 'https://db.ygoprodeck.com/api/v7/cardinfo.php?misc=yes&format=genesys' &> /dev/null
cat cardinfo.php | jq . > cards.json
/home/aro/update_images.sh
rm cardinfo.php
wget https://db.ygoprodeck.com/api/v7/cardsets.php &> /dev/null
cat cardsets.php | jq . > sets.json
rm cardsets.php
