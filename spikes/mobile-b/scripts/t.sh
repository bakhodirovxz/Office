source /c/Users/uge226/Desktop/office/spikes/mobile-b/scripts/env.sh
cd /c/Users/uge226/Desktop/office/spikes/mobile-b/scripts
tailtxt() { node cdp-eval.js "JSON.stringify({tail:__wc.ed.plain_text().slice(-70), types:__perf.inputTypes.slice(-14), ev:__perf.events.slice(-8).map(e=>e.slice(1).join(' '))})"; }
