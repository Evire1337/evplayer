package io.github.evire1337.evplayer

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import androidx.activity.OnBackPressedCallback
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.webkit.WebSettings
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import org.json.JSONObject

class MainActivity : TauriActivity() {
  companion object {
    @Volatile private var webView: WebView? = null

    @Volatile private var instance: MainActivity? = null

    /** Android does not limit the app in the background (battery optimisation is off for it) */
    fun keepsRunning(context: Context): Boolean {
      if (Build.VERSION.SDK_INT < 23) return true
      return context.getSystemService(PowerManager::class.java).isIgnoringBatteryOptimizations(context.packageName)
    }

    /** ask Android (with its own dialog) to let the app run in the background */
    fun askToKeepRunning(context: Context) {
      if (Build.VERSION.SDK_INT < 23 || keepsRunning(context)) return
      val activity = instance ?: return
      Handler(Looper.getMainLooper()).post {
        try {
          activity.startActivity(Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:${context.packageName}")))
        } catch (e: Exception) {
          // some phones hide that dialog: the list of apps instead
          activity.startActivity(Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
        }
      }
    }

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
    instance = this
    // "back": first the app's own previous tab; on the first tab the app goes to the
    // background like other music apps, and the music keeps playing (closing it would stop it)
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        val view = webView
        if (view == null) {
          moveTaskToBack(true)
          return
        }
        view.evaluateJavascript("window.__evplayerBack ? window.__evplayerBack() : false") { went ->
          if (went != "true") moveTaskToBack(true)
        }
      }
    })
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
    // the page runs in a separate process, which Android lets go first when the app is off
    // the screen (after a while in another app the player was simply gone); it stays
    // important while the app is in the background too
    if (Build.VERSION.SDK_INT >= 26) webView.setRendererPriorityPolicy(WebView.RENDERER_PRIORITY_IMPORTANT, false)
    // on by default, as far as Android allows: an app cannot switch its battery
    // optimisation off itself, so on the first start Android's own dialog asks once (later
    // it is in the settings, under "Работа в фоне")
    val prefs = getSharedPreferences("evplayer", MODE_PRIVATE)
    if (!prefs.getBoolean("askedKeepRunning", false)) {
      prefs.edit().putBoolean("askedKeepRunning", true).apply()
      Handler(Looper.getMainLooper()).postDelayed({ askToKeepRunning(applicationContext) }, 1500)
    }
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
      instance = null
      MediaService.stop(applicationContext)
    }
    super.onDestroy()
  }
}
