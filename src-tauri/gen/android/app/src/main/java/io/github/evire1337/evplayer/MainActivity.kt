package io.github.evire1337.evplayer

import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.webkit.WebSettings
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import org.json.JSONObject

class MainActivity : TauriActivity() {
  companion object {
    @Volatile private var webView: WebView? = null

    /** the page (and its music) is there to take commands */
    val isRunning: Boolean get() = webView != null

    /** a button in the notification or on the lock screen: hand it to the page */
    fun command(action: String, value: Double? = null) {
      val view = webView ?: return
      val arg = if (value == null) "undefined" else value.toString()
      Handler(Looper.getMainLooper()).post {
        view.evaluateJavascript("window.__evplayerMediaCommand && window.__evplayerMediaCommand(${JSONObject.quote(action)}, $arg)", null)
      }
    }

    private fun visible(value: Boolean) {
      val view = webView ?: return
      view.evaluateJavascript("window.__evplayerVisible && window.__evplayerVisible($value)", null)
    }
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  override fun onWebViewCreate(webView: WebView) {
    MainActivity.webView = webView
    // A phone keeps no cache: the page's network answers are never written to storage, and
    // whatever an earlier start left behind is cleared. The library, playlists and settings
    // (kept by the page itself) are not a cache and stay.
    webView.settings.cacheMode = WebSettings.LOAD_NO_CACHE
    webView.clearCache(true)
    // buttons in the notification start the music without a tap in the page first
    webView.settings.mediaPlaybackRequiresUserGesture = false
    webView.addJavascriptInterface(MediaBridge(applicationContext), "EvPlayerAndroid")
    // the home-screen widget's buttons can now reach the page
    PlayerWidget.render(applicationContext)
  }

  override fun onPause() {
    super.onPause()
    // the base class pauses the page when the app leaves the screen; while music plays it
    // has to keep going (the page itself calms its animations down when told it is hidden)
    if (MediaService.isPlaying) webView?.onResume()
  }

  override fun onStart() {
    super.onStart()
    visible(true)
  }

  override fun onStop() {
    super.onStop()
    visible(false)
    cacheDir.listFiles()?.forEach { it.deleteRecursively() }
  }

  override fun onDestroy() {
    if (isFinishing) {
      webView = null
      MediaService.stop(applicationContext)
    }
    super.onDestroy()
  }
}
