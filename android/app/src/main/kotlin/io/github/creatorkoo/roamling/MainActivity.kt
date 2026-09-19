// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
package io.github.creatorkoo.roamling

import android.Manifest
import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.ComponentName
import android.content.Intent
import android.content.ServiceConnection
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.IBinder
import android.provider.Settings
import android.util.Log
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowInsets
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.SeekBar
import android.widget.TextView
import kotlin.math.roundToInt

class MainActivity : Activity() {
    private var service: CompanionService? = null
    private var bound = false
    private var resumed = false
    private var awaitingPermission = false
    private var startWhenConnected = false
    private lateinit var showButton: Button
    private lateinit var hideButton: Button
    private lateinit var stopButton: Button
    private lateinit var status: TextView
    private lateinit var sizeBar: SeekBar
    private lateinit var sizeLabel: TextView
    private var draggingSize = false
    private lateinit var stage: View
    internal val previewIsAttached: Boolean get() = service?.visible == true
    internal val previewOverlay: MochiOverlay? get() = service?.overlay
    internal val companionService: CompanionService? get() = service
    private val changed: () -> Unit = { refresh() }
    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName, binder: IBinder) {
            service = (binder as CompanionService.LocalBinder).service
            service?.subscribe(changed)
            service?.validatePermissions()
            if (startWhenConnected && resumed) {
                startWhenConnected = false
                requestShow()
            }
        }
        override fun onServiceDisconnected(name: ComponentName) { service = null; refresh() }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        awaitingPermission = savedInstanceState?.getBoolean("awaiting_permission") ?: false
        buildScreen()
    }

    override fun onStart() {
        super.onStart()
        bound = bindService(Intent(this, CompanionService::class.java), connection, BIND_AUTO_CREATE)
    }

    override fun onResume() {
        super.onResume()
        resumed = true
        service?.validatePermissions()
        if (awaitingPermission) {
            awaitingPermission = false
            if (Settings.canDrawOverlays(this)) {
                if (service == null) startWhenConnected = true else requestShow()
            }
        }
        refresh()
    }

    override fun onSaveInstanceState(outState: Bundle) {
        outState.putBoolean("awaiting_permission", awaitingPermission)
        super.onSaveInstanceState(outState)
    }

    override fun dispatchTouchEvent(event: MotionEvent): Boolean {
        if (event.actionMasked == MotionEvent.ACTION_DOWN || event.actionMasked == MotionEvent.ACTION_MOVE ||
            event.actionMasked == MotionEvent.ACTION_UP) service?.overlay?.noteInput()
        return super.dispatchTouchEvent(event)
    }

    override fun onStop() {
        resumed = false
        service?.unsubscribe(changed)
        service = null
        if (bound) { unbindService(connection); bound = false }
        // The explicitly started foreground service retains the companion.
        super.onStop()
    }

    private fun requestShow() {
        if (!resumed) return
        if (!Settings.canDrawOverlays(this)) {
            awaitingPermission = true
            try { startActivity(Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:$packageName"))) }
            catch (_: ActivityNotFoundException) { awaitingPermission = false; status.setText(R.string.permission_unavailable) }
            return
        }
        if (!CompanionService.notificationsAllowed(this)) {
            status.setText(R.string.notification_needed)
            if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED &&
                (!getPreferences(MODE_PRIVATE).getBoolean("notification_asked", false) || shouldShowRequestPermissionRationale(Manifest.permission.POST_NOTIFICATIONS))) {
                getPreferences(MODE_PRIVATE).edit().putBoolean("notification_asked", true).apply()
                requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
            } else {
                // Settings return is deliberately not an automatic start.
                try {
                    startActivity(Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, packageName))
                } catch (_: ActivityNotFoundException) { status.setText(R.string.permission_unavailable) }
            }
            return
        }
        try {
            startForegroundService(Intent(this, CompanionService::class.java).setAction(CompanionService.ACTION_SHOW))
        } catch (failure: RuntimeException) {
            Log.e("Roamling", "Could not request companion", failure)
            status.setText(R.string.show_failed)
        }
    }

    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == 1 && grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED && resumed) requestShow()
        else refresh()
    }

    private fun refresh() {
        val session = service
        showButton.isEnabled = session != null && (!session.running || session.hidden)
        hideButton.isEnabled = session?.running == true && !session.hidden
        stopButton.isEnabled = session?.running == true
        sizeBar.isEnabled = session != null
        if (session != null && !draggingSize) showSize(session.scale)
        status.setText(when {
            session?.error != null -> checkNotNull(session.error)
            !Settings.canDrawOverlays(this) -> R.string.permission_needed
            !CompanionService.notificationsAllowed(this) -> R.string.notification_needed
            session?.loading == true -> R.string.loading
            session?.hidden == true -> R.string.hidden
            session?.running == true && session.locked -> R.string.locked
            session?.running == true -> R.string.showing
            else -> R.string.ready
        })
    }
    private fun buildScreen() {
        val landscape = resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
        val root = LinearLayout(this).apply {
            orientation = if (landscape) LinearLayout.HORIZONTAL else LinearLayout.VERTICAL
            setBackgroundColor(Color.rgb(250, 247, 240))
            setPadding(dp(28), dp(28), dp(28), dp(28))
            setOnApplyWindowInsetsListener { view, insets ->
                val bars = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
                view.setPadding(dp(28) + bars.left, dp(24) + bars.top, dp(28) + bars.right, dp(24) + bars.bottom)
                insets
            }
        }
        fun label(text: Int, size: Float) = TextView(this).apply {
            setText(text); textSize = size; setTextColor(Color.rgb(60, 57, 52))
            setPadding(0, 0, 0, dp(14))
        }
        val controls = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            if (landscape) setPadding(0, 0, dp(8), 0)
        }
        controls.addView(label(R.string.app_name, 16f))
        controls.addView(label(R.string.preview_title, 30f).apply { setTypeface(typeface, Typeface.BOLD) })
        controls.addView(label(R.string.preview_description, 16f))
        showButton = Button(this).apply {
            id = R.id.show_mochi; setText(R.string.show_mochi); setOnClickListener { requestShow() }
        }
        hideButton = Button(this).apply {
            id = R.id.hide_mochi; setText(R.string.hide_mochi)
            setOnClickListener { service?.hideCompanion() }
        }
        controls.addView(showButton, LinearLayout.LayoutParams(-1, dp(56)))
        controls.addView(hideButton, LinearLayout.LayoutParams(-1, dp(56)))
        stopButton = Button(this).apply {
            id = R.id.stop_mochi; setText(R.string.stop_mochi)
            setOnClickListener { service?.stopCompanion() }
        }
        controls.addView(stopButton, LinearLayout.LayoutParams(-1, dp(56)))
        sizeLabel = label(R.string.size_label, 14f).apply { id = R.id.size_label; setPadding(0, dp(16), 0, 0) }
        controls.addView(sizeLabel)
        sizeBar = SeekBar(this).apply {
            id = R.id.size_bar
            max = SIZE_STEPS
            setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
                override fun onProgressChanged(bar: SeekBar, progress: Int, fromUser: Boolean) {
                    val scale = scaleAt(progress)
                    sizeLabel.text = getString(R.string.size_label, scale)
                    if (fromUser) service?.setScale(scale)
                }
                override fun onStartTrackingTouch(bar: SeekBar) { draggingSize = true }
                override fun onStopTrackingTouch(bar: SeekBar) { draggingSize = false }
            })
        }
        controls.addView(sizeBar, LinearLayout.LayoutParams(-1, dp(48)))
        showSize(PreviewRuntime.MAX_SCALE)
        status = label(R.string.loading, 14f).apply { id = R.id.preview_status; setPadding(0, dp(16), 0, dp(16)) }
        controls.addView(status)
        val scroll = ScrollView(this).apply {
            scrollBarStyle = View.SCROLLBARS_INSIDE_INSET
            addView(controls)
        }
        root.addView(scroll, if (landscape) LinearLayout.LayoutParams(0, -1, 1f) else LinearLayout.LayoutParams(-1, -2))
        stage = TextView(this).apply {
            id = R.id.mochi_stage
            setText(R.string.preview_space)
            textSize = 13f
            setTextColor(Color.rgb(132, 126, 117))
            gravity = Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(20))
            background = GradientDrawable().apply { setColor(Color.rgb(240, 234, 223)); cornerRadius = dp(28).toFloat() }
        }
        root.addView(stage, (if (landscape) LinearLayout.LayoutParams(0, -1, 1f) else LinearLayout.LayoutParams(-1, 0, 1f)).apply {
            if (landscape) marginStart = dp(24)
        })
        setContentView(root)
    }

    private fun showSize(scale: Double) {
        sizeBar.progress = ((scale - PreviewRuntime.MIN_SCALE) / SIZE_STEP).roundToInt().coerceIn(0, SIZE_STEPS)
        sizeLabel.text = getString(R.string.size_label, scaleAt(sizeBar.progress))
    }

    private fun scaleAt(progress: Int): Double =
        PreviewRuntime.clampScale(PreviewRuntime.MIN_SCALE + progress * SIZE_STEP)

    private fun dp(value: Int) = (value * resources.displayMetrics.density).roundToInt()

    private companion object {
        /** 0.10x to 1.00x in hundredths: fine enough to feel continuous, coarse enough to land on a round number. */
        const val SIZE_STEP = 0.01
        const val SIZE_STEPS = 90
    }
}
