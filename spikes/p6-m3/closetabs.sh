#!/bin/sh
# close every Chrome tab but the first (DevTools HTTP; page.close hangs on Android Chrome)
for id in $(curl -s http://127.0.0.1:9222/json/list | grep '"id"' | sed 's/.*: "\(.*\)".*/\1/' | tail -n +2); do curl -s http://127.0.0.1:9222/json/close/$id >/dev/null; done
