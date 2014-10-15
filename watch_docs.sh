#!/usr/bin/env bash

while true; do
  find src -iname \*rs |\
    inotifywait -e close_write,delete_self --fromfile - && cargo doc;
done
