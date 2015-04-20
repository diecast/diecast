#!/usr/bin/env bash

trap 'exit' INT TERM
trap 'kill 0' EXIT

# web server
mkdir -p target/doc
cd target/doc
python2 -m SimpleHTTPServer 4040 &

# regen docs
cd ../..

while true; do
  inotifywait -e close_write,delete_self -r src/ && cargo doc;
done

