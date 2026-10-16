package com.office.probe;

import android.app.Activity;
import android.os.Bundle;
import android.text.Editable;
import android.text.InputType;
import android.text.TextWatcher;
import android.util.Log;
import android.view.Gravity;
import android.view.ViewGroup;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

// Control for the kill test: a stock multiline EditText with the same input type the shim
// asks GameActivity for, so a failure in the probe can be attributed to eframe/winit and not
// to the keyboard set-up. Logs every text change to logcat (tag "control") and on screen.
public class ControlActivity extends Activity {
    private TextView log;
    private final long t0 = System.currentTimeMillis();

    @Override
    protected void onCreate(Bundle b) {
        super.onCreate(b);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(24, 90, 24, 24);
        TextView title = new TextView(this);
        title.setText("CONTROL: stock EditText (multiline, autocorrect, cap sentences)");
        root.addView(title);
        final EditText et = new EditText(this);
        et.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE
                | InputType.TYPE_TEXT_FLAG_AUTO_CORRECT | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES);
        et.setMinLines(4);
        et.setGravity(Gravity.TOP);
        root.addView(et, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        log = new TextView(this);
        log.setTextSize(10);
        log.setTypeface(android.graphics.Typeface.MONOSPACE);
        ScrollView sv = new ScrollView(this);
        sv.addView(log);
        root.addView(sv, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));
        setContentView(root);
        et.addTextChangedListener(new TextWatcher() {
            public void beforeTextChanged(CharSequence s, int st, int c, int a) {}
            public void onTextChanged(CharSequence s, int st, int before, int count) {
                String line = String.format("%6.2f change start=%d removed=%d added=%d text=%s", (System.currentTimeMillis() - t0) / 1000.0, st, before, count, quote(s));
                Log.i("control", line);
                log.append(line + "\n");
            }
            public void afterTextChanged(Editable e) {}
        });
        et.requestFocus();
    }

    private static String quote(CharSequence s) {
        return "\"" + s.toString().replace("\n", "\n") + "\"";
    }
}
