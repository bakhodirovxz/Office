package com.office.probe;

import android.os.Bundle;

import com.google.androidgamesdk.GameActivity;

// Test scaffold only. Structure adapted from rust-mobile/rust-android-examples `agdk-eframe`
// (no LICENSE file was found in that repository; see docs/phase0/mobile-a-kill.md).
public class MainActivity extends GameActivity {
    static {
        System.loadLibrary("main");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
    }
}
