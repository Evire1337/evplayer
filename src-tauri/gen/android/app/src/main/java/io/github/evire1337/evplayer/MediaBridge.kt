package io.github.evire1337.evplayer

import android.content.Context
import android.webkit.JavascriptInterface

/** what the page can call as window.EvPlayerAndroid */
class MediaBridge(private val context: Context) {
  /** the playing song and its state, as JSON (see src/platform/androidMedia.ts) */
  @JavascriptInterface
  fun update(json: String) = MediaService.update(context, json)

  /** nothing plays any more: remove the notification */
  @JavascriptInterface
  fun clear() = MediaService.stop(context)

  /** is the app free to run in the background (battery optimisation off for it) */
  @JavascriptInterface
  fun keepsRunning(): Boolean = MainActivity.keepsRunning(context)

  /** show Android's dialog that lets the app run in the background */
  @JavascriptInterface
  fun askToKeepRunning() = MainActivity.askToKeepRunning(context)
}
