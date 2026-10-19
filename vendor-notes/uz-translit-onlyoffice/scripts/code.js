// Background plugin: adds "to Latin" / "to Cyrillic" to the context menu and the
// plugin toolbar, and rewrites the current selection in place.
(function (window) {
	'use strict';

	var TO_LATIN = 'cyr';     // source script for convert()
	var TO_CYRILLIC = 'lat';
	var LABELS = {
		cyr: { progress: 'Lotinga o‘girilmoqda…', source: /[Ѐ-ӿ]/, none: 'Belgilangan matnda kiril harflari yo‘q' },
		lat: { progress: 'Kirillga o‘girilmoqda…', source: /[A-Za-z]/, none: 'Belgilangan matnda lotin harflari yo‘q' }
	};
	var NOTICE_MS = 10000;
	var LARGE_CHARS = 20000;

	// Every paragraph (table cells included) on its own line, so the result can be
	// handed back to ReplaceTextSmart one string per paragraph.
	var SELECTION_OPTIONS = [{
		Numbering: false,
		Math: false,
		TableCellSeparator: '\n',
		TableRowSeparator: '\n',
		ParaSeparator: '\n',
		TabSymbol: '\t',
		NewLineSeparator: '\r'
	}];

	var busy = false;
	var status = null;

	function plugin() { return window.Asc.plugin; }

	// callCommand ships Asc.scope inside the command source, and the editor rejects
	// any command whose source contains "import " ("static import is not allowed in
	// macros") - which real text such as "import hisobiga" does. Hex keeps every
	// word out of the source.
	function toHex(text) {
		var out = '';
		for (var i = 0; i < text.length; i++) {
			out += ('000' + text.charCodeAt(i).toString(16)).slice(-4);
		}
		return out;
	}

	// All messages go to the status bar. A "Block" action is not an option: while
	// it is open ReplaceTextSmart silently changes nothing (measured on DS 9.3.1).
	function showStatus(text, autoHide) {
		if (status) {
			plugin().executeMethod('EndAction', ['Information', status]);
		}
		status = text;
		plugin().executeMethod('StartAction', ['Information', text]);
		if (autoHide) {
			setTimeout(function () {
				if (status === text) {
					plugin().executeMethod('EndAction', ['Information', text]);
					status = null;
				}
			}, NOTICE_MS);
		}
	}

	function finish(message) {
		busy = false;
		window.uzTranslitLast = { message: message, at: Date.now() };
		// The editor writes its own status after a re-layout and would wipe a
		// message shown right away.
		setTimeout(function () { showStatus(message, true); }, 700);
	}

	function doneMessage(count, started) {
		var seconds = ((Date.now() - started) / 1000).toFixed(1);
		return 'Tayyor: ' + count + ' ta paragraf o‘girildi, ' + seconds + ' s';
	}

	function stripParaMark(text) {
		return text.replace(/\r?\n$/, '');
	}

	// Word editor: ReplaceTextSmart per paragraph, in batches, last paragraph
	// first. Measured on a 108-page document (DS 9.3.1):
	//  - one ReplaceTextSmart over the whole selection: 420 s, ~280 000 change
	//    records in one save ("saveChanges deleteIndex: 279462"), frozen tab;
	//  - all paragraphs in one command: 25-330 s, still a frozen tab;
	//  - rewriting runs (ClearContent + AddText): 615 s and an editor crash.
	// Batches of ~1 s with a pause in between keep the tab responsive, send the
	// changes in small pieces and let the status bar show progress.
	var BATCH_TARGET_MS = 1000;
	var BATCH_PAUSE_MS = 60;

	function transliterateDocument(direction, label, started) {
		plugin().callCommand(function () {
			var doc = Api.GetDocument();
			var sel = doc.GetRangeBySelect();
			var paras = sel ? sel.GetAllParagraphs() : null;
			if (!paras || !paras.length) {
				return null;
			}
			var opt = { Numbering: false, Math: false, TabSymbol: '\t', NewLineSeparator: '\r' };
			var selStart = sel.GetStartPos(), selEnd = sel.GetEndPos(), last = paras.length - 1, texts = [];
			for (var i = 0; i <= last; i++) {
				if (i !== 0 && i !== last) {
					// Paragraph.GetText: 0.4 s for 3389 paragraphs; GetRange().GetText: 75 s.
					texts.push(paras[i].GetText(opt));
					continue;
				}
				var range = paras[i].GetRange();
				var s = Math.max(selStart, range.GetStartPos()), e = Math.min(selEnd, range.GetEndPos());
				texts.push(e > s ? doc.GetRange(s, e).GetText(opt) : '');
			}
			return { texts: texts, selStart: selStart, selEnd: selEnd };
		}, false, false, function (snapshot) {
			if (!snapshot) {
				return transliterateSelection(direction, label, started);
			}
			var texts = snapshot.texts;
			var jobs = texts.map(function (text) {
				var source = stripParaMark(text);
				if (!label.source.test(source)) {
					return '';
				}
				var converted = convert(source, direction, {});
				return converted === source ? '' : toHex(converted);
			});
			var pending = [];
			for (var k = jobs.length - 1; k >= 0; k--) {
				if (jobs[k]) {
					pending.push(k);
				}
			}
			if (!pending.length) {
				return finish(label.none);
			}
			runBatches({
				label: label,
				started: started,
				jobs: jobs,
				pending: pending,
				total: pending.length,
				paraCount: texts.length,
				selStart: snapshot.selStart,
				selEnd: snapshot.selEnd,
				size: 10,
				done: 0
			});
		});
	}

	function runBatches(state) {
		var batch = state.pending.splice(0, state.size);
		if (!batch.length) {
			return finish(doneMessage(state.done, state.started));
		}
		var percent = Math.round(100 * (state.total - state.pending.length - batch.length) / state.total);
		showStatus(state.label.progress + ' ' + percent + '%', false);
		window.Asc.scope.batch = batch.map(function (k) { return [k, state.jobs[k]]; });
		window.Asc.scope.meta = [state.paraCount, state.selStart, state.selEnd, 0];
		var t0 = Date.now();
		plugin().callCommand(function () {
			var meta = Asc.scope.meta, batch = Asc.scope.batch;
			var doc = Api.GetDocument();
			var sel = doc.GetRangeBySelect();
			var paras = sel ? sel.GetAllParagraphs() : [];
			if (paras.length !== meta[0]) {
				return -1;
			}
			var last = paras.length - 1, done = 0;
			for (var b = 0; b < batch.length; b++) {
				var k = batch[b][0], hex = batch[b][1], text = '';
				for (var i = 0; i < hex.length; i += 4) {
					text += String.fromCharCode(parseInt(hex.substr(i, 4), 16));
				}
				var range = paras[k].GetRange();
				// Edge paragraphs are cut to the original selection. Paragraphs are
				// processed last to first, so these absolute positions stay valid:
				// the last paragraph goes in the first batch, the first one last.
				if (k === 0 || k === last) {
					range = doc.GetRange(Math.max(meta[1], range.GetStartPos()), Math.min(meta[2], range.GetEndPos()));
				}
				range.Select();
				if (Api.ReplaceTextSmart([text], '\t', '\r')) {
					done++;
				}
			}
			// Re-select the whole block so the next batch finds the same paragraphs.
			paras[0].GetRange().ExpandTo(paras[last].GetRange()).Select();
			return done;
		// Re-layout of a 100-page document after every batch cost ~1 s per batch;
		// lay out once, after the final batch.
		}, false, state.pending.length === 0, function (done) {
			if (done === -1 || typeof done !== 'number') {
				return finish('O‘girish to‘xtadi (' + state.done + ' ta paragraf o‘girildi): belgilash o‘zgardi. Qolganini qayta belgilab o‘giring');
			}
			state.done += done;
			var elapsed = Date.now() - t0;
			// Aim each batch at ~BATCH_TARGET_MS so the tab never stalls for long.
			state.size = Math.max(1, Math.min(200, Math.round(state.size * BATCH_TARGET_MS / Math.max(elapsed, 50))));
			setTimeout(function () { runBatches(state); }, BATCH_PAUSE_MS);
		});
	}

	// Spreadsheet and presentation editors (and selections without paragraphs):
	// one ReplaceTextSmart over the whole selection.
	function transliterateSelection(direction, label, started) {
		plugin().executeMethod('GetSelectedText', SELECTION_OPTIONS, function (text) {
			if (!text || !text.trim()) {
				return finish('Avval o‘giriladigan matnni belgilang');
			}
			if (!label.source.test(text)) {
				return finish(label.none);
			}
			var converted = convert(text, direction, {});
			if (converted === text) {
				return finish(label.none);
			}
			var paragraphs = converted.split('\n');
			// The selection text ends with a paragraph mark; without dropping it
			// ReplaceTextSmart appends an empty paragraph after the selection.
			if (paragraphs.length > 1 && paragraphs[paragraphs.length - 1] === '') {
				paragraphs.pop();
			}
			window.Asc.scope.hex = paragraphs.map(toHex);
			plugin().callCommand(function () {
				var paras = Asc.scope.hex.map(function (hex) {
					var out = '';
					for (var i = 0; i < hex.length; i += 4) {
						out += String.fromCharCode(parseInt(hex.substr(i, 4), 16));
					}
					return out;
				});
				return Api.ReplaceTextSmart(paras, '\t', '\r');
			}, false, true, function (ok) {
				if (ok === false) {
					finish('O‘girib bo‘lmadi. Kichikroq qismini belgilab qayta urinib ko‘ring');
				} else {
					finish(doneMessage(paragraphs.length, started));
				}
			});
		});
	}

	function transliterate(direction) {
		if (busy) {
			showStatus('Oldingi o‘girish hali tugamadi, kuting…', false);
			return;
		}
		busy = true;
		var label = LABELS[direction];
		var started = Date.now();
		showStatus(label.progress, false);
		if (plugin().info.editorType === 'word') {
			transliterateDocument(direction, label, started);
		} else {
			transliterateSelection(direction, label, started);
		}
	}

	function contextButton(parent, text, direction) {
		var button = new Asc.ButtonContextMenu(parent);
		button.text = text;
		button.addCheckers('Selection');
		button.attachOnClick(function () { transliterate(direction); });
		return button;
	}

	function toolbarButton(parent, text, direction) {
		var button = new Asc.ButtonToolbar(parent);
		button.text = text;
		button.icons = 'resources/%theme-type%(light|dark)/icon%scale%(default).png';
		button.attachOnClick(function () { transliterate(direction); });
		return button;
	}

	window.Asc.plugin.init = function () {
		var menu = new Asc.ButtonContextMenu();
		menu.text = 'Lotin ⇄ Kiril';
		menu.addCheckers('Selection');
		contextButton(menu, 'Lotinga o‘girish', TO_LATIN);
		contextButton(menu, 'Kirillga o‘girish', TO_CYRILLIC);

		var tab = new Asc.ButtonToolbar();
		tab.text = 'Lotin ⇄ Kiril';
		toolbarButton(tab, 'Lotinga', TO_LATIN);
		toolbarButton(tab, 'Kirillga', TO_CYRILLIC);

		Asc.Buttons.registerContextMenu();
		Asc.Buttons.registerToolbarMenu();
	};

	window.Asc.plugin.button = function () {};
})(window);
