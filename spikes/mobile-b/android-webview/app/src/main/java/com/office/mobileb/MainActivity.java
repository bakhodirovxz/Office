package com.office.mobileb;

import android.app.Activity;
import android.os.Bundle;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import androidx.webkit.WebViewAssetLoader;

/** Spike 3 B5: the B web page inside a plain Android WebView, served from the APK's assets over https://appassets.androidplatform.net. */
public class MainActivity extends Activity {
    @Override
    protected void onCreate(Bundle b) {
        super.onCreate(b);
        WebView.setWebContentsDebuggingEnabled(true);
        WebView wv = new WebView(this);
        WebSettings s = wv.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        final WebViewAssetLoader loader = new WebViewAssetLoader.Builder()
            .addPathHandler("/assets/", new WebViewAssetLoader.AssetsPathHandler(this)).build();
        wv.setWebViewClient(new WebViewClient() {
            @Override
            public WebResourceResponse shouldInterceptRequest(WebView v, WebResourceRequest r) {
                WebResourceResponse resp = loader.shouldInterceptRequest(r.getUrl());
                if (resp != null && r.getUrl().getPath().endsWith(".wasm")) resp.setMimeType("application/wasm");
                return resp;
            }
        });
        setContentView(wv);
        wv.loadUrl("https://appassets.androidplatform.net/assets/index.html?doc=long.docx");
    }
}
