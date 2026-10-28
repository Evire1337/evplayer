package io.github.evire1337.evplayer

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapShader
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.Shader
import android.widget.RemoteViews

/**
 * The player on the home screen: cover, song, artist and previous / play / next / like
 * buttons, drawn from the same state as the notification. The music plays in the app's
 * page, so while the app is not running the buttons open it instead.
 */
class PlayerWidget : AppWidgetProvider() {
  companion object {
    private const val ACTION = "evplayer.widget"
    private const val EXTRA = "command"
    /** the cover as last drawn (kept small: a widget's picture travels between processes) */
    @Volatile private var cover: Bitmap? = null
    @Volatile private var coverOf: Bitmap? = null

    fun render(context: Context, art: Bitmap? = coverOf) {
      val manager = AppWidgetManager.getInstance(context)
      val ids = manager.getAppWidgetIds(ComponentName(context, PlayerWidget::class.java))
      if (ids.isEmpty()) return
      val s = MediaService.state
      val alive = MainActivity.isRunning
      val views = RemoteViews(context.packageName, R.layout.widget_player)
      if (s != null && s.title.isNotEmpty()) {
        views.setTextViewText(R.id.widget_title, s.title)
        views.setTextViewText(R.id.widget_artist, s.artist)
        views.setImageViewResource(R.id.widget_toggle, if (s.playing) R.drawable.ic_w_pause else R.drawable.ic_w_play)
        views.setContentDescription(R.id.widget_toggle, if (s.playing) "Пауза" else "Играть")
        views.setImageViewResource(R.id.widget_like, if (s.liked) R.drawable.ic_heart_filled else R.drawable.ic_heart)
      } else {
        views.setTextViewText(R.id.widget_title, "EvPlayer")
        views.setTextViewText(R.id.widget_artist, context.getString(R.string.widget_idle))
        views.setImageViewResource(R.id.widget_toggle, R.drawable.ic_w_play)
        views.setImageViewResource(R.id.widget_like, R.drawable.ic_heart)
      }
      if (art != null && s != null) {
        if (coverOf !== art) {
          cover = rounded(art)
          coverOf = art
        }
        views.setImageViewBitmap(R.id.widget_cover, cover)
      } else {
        views.setImageViewResource(R.id.widget_cover, R.drawable.widget_cover_empty)
      }
      val open = openApp(context)
      views.setOnClickPendingIntent(R.id.widget_root, open)
      views.setOnClickPendingIntent(R.id.widget_cover, open)
      val buttons = mapOf(
        R.id.widget_prev to "prevTrack",
        R.id.widget_toggle to "togglePlay",
        R.id.widget_next to "next",
        R.id.widget_like to "like",
      )
      for ((id, command) in buttons) {
        views.setOnClickPendingIntent(id, if (alive && s != null) button(context, command) else open)
      }
      manager.updateAppWidget(ids, views)
    }

    /** nothing plays any more (the app was closed): the widget opens the app */
    fun clear(context: Context) {
      coverOf = null
      cover = null
      render(context, null)
    }

    private fun openApp(context: Context): PendingIntent {
      val intent = Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
      return PendingIntent.getActivity(context, 10, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }

    private fun button(context: Context, command: String): PendingIntent {
      val intent = Intent(context, Buttons::class.java).setAction(ACTION).putExtra(EXTRA, command)
      return PendingIntent.getBroadcast(context, command.hashCode(), intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }

    /** a small square cover with rounded corners */
    private fun rounded(src: Bitmap): Bitmap {
      val size = 240
      val side = minOf(src.width, src.height)
      val square = Bitmap.createBitmap(src, (src.width - side) / 2, (src.height - side) / 2, side, side)
      val scaled = Bitmap.createScaledBitmap(square, size, size, true)
      val out = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
      val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { shader = BitmapShader(scaled, Shader.TileMode.CLAMP, Shader.TileMode.CLAMP) }
      Canvas(out).drawRoundRect(RectF(0f, 0f, size.toFloat(), size.toFloat()), 40f, 40f, paint)
      return out
    }
  }

  override fun onUpdate(context: Context, appWidgetManager: AppWidgetManager, appWidgetIds: IntArray) {
    render(context)
  }

  /** a widget button: handed to the page like a notification button */
  class Buttons : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
      val command = intent.getStringExtra(EXTRA) ?: return
      if (MainActivity.isRunning) {
        MainActivity.command(command)
      } else {
        // the app was closed in the meantime: from now on the widget opens it
        render(context, null)
      }
    }
  }
}
