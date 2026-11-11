package io.github.evire1337.evplayer

import android.content.Context
import android.content.Intent
import android.os.Environment
import android.os.Handler
import android.os.Looper
import androidx.core.content.FileProvider
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL

/**
 * A new version's APK, fetched by the app itself and handed to Android's installer when it is
 * there. It used to go through Android's DownloadManager, which some phones switch off or never
 * start (the button then did nothing, with no word why); the app's own download works the same
 * everywhere and tells the page how far it got and what went wrong.
 * The file is removed on the next start, so nothing stays on the phone.
 */
object UpdateInstaller {
  private const val NAME = "EvPlayer-update.apk"
  private const val MIME = "application/vnd.android.package-archive"
  private const val ALLOWED = "https://github.com/Evire1337/evplayer/"
  private val handler = Handler(Looper.getMainLooper())

  /** the download in progress; a newer one makes an older one stop */
  @Volatile private var generation = 0

  /** the app's own folder the installer is allowed to read from (see res/xml/file_paths.xml) */
  private fun dir(context: Context): File {
    val folder = context.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS)
    if (folder != null) {
      folder.mkdirs()
      return folder
    }
    return File(context.cacheDir, "updates").also { it.mkdirs() }
  }

  /** an update downloaded earlier is not needed any more */
  fun cleanUp(context: Context) {
    for (folder in listOfNotNull(context.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS), File(context.cacheDir, "updates"))) {
      folder.listFiles()?.filter { it.name.endsWith(".apk") || it.name.endsWith(".part") }?.forEach { it.delete() }
    }
  }

  fun install(context: Context, url: String) {
    val app = context.applicationContext
    if (!url.startsWith(ALLOWED)) {
      MainActivity.updateProgress(0, true, "адрес обновления не из репозитория EvPlayer")
      return
    }
    cleanUp(app)
    val mine = ++generation
    Thread {
      try {
        val file = fetch(url, File(dir(app), NAME), mine)
        if (mine != generation) return@Thread
        MainActivity.updateProgress(100, true, null)
        handler.post { openInstaller(app, file) }
      } catch (e: Exception) {
        if (mine == generation) {
          MainActivity.updateProgress(0, true, "не получилось скачать обновление: ${e.message ?: e.javaClass.simpleName}")
        }
      }
    }.start()
  }

  /** the file at the address, written next to its final place and renamed when whole */
  private fun fetch(url: String, target: File, mine: Int): File {
    val part = File(target.parentFile, target.name + ".part")
    val connection = URL(url).openConnection() as HttpURLConnection
    try {
      connection.connectTimeout = 15_000
      connection.readTimeout = 30_000
      // GitHub sends a release file on to its own storage server
      connection.instanceFollowRedirects = true
      connection.setRequestProperty("Accept", "application/octet-stream")
      val code = connection.responseCode
      if (code !in 200..299) throw IOException("сервер ответил $code")
      val total = connection.contentLengthLong
      var done = 0L
      var shown = -1
      connection.inputStream.use { input ->
        FileOutputStream(part).use { output ->
          val buffer = ByteArray(64 * 1024)
          while (true) {
            if (mine != generation) throw IOException("отменено")
            val n = input.read(buffer)
            if (n < 0) break
            output.write(buffer, 0, n)
            done += n
            if (total > 0) {
              val percent = (done * 100 / total).toInt().coerceIn(0, 99)
              if (percent != shown) {
                shown = percent
                MainActivity.updateProgress(percent, false, null)
              }
            }
          }
        }
      }
      if (total > 0 && done != total) throw IOException("файл пришёл не целиком")
      if (done == 0L) throw IOException("пустой файл")
      target.delete()
      if (!part.renameTo(target)) throw IOException("не удалось сохранить файл")
      return target
    } finally {
      connection.disconnect()
      if (part.exists() && !target.exists()) part.delete()
    }
  }

  private fun openInstaller(context: Context, file: File) {
    try {
      val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
      val intent = Intent(Intent.ACTION_VIEW)
        .setDataAndType(uri, MIME)
        .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
      context.startActivity(intent)
    } catch (e: Exception) {
      MainActivity.updateProgress(0, true, "не удалось открыть установщик: ${e.message ?: e.javaClass.simpleName}")
    }
  }
}
