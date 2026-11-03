package io.github.evire1337.evplayer

import android.app.DownloadManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.Uri
import android.os.Environment
import android.os.Handler
import android.os.Looper
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import java.io.File

/**
 * A new version's APK, fetched inside the app (with Android's own download notification) and
 * handed to Android's installer when it is there. It used to open the link in the browser.
 * The file is removed on the next start, so nothing stays on the phone.
 */
object UpdateInstaller {
  private const val MIME = "application/vnd.android.package-archive"
  private const val NAME = "EvPlayer-update.apk"
  private var downloadId = -1L
  private val handler = Handler(Looper.getMainLooper())

  private fun dir(context: Context): File? = context.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS)

  /** an update downloaded earlier is not needed any more */
  fun cleanUp(context: Context) {
    dir(context)?.listFiles()?.filter { it.name.endsWith(".apk") }?.forEach { it.delete() }
  }

  fun install(context: Context, url: String) {
    val app = context.applicationContext
    if (!url.startsWith("https://github.com/Evire1337/evplayer/")) return
    val manager = app.getSystemService(DownloadManager::class.java)
    if (downloadId >= 0) manager.remove(downloadId)
    cleanUp(app)
    val file = File(dir(app), NAME)
    val request = DownloadManager.Request(Uri.parse(url))
      .setTitle("Обновление EvPlayer")
      .setMimeType(MIME)
      .setDestinationUri(Uri.fromFile(file))
      .setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE)
    downloadId = manager.enqueue(request)
    val id = downloadId

    val receiver = object : BroadcastReceiver() {
      override fun onReceive(c: Context, intent: Intent) {
        if (intent.getLongExtra(DownloadManager.EXTRA_DOWNLOAD_ID, -1) != id) return
        app.unregisterReceiver(this)
        handler.removeCallbacksAndMessages(null)
        if (status(manager, id) == DownloadManager.STATUS_SUCCESSFUL && file.exists()) {
          MainActivity.updateProgress(100, true, null)
          openInstaller(app, file)
        } else {
          MainActivity.updateProgress(0, true, "не получилось скачать обновление")
        }
      }
    }
    ContextCompat.registerReceiver(app, receiver, IntentFilter(DownloadManager.ACTION_DOWNLOAD_COMPLETE), ContextCompat.RECEIVER_EXPORTED)
    poll(manager, id)
  }

  private fun status(manager: DownloadManager, id: Long): Int {
    manager.query(DownloadManager.Query().setFilterById(id)).use { c ->
      return if (c.moveToFirst()) c.getInt(c.getColumnIndexOrThrow(DownloadManager.COLUMN_STATUS)) else DownloadManager.STATUS_FAILED
    }
  }

  /** how far it got, for the page, twice a second */
  private fun poll(manager: DownloadManager, id: Long) {
    handler.postDelayed({
      if (id != downloadId) return@postDelayed
      manager.query(DownloadManager.Query().setFilterById(id)).use { c ->
        if (!c.moveToFirst()) return@postDelayed
        val done = c.getLong(c.getColumnIndexOrThrow(DownloadManager.COLUMN_BYTES_DOWNLOADED_SO_FAR))
        val total = c.getLong(c.getColumnIndexOrThrow(DownloadManager.COLUMN_TOTAL_SIZE_BYTES))
        val state = c.getInt(c.getColumnIndexOrThrow(DownloadManager.COLUMN_STATUS))
        if (total > 0) MainActivity.updateProgress((done * 100 / total).toInt().coerceIn(0, 99), false, null)
        if (state == DownloadManager.STATUS_FAILED) {
          MainActivity.updateProgress(0, true, "не получилось скачать обновление")
          return@postDelayed
        }
      }
      poll(manager, id)
    }, 500)
  }

  private fun openInstaller(context: Context, file: File) {
    val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
    val intent = Intent(Intent.ACTION_VIEW)
      .setDataAndType(uri, MIME)
      .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
    context.startActivity(intent)
  }
}
