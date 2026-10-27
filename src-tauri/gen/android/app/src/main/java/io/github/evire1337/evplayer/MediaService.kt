package io.github.evire1337.evplayer

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.support.v4.media.MediaMetadataCompat
import android.support.v4.media.session.MediaSessionCompat
import android.support.v4.media.session.PlaybackStateCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.media.app.NotificationCompat.MediaStyle
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
import kotlin.concurrent.thread

/**
 * The playing song in the notification shade and on the lock screen, with previous / play
 * / next / like buttons and a seek bar. The music itself plays in the app's page; this
 * service only shows it and passes button presses back. While music plays it runs in the
 * foreground, which also keeps Android from closing the app in the background.
 */
class MediaService : Service() {
  companion object {
    private const val CHANNEL = "playback"
    private const val NOTIFICATION_ID = 1
    private const val ACTION_PREV = "evplayer.prev"
    private const val ACTION_TOGGLE = "evplayer.toggle"
    private const val ACTION_NEXT = "evplayer.next"
    private const val ACTION_LIKE = "evplayer.like"

    /** the latest state sent by the page */
    @Volatile var state: State? = null
    @Volatile private var instance: MediaService? = null

    val isPlaying: Boolean get() = state?.playing == true

    /** called by the page (through MediaBridge) whenever the song or its state changes */
    fun update(context: Context, json: String) {
      val next = State.parse(json) ?: return
      state = next
      val service = instance
      if (service != null) {
        Handler(Looper.getMainLooper()).post { service.render() }
      } else if (next.title.isNotEmpty() && next.playing) {
        val intent = Intent(context, MediaService::class.java)
        if (Build.VERSION.SDK_INT >= 26) context.startForegroundService(intent) else context.startService(intent)
      }
    }

    fun stop(context: Context) {
      state = null
      context.stopService(Intent(context, MediaService::class.java))
    }
  }

  data class State(
    val title: String,
    val artist: String,
    val artwork: String?,
    val playing: Boolean,
    val positionMs: Long,
    val durationMs: Long,
    val liked: Boolean,
  ) {
    companion object {
      fun parse(json: String): State? = try {
        val o = JSONObject(json)
        State(
          title = o.optString("title"),
          artist = o.optString("artist"),
          artwork = o.optString("artwork").takeIf { it.startsWith("https://") },
          playing = o.optBoolean("playing"),
          positionMs = o.optLong("positionMs"),
          durationMs = o.optLong("durationMs"),
          liked = o.optBoolean("liked"),
        )
      } catch (e: Exception) {
        null
      }
    }
  }

  private lateinit var session: MediaSessionCompat
  private var artUrl: String? = null
  private var art: Bitmap? = null
  private var foreground = false

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onCreate() {
    super.onCreate()
    instance = this
    if (Build.VERSION.SDK_INT >= 26) {
      val channel = NotificationChannel(CHANNEL, "Воспроизведение", NotificationManager.IMPORTANCE_LOW)
      channel.setShowBadge(false)
      getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }
    session = MediaSessionCompat(this, "EvPlayer").apply {
      setCallback(object : MediaSessionCompat.Callback() {
        override fun onPlay() = MainActivity.command("togglePlay")
        override fun onPause() = MainActivity.command("togglePlay")
        override fun onSkipToNext() = MainActivity.command("next")
        override fun onSkipToPrevious() = MainActivity.command("prevTrack")
        override fun onSeekTo(pos: Long) = MainActivity.command("seekTo", pos.toDouble())
        override fun onCustomAction(action: String?, extras: android.os.Bundle?) {
          if (action == ACTION_LIKE) MainActivity.command("like")
        }
      })
      setSessionActivity(openAppIntent())
      isActive = true
    }
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    when (intent?.action) {
      ACTION_PREV -> MainActivity.command("prevTrack")
      ACTION_TOGGLE -> MainActivity.command("togglePlay")
      ACTION_NEXT -> MainActivity.command("next")
      ACTION_LIKE -> MainActivity.command("like")
    }
    render()
    return START_NOT_STICKY
  }

  override fun onTaskRemoved(rootIntent: Intent?) {
    // the app was swiped away: the page and its music are gone, so is the notification
    stopSelf()
  }

