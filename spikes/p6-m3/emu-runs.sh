#!/bin/sh
# usage: SER=emulator-5554 emu-runs.sh <tag-prefix> [only]: 3x control then 3x 108-page against the emulator already forwarded on 9222.
# Each run is retried (hoshiya-serve sometimes needs more than the script's 15 s to start on a loaded machine).
SER=${SER:-emulator-5554}
F=domains/docs/crates/docs-translit/tests/fixtures
run() { # tag file timeout
  for try in 1 2 3 4; do
    sh spikes/p6-m3/closetabs.sh
    node scripts/mobile-m3.mjs --cdp --serial $SER --dist E:/office-target/p5/dist --file $2 --tag $1 --out docs/phase6/evidence/m3 --timeout $3 > E:/office-target/tmp/$1.json 2>&1 && return
  done
}
for i in 1 2 3; do [ -s E:/office-target/tmp/$1-small-$i.json ] && grep -q '"opened"' E:/office-target/tmp/$1-small-$i.json || run $1-small-$i $F/word/01-split-runs.docx 90; done
for i in 1 2 3; do [ -s E:/office-target/tmp/$1-big-$i.json ] && grep -q '"opened"' E:/office-target/tmp/$1-big-$i.json || run $1-big-$i $F/uz108-latn.docx 400; done
echo done > E:/office-target/tmp/$1-done2
