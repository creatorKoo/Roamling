// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
package io.github.creatorkoo.roamling

import android.app.AppOpsManager
import android.app.KeyguardManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.content.res.Configuration
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.PorterDuff
import android.graphics.drawable.Icon
import android.os.Binder
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.provider.Settings
import android.util.Log
import java.util.concurrent.Executors
import kotlin.math.roundToInt
import uniffi.roamling_core.FfiPoint

/** Owns one explicitly started companion session, independently of Activity lifetime. */
class CompanionService : Service() {
    inner class LocalBinder : Binder() { val service: CompanionService get() = this@CompanionService }
    private val binder = LocalBinder()
    private val main = Handler(Looper.getMainLooper())
    private val loader = Executors.newSingleThreadExecutor()
    private var images: MochiImages? = null
    internal var overlay: MochiOverlay? = null
        private set
    var running = false
        private set
    var hidden = false
        private set
    var loading = false
        private set
    var error: Int? = null
        private set
    private var destroyed = false
    private var loadGeneration = 0
    private var screenOff = false
    private val listeners = mutableSetOf<() -> Unit>()
    private val notifications get() = getSystemService(NotificationManager::class.java)
    // Same paw glyph as the desktop mark, with a transparent monochrome mask.
    private val notificationIcon by lazy {
        val size = (24 * resources.displayMetrics.density).roundToInt()
        val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.WHITE; textAlign = Paint.Align.CENTER; textSize = size * 0.9f
        }
        val metrics = paint.fontMetrics
        val canvas = Canvas(bitmap)
        canvas.drawText("🐾", size / 2f, size / 2f - (metrics.ascent + metrics.descent) / 2, paint)
        canvas.drawColor(Color.WHITE, PorterDuff.Mode.SRC_IN)
        Icon.createWithBitmap(bitmap)
    }
    private val preferences get() = getSharedPreferences("companion", MODE_PRIVATE)
    val visible: Boolean get() = overlay?.isShowing == true
    val locked: Boolean get() = screenOff || !getSystemService(PowerManager::class.java).isInteractive ||
        getSystemService(KeyguardManager::class.java).isKeyguardLocked

    private val screenReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            screenOff = intent.action == Intent.ACTION_SCREEN_OFF
            if (running) reconcileVisibility()
        }
    }
    private val permissionWatcher = AppOpsManager.OnOpChangedListener { _, packageName ->
        if (packageName == this.packageName) main.post {
            if (running && !Settings.canDrawOverlays(this)) fail(R.string.permission_needed)
        }
    }

    override fun onCreate() {
        super.onCreate()
        notifications.createNotificationChannel(NotificationChannel(CHANNEL, getString(R.string.notification_channel), NotificationManager.IMPORTANCE_LOW))
        val filter = IntentFilter().apply {
            addAction(Intent.ACTION_SCREEN_OFF); addAction(Intent.ACTION_SCREEN_ON); addAction(Intent.ACTION_USER_PRESENT)
        }
        // These are protected system broadcasts. SystemUI's USER_PRESENT can
        // originate from its own privileged UID rather than system UID 1000.
        if (Build.VERSION.SDK_INT >= 33) registerReceiver(screenReceiver, filter, RECEIVER_EXPORTED)
        else registerReceiver(screenReceiver, filter)
        getSystemService(AppOpsManager::class.java).startWatchingMode(AppOpsManager.OPSTR_SYSTEM_ALERT_WINDOW, packageName, permissionWatcher)
    }

    override fun onBind(intent: Intent): IBinder = binder
    fun subscribe(listener: () -> Unit) { listeners.add(listener); listener() }
    fun unsubscribe(listener: () -> Unit) { listeners.remove(listener) }
    private fun publish() { listeners.toList().forEach { it() } }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_SHOW -> startCompanion()
            ACTION_HIDE -> hideCompanion()
            ACTION_STOP -> stopCompanion()
            else -> if (!running) stopSelf()
        }
        return START_NOT_STICKY
    }

    private fun startCompanion() {
        if (!Settings.canDrawOverlays(this)) { fail(R.string.permission_needed); return }
        if (!notificationsAllowed(this)) { fail(R.string.notification_needed); return }
        error = null
        hidden = false
        if (!running) {
            running = true
            try {
                if (Build.VERSION.SDK_INT >= 34) startForeground(NOTIFICATION_ID, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
                else startForeground(NOTIFICATION_ID, notification())
            } catch (failure: RuntimeException) {
                Log.e("Roamling", "Foreground session could not start", failure)
                fail(R.string.show_failed); return
            }
        }
        if (images != null) { reconcileVisibility(); return }
        if (loading) { publish(); return }
        loading = true
        val generation = ++loadGeneration
        publish()
        loader.execute {
            val result = runCatching { MochiImages.load() }
            main.post {
                if (destroyed || !running || generation != loadGeneration) result.getOrNull()?.close()
                else {
                    loading = false
                    images = result.getOrNull()
                    if (images == null) {
                        Log.e("Roamling", "Bori load failed", result.exceptionOrNull())
                        fail(R.string.load_failed)
                    } else reconcileVisibility()
                }
            }
        }
    }

    fun hideCompanion() {
        if (!running) return
        hidden = true
        reconcileVisibility()
    }

    fun validatePermissions() {
        if (running && !Settings.canDrawOverlays(this)) fail(R.string.permission_needed)
        else if (running && !notificationsAllowed(this)) fail(R.string.notification_needed)
    }

    private fun reconcileVisibility() {
        if (!running) return
        validatePermissions()
        if (!running) return
        if (hidden || locked) overlay?.pause()
        else if (images != null) {
            try {
                if (overlay == null) overlay = MochiOverlay(this, checkNotNull(images), ::savePosition) {
                    Log.e("Roamling", "Overlay update failed", it)
                    fail(R.string.show_failed)
                }
                if (!visible) checkNotNull(overlay).show(carried = savedPosition()) { publish() }
            } catch (failure: RuntimeException) {
                Log.e("Roamling", "Overlay attach failed", failure)
                fail(R.string.show_failed); return
            }
        }
        notifications.notify(NOTIFICATION_ID, notification())
        publish()
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        overlay?.close() // saves its center before rebuilding against new metrics/density
        overlay = null
        if (running) reconcileVisibility()
    }

    internal fun savedPosition(): FfiPoint? {
        if (!preferences.contains("x") || !preferences.contains("y")) return null
        val x = Double.fromBits(preferences.getLong("x", 0))
        val y = Double.fromBits(preferences.getLong("y", 0))
        return if (x.isFinite() && y.isFinite()) FfiPoint(x, y) else null
    }

    private fun savePosition(point: FfiPoint) {
        preferences.edit().putLong("x", point.x.toBits()).putLong("y", point.y.toBits()).apply()
    }

    private fun fail(message: Int) { stopCompanion(); error = message; publish() }

    fun stopCompanion() {
        running = false
        hidden = false
        loading = false
        loadGeneration++
        overlay?.close()
        overlay = null
        images?.close()
        images = null
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
        publish()
    }

    override fun onDestroy() {
        destroyed = true
        stopCompanion()
        unregisterReceiver(screenReceiver)
        getSystemService(AppOpsManager::class.java).stopWatchingMode(permissionWatcher)
        listeners.clear()
        loader.shutdown()
        super.onDestroy()
    }

    private fun notification(): Notification {
        fun action(action: String) = PendingIntent.getService(this, action.hashCode(),
            Intent(this, CompanionService::class.java).setAction(action), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(notificationIcon).setContentTitle(getString(R.string.app_name))
            .setContentText(getString(when { hidden -> R.string.hidden; locked -> R.string.locked; else -> R.string.notification_running }))
            .setContentIntent(open).setOngoing(true).setOnlyAlertOnce(true).setShowWhen(false)
            .setCategory(Notification.CATEGORY_SERVICE)
            .addAction(Notification.Action.Builder(null, getString(if (hidden) R.string.show_mochi else R.string.hide_mochi),
                action(if (hidden) ACTION_SHOW else ACTION_HIDE)).build())
            .addAction(Notification.Action.Builder(null, getString(R.string.stop_mochi), action(ACTION_STOP)).build())
            .build()
    }

    companion object {
        const val CHANNEL = "companion"
        const val NOTIFICATION_ID = 1
        const val ACTION_SHOW = "io.github.creatorkoo.roamling.SHOW"
        const val ACTION_HIDE = "io.github.creatorkoo.roamling.HIDE"
        const val ACTION_STOP = "io.github.creatorkoo.roamling.STOP"
        fun notificationsAllowed(context: Context): Boolean {
            val manager = context.getSystemService(NotificationManager::class.java)
            return manager.areNotificationsEnabled() && manager.getNotificationChannel(CHANNEL)?.importance != NotificationManager.IMPORTANCE_NONE
        }
    }
}
