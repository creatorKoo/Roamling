// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
package io.github.creatorkoo.roamling

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.Rect
import android.graphics.RectF
import android.view.Choreographer
import android.view.Gravity
import android.view.View
import android.view.WindowInsets
import android.view.WindowManager
import uniffi.roamling_android.AtlasFrame
import uniffi.roamling_android.Player
import kotlin.math.roundToInt

/** A1's visible-Activity preview. No background owner or persistent timer. */
internal class MochiOverlay(context: Context, private val images: MochiImages) : AutoCloseable {
    private val windowContext = context.createWindowContext(WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY, null)
    private val manager = windowContext.getSystemService(WindowManager::class.java)
    private val choreographer = Choreographer.getInstance()
    private val player = Player(images.atlas)
    private var lastTime = 0L
    private var closed = false
    private var windowAdded = false
    private val view = SpriteView(windowContext)
    val isShowing: Boolean get() = view.isAttachedToWindow

    private val animate = object : Choreographer.FrameCallback {
        override fun doFrame(frameTimeNanos: Long) {
            if (closed || !isShowing) return
            val delta = if (lastTime == 0L) 0.0 else (frameTimeNanos - lastTime) / 1_000_000_000.0
            lastTime = frameTimeNanos
            // Only idle in A1. A2 will supply FfiTickOutput.capability/delta.
            val next = player.advance(0u, delta)
            if (next != view.frame) {
                view.frame = next
                view.invalidate()
            }
            choreographer.postFrameCallback(this)
        }
    }

    fun show(centerX: Int, centerY: Int, onShown: () -> Unit) {
        check(!closed)
        if (isShowing) return
        val density = windowContext.resources.displayMetrics.density
        val width = (96 * density).roundToInt()
        val height = (104 * density).roundToInt()
        val metrics = manager.currentWindowMetrics
        val insets = metrics.windowInsets.getInsetsIgnoringVisibility(
            WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout()
        )
        val bounds = metrics.bounds
        val left = bounds.left + insets.left
        val top = bounds.top + insets.top
        val right = (bounds.right - insets.right - width).coerceAtLeast(left)
        val bottom = (bounds.bottom - insets.bottom - height).coerceAtLeast(top)
        val params = WindowManager.LayoutParams(
            width, height, WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
            PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            setFitInsetsTypes(0)
            x = (centerX - width / 2).coerceIn(left, right)
            y = (centerY - height / 2).coerceIn(top, bottom)
            title = "Roamling Bori"
        }
        view.frame = player.advance(0u, 0.0)
        manager.addView(view, params)
        windowAdded = true
        // addView returns before attachment. Report the actual visible state
        // on the next UI turn so Show/Hide controls do not stay stale.
        view.post {
            if (!closed && isShowing) {
                onShown()
                choreographer.postFrameCallback(animate)
            }
        }
    }

    override fun close() {
        if (closed) return
        closed = true
        choreographer.removeFrameCallback(animate)
        if (windowAdded) {
            manager.removeViewImmediate(view)
            windowAdded = false
        }
        player.destroy()
    }

    private inner class SpriteView(context: Context) : View(context) {
        var frame: AtlasFrame? = null
        private val paint = Paint().apply { isFilterBitmap = false; isAntiAlias = false }
        private val source = Rect()
        private val destination = RectF()

        override fun onDraw(canvas: Canvas) {
            val cell = frame ?: return
            val bitmap = if (cell.extension) images.extension else images.standard
            source.set(cell.x.toInt(), cell.y.toInt(), (cell.x + cell.width).toInt(), (cell.y + cell.height).toInt())
            destination.set(0f, 0f, width.toFloat(), height.toFloat())
            canvas.drawBitmap(bitmap, source, destination, paint)
        }
    }
}
