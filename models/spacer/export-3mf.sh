#!/usr/bin/env bash
set -e

openscad \
    -Douter=30.0 -Dinner=12.0 -Dheight=10.0 \
    -o spacer.3mf \
    api.scad
