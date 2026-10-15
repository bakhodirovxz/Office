#!/bin/bash
# poll the Android HRM tab until the editor is ready
for i in $(seq 1 50); do s=$(node cdp-eval.js "document.body.dataset.state + ' ' + (window.__readyMs||'')" 2>&1); case $s in *ready*|*error*) echo $s; exit 0;; esac; sleep 5; done; echo timeout $s
