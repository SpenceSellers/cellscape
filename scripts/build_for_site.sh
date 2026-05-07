#!/bin/sh

# This is really just meant for me, sorry
set -e
trunk build --release
rm ~/Repos/infra/spencesellerscom/hugo-site/static/cellscape/*
cp dist/* ~/Repos/infra/spencesellerscom/hugo-site/static/cellscape/