package io.github.evire1337.evplayer

import android.os.Bundle
import android.webkit.WebSettings
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  // A phone keeps no cache: the page's network answers are never written to storage, and
  // whatever an earlier start left behind is cleared. The library, playlists and settings
  // (kept by the page itself) are not a cache and stay.
  override fun onWebViewCreate(webView: WebView) {
    webView.settings.cacheMode = WebSettings.LOAD_NO_CACHE
    webView.clearCache(true)
    // the page starts music from media keys and the notification without a tap first
    webView.settings.mediaPlaybackRequiresUserGesture = false
  }

  override fun onStop() {
    super.onStop()
    cacheDir.listFiles()?.forEach { it.deleteRecursively() }
  }
}
