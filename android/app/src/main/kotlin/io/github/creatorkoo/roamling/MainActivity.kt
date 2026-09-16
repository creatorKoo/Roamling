// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
package io.github.creatorkoo.roamling

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Bundle
import android.provider.Settings
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.WindowInsets
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import java.util.concurrent.Executors
import kotlin.math.roundToInt

class MainActivity : Activity() {
    private val loader = Executors.newSingleThreadExecutor()
    private var images: MochiImages? = null
    private var overlay: MochiOverlay? = null
    private var resumed = false
    private var showRequested = false
    private var awaitingPermission = false
    private var loadFailed = false
    private lateinit var showButton: Button
    private lateinit var hideButton: Button
    private lateinit var status: TextView
    private lateinit var stage: View
    internal val previewIsAttached: Boolean get() = overlay?.isShowing == true

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        showRequested = savedInstanceState?.getBoolean("show_requested") ?: false
        awaitingPermission = savedInstanceState?.getBoolean("awaiting_permission") ?: false
        buildScreen()
        loader.execute {
            val loaded = runCatching { MochiImages.load() }
            runOnUiThread {
                if (isDestroyed || isFinishing) {
                    loaded.getOrNull()?.close()
                } else {
                    images = loaded.getOrNull()
                    loadFailed = loaded.isFailure
                    loaded.exceptionOrNull()?.let { Log.e("Roamling", "Mochi load failed", it) }
                    refresh()
                }
            }
        }
    }

    override fun onResume() {
        super.onResume()
        resumed = true
        if (awaitingPermission) {
            awaitingPermission = false
            showRequested = Settings.canDrawOverlays(this)
        }
        refresh()
    }

    override fun onSaveInstanceState(outState: Bundle) {
        outState.putBoolean("show_requested", showRequested || previewIsAttached)
        outState.putBoolean("awaiting_permission", awaitingPermission)
        super.onSaveInstanceState(outState)
    }

    override fun onStop() {
        resumed = false
        // On Android P+, state saving can happen after onStop. Keep the
        // display intent for configuration recreation before removing the view.
        showRequested = isChangingConfigurations && (showRequested || previewIsAttached)
        removeOverlay()
        super.onStop()
    }

    override fun onDestroy() {
        removeOverlay()
        images?.close()
        images = null
        loader.shutdown()
        super.onDestroy()
    }

    private fun requestShow() {
        if (!Settings.canDrawOverlays(this)) {
            awaitingPermission = true
            try {
                startActivity(Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:$packageName")))
            } catch (_: ActivityNotFoundException) {
                awaitingPermission = false
                status.setText(R.string.permission_unavailable)
            }
        } else {
            showRequested = true
            refresh()
        }
    }

    private fun removeOverlay() {
        overlay?.close()
        overlay = null
    }

    private fun refresh() {
        showButton.isEnabled = images != null && !previewIsAttached
        hideButton.isEnabled = previewIsAttached
        status.setText(when {
            loadFailed -> R.string.load_failed
            images == null -> R.string.loading
            !Settings.canDrawOverlays(this) -> R.string.permission_needed
            previewIsAttached -> R.string.showing
            else -> R.string.ready
        })
        if (!resumed || !showRequested || images == null || previewIsAttached) return
        stage.post {
            if (!resumed || !showRequested || previewIsAttached || isDestroyed) return@post
            if (!Settings.canDrawOverlays(this)) {
                showRequested = false
                refresh()
                return@post
            }
            val location = IntArray(2)
            stage.getLocationOnScreen(location)
            val next = MochiOverlay(this, checkNotNull(images))
            try {
                next.show(location[0] + stage.width / 2, location[1] + stage.height / 2) { refresh() }
                overlay = next
                showRequested = false
                refresh()
            } catch (failure: RuntimeException) {
                next.close()
                showRequested = false
                Log.e("Roamling", "Overlay attach failed", failure)
                status.setText(R.string.show_failed)
            }
        }
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
            setOnClickListener { showRequested = false; removeOverlay(); refresh() }
        }
        controls.addView(showButton, LinearLayout.LayoutParams(-1, dp(56)))
        controls.addView(hideButton, LinearLayout.LayoutParams(-1, dp(56)))
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

    private fun dp(value: Int) = (value * resources.displayMetrics.density).roundToInt()
}