  override fun onDestroy() {
    instance = null
    session.release()
    super.onDestroy()
  }

  private fun openAppIntent(): PendingIntent {
    val intent = Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
    return PendingIntent.getActivity(this, 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
  }

  private fun actionIntent(action: String): PendingIntent {
    val intent = Intent(this, MediaService::class.java).setAction(action)
    return PendingIntent.getService(this, action.hashCode(), intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
  }

  fun render() {
    val s = state
    if (s == null || s.title.isEmpty()) {
      stopSelf()
      return
    }
    loadArtwork(s.artwork)
    session.setMetadata(
      MediaMetadataCompat.Builder()
        .putString(MediaMetadataCompat.METADATA_KEY_TITLE, s.title)
        .putString(MediaMetadataCompat.METADATA_KEY_ARTIST, s.artist)
        .putLong(MediaMetadataCompat.METADATA_KEY_DURATION, s.durationMs)
        .putBitmap(MediaMetadataCompat.METADATA_KEY_ALBUM_ART, art)
        .build(),
    )
    val likeAction = PlaybackStateCompat.CustomAction.Builder(
      ACTION_LIKE,
      if (s.liked) "Убрать из понравившихся" else "Нравится",
      if (s.liked) R.drawable.ic_heart_filled else R.drawable.ic_heart,
    ).build()
    session.setPlaybackState(
      PlaybackStateCompat.Builder()
        .setState(if (s.playing) PlaybackStateCompat.STATE_PLAYING else PlaybackStateCompat.STATE_PAUSED, s.positionMs, if (s.playing) 1f else 0f)
        .setActions(
          PlaybackStateCompat.ACTION_PLAY or PlaybackStateCompat.ACTION_PAUSE or PlaybackStateCompat.ACTION_PLAY_PAUSE or
            PlaybackStateCompat.ACTION_SKIP_TO_NEXT or PlaybackStateCompat.ACTION_SKIP_TO_PREVIOUS or PlaybackStateCompat.ACTION_SEEK_TO,
        )
        .addCustomAction(likeAction)
        .build(),
    )

    val notification: Notification = NotificationCompat.Builder(this, CHANNEL)
      .setSmallIcon(R.drawable.ic_notification)
      .setContentTitle(s.title)
      .setContentText(s.artist)
      .setLargeIcon(art)
      .setContentIntent(openAppIntent())
      .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
      .setOnlyAlertOnce(true)
      .setShowWhen(false)
      .setOngoing(s.playing)
      .addAction(android.R.drawable.ic_media_previous, "Предыдущий", actionIntent(ACTION_PREV))
      .addAction(
        if (s.playing) android.R.drawable.ic_media_pause else android.R.drawable.ic_media_play,
        if (s.playing) "Пауза" else "Играть",
        actionIntent(ACTION_TOGGLE),
      )
      .addAction(android.R.drawable.ic_media_next, "Следующий", actionIntent(ACTION_NEXT))
      .addAction(if (s.liked) R.drawable.ic_heart_filled else R.drawable.ic_heart, "Нравится", actionIntent(ACTION_LIKE))
      .setStyle(MediaStyle().setMediaSession(session.sessionToken).setShowActionsInCompactView(0, 1, 2))
      .build()

    if (s.playing) {
      val type = if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK else 0
      ServiceCompat.startForeground(this, NOTIFICATION_ID, notification, type)
      foreground = true
    } else {
      // paused: the notification stays (to start again from it) but may be swiped away
      if (foreground) {
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_DETACH)
        foreground = false
      }
      getSystemService(NotificationManager::class.java).notify(NOTIFICATION_ID, notification)
    }
  }

  /** the cover, fetched once per song in the background; the notification is redrawn when it arrives */
  private fun loadArtwork(url: String?) {
    if (url == artUrl) return
    artUrl = url
    art = null
    if (url == null) return
    thread {
      val bitmap = try {
        val conn = URL(url).openConnection() as HttpURLConnection
        conn.connectTimeout = 8000
        conn.readTimeout = 8000
        conn.inputStream.use { BitmapFactory.decodeStream(it) }
      } catch (e: Exception) {
        null
      }
      Handler(Looper.getMainLooper()).post {
        if (artUrl == url && bitmap != null) {
          art = bitmap
          render()
        }
      }
    }
  }
}
