// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
package io.github.creatorkoo.roamling

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.Rect
import android.graphics.RectF
import android.hardware.display.DisplayManager
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Choreographer
import android.view.Display
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowInsets
import android.view.WindowManager
import uniffi.roamling_android.AtlasFrame
import uniffi.roamling_core.FfiDisplay
import uniffi.roamling_core.FfiPoint
import uniffi.roamling_core.FfiRect
import kotlin.math.ceil
import kotlin.math.roundToInt

/** With the keyboard up the pet keeps to this many of its own heights under the status bar... */
private const val KEYBOARD_BAND_BODIES = 1.5
/** ...or this share of the usable screen, whichever is taller. */
private const val KEYBOARD_BAND_SHARE = 0.28

/** Service-owned window and input adapter; all behavior lives in the shared core. */
internal class MochiOverlay(
    context: Context,
    private val images: MochiImages,
    private val savePosition: (FfiPoint) -> Unit = {},
    initialScale: Double = PreviewRuntime.MAX_SCALE,
    private val onFailure: (RuntimeException) -> Unit = { throw it },
) : AutoCloseable {
    private val windowContext = context.createDisplayContext(
        context.getSystemService(DisplayManager::class.java).getDisplay(Display.DEFAULT_DISPLAY)
    ).createWindowContext(WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY, null)
    private val manager = windowContext.getSystemService(WindowManager::class.java)
    private val choreographer = Choreographer.getInstance()
    private val handler = Handler(Looper.getMainLooper())
    private val density = windowContext.resources.displayMetrics.density.toDouble()
    private var runtime: PreviewRuntime? = null
    private var params: WindowManager.LayoutParams? = null
    private var closed = false
    private var windowAdded = false
    private var visibleRequested = false
    private var attachmentGeneration = 0
    private var scale = PreviewRuntime.clampScale(initialScale)
    private var fullWorld: FfiRect? = null
    internal var imeVisible = false
        private set
    /** Sees whether the soft keyboard is up. Draws nothing and takes no touch. */
    private val imeWatcher = View(windowContext).apply {
        setOnApplyWindowInsetsListener { _, insets ->
            onImeChanged(insets.isVisible(WindowInsets.Type.ime()))
            insets
        }
    }
    private var imeWatcherAdded = false
    private val view = SpriteView(windowContext)
    val isShowing: Boolean get() = visibleRequested && view.isAttachedToWindow
    internal var tickCount = 0L
        private set
    val position: FfiPoint? get() = runtime?.position
    internal val boundsOnScreen: Rect get() {
        val location = IntArray(2)
        view.getLocationOnScreen(location)
        return Rect(location[0], location[1], location[0] + view.width, location[1] + view.height)
    }
    internal val hasContact: Boolean get() = runtime?.hasContact == true
    internal fun dispatchContact(event: MotionEvent): Boolean = view.dispatchTouchEvent(event)

    private val delayedTick = Runnable { tick() }

    private val animate = object : Choreographer.FrameCallback {
        override fun doFrame(frameTimeNanos: Long) {
            tick()
        }
    }

    private fun tick() {
        if (closed || !isShowing) return
        val active = runtime ?: return
        tickCount++
        active.tick()
        render()
        if (!closed) schedule(active.interval)
    }

    private fun schedule(seconds: Double) {
        choreographer.removeFrameCallback(animate)
        handler.removeCallbacks(delayedTick)
        if (closed || !isShowing) return
        if (seconds <= 1.0 / 60.0) choreographer.postFrameCallback(animate)
        else handler.postDelayed(delayedTick, ceil(seconds * 1000).toLong())
    }

    fun noteInput() {
        runtime?.noteInput()
        schedule(0.0)
    }

    private fun render() {
        val active = runtime ?: return
        val layout = params ?: return
        val point = active.position
        if (active.positionNeedsSaving) savePosition(point)
        val x = (point.x * density - layout.width / 2.0).roundToInt()
        val y = (point.y * density - layout.height / 2.0).roundToInt()
        if (layout.x != x || layout.y != y) {
            layout.x = x
            layout.y = y
            try { manager.updateViewLayout(view, layout) }
            catch (failure: RuntimeException) { onFailure(failure); return }
        }
        if (active.frame != view.frame) {
            view.frame = active.frame
            view.invalidate()
        }
    }

    // Core/raw touch coordinates have a physical left origin even in RTL locales.
    @SuppressLint("RtlHardcoded")
    fun show(centerX: Int? = null, centerY: Int? = null, carried: FfiPoint? = null, onShown: () -> Unit) {
        check(!closed)
        if (isShowing) return
        if (runtime != null) { resume(onShown); return }
        val width = (PreviewRuntime.BASE_WIDTH * scale * density).roundToInt().coerceAtLeast(1)
        val height = (PreviewRuntime.BASE_HEIGHT * scale * density).roundToInt().coerceAtLeast(1)
        val metrics = manager.currentWindowMetrics
        val insets = metrics.windowInsets.getInsetsIgnoringVisibility(
            WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout()
        )
        val bounds = metrics.bounds
        val left = bounds.left + insets.left
        val top = bounds.top + insets.top
        // Android's entire world excludes system UI. Core positions are
        // centers; only WindowManager uses the sprite's top-left corner.
        val safe = FfiRect(left / density, top / density,
            (bounds.width() - insets.left - insets.right) / density,
            (bounds.height() - insets.top - insets.bottom) / density)
        fullWorld = safe
        val display = FfiDisplay("android", safe, safe)
        val active = PreviewRuntime(images.atlas, display,
            carried ?: FfiPoint((centerX ?: bounds.centerX()) / density, (centerY ?: bounds.centerY()) / density),
            initialScale = scale)
        runtime = active
        view.smooth = scale < 1.0
        val params = WindowManager.LayoutParams(
            width, height, WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_WATCH_OUTSIDE_TOUCH or
                WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
            PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.LEFT
            setFitInsetsTypes(0)
            x = (active.position.x * density - width / 2.0).roundToInt()
            y = (active.position.y * density - height / 2.0).roundToInt()
            title = "Roamling Bori"
        }
        this.params = params
        active.tick()
        view.frame = active.frame
        resume(onShown)
    }

    private fun resume(onShown: () -> Unit) {
        check(!closed)
        runtime?.setHidden(false)
        visibleRequested = true
        val generation = ++attachmentGeneration
        manager.addView(view, checkNotNull(params))
        windowAdded = true
        watchKeyboard()
        // addView returns before attachment. Report the actual visible state
        // on the next UI turn so Show/Hide controls do not stay stale.
        view.post {
            if (!closed && isShowing && generation == attachmentGeneration) {
                onShown()
                schedule(checkNotNull(runtime).interval)
            }
        }
    }

    /** Live from the size bar: the core re-clamps, the window follows. */
    fun resize(newScale: Double) {
        scale = PreviewRuntime.clampScale(newScale)
        val active = runtime ?: return
        val layout = params ?: return
        active.resize(scale)
        layout.width = (active.width * density).roundToInt().coerceAtLeast(1)
        layout.height = (active.height * density).roundToInt().coerceAtLeast(1)
        view.smooth = scale < 1.0
        // Force the move: the size changed even when the centre did not.
        layout.x = Int.MIN_VALUE
        if (windowAdded) render()
        view.invalidate()
    }

    /**
     * A one-pixel window the height of the screen. `FLAG_ALT_FOCUSABLE_IM` on a
     * non-focusable window is what makes the system tell it about the keyboard;
     * the window the pet is drawn in is never told. Only presence arrives -- an
     * overlay sits above the keyboard, so its height is reported as zero
     * (measured; docs/android.md, R17).
     */
    @SuppressLint("RtlHardcoded")
    private fun watchKeyboard() {
        if (imeWatcherAdded) return
        val layout = WindowManager.LayoutParams(
            1, WindowManager.LayoutParams.MATCH_PARENT, WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM or WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
            PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.LEFT
            setFitInsetsTypes(0)
            title = "Roamling keyboard watch"
        }
        try { manager.addView(imeWatcher, layout); imeWatcherAdded = true }
        catch (failure: RuntimeException) { Log.w("Roamling", "Keyboard watch unavailable", failure) }
    }

    private fun onImeChanged(visible: Boolean) {
        if (visible == imeVisible) return
        imeVisible = visible
        Log.i("Roamling", "keyboard visible=$visible")
        applyWorld()
    }

    /**
     * While the keyboard is up the pet keeps to a band under the status bar and
     * sits still: the keyboard's height is unknowable from here, and on a phone
     * the space just above it is the text being typed.
     */
    private fun applyWorld() {
        val active = runtime ?: return
        val full = fullWorld ?: return
        val world = if (!imeVisible) full else {
            val band = maxOf(active.height * KEYBOARD_BAND_BODIES, full.height * KEYBOARD_BAND_SHARE)
                .coerceAtMost(full.height)
            FfiRect(full.x, full.y, full.width, band)
        }
        active.setWorld(FfiDisplay("android", world, world), roaming = !imeVisible)
        if (windowAdded) { render(); schedule(0.0) }
    }

    fun pause() {
        visibleRequested = false
        attachmentGeneration++
        choreographer.removeFrameCallback(animate)
        handler.removeCallbacks(delayedTick)
        runtime?.setHidden(true)
        position?.let(savePosition)
        if (windowAdded) {
            manager.removeViewImmediate(view)
            windowAdded = false
        }
        if (imeWatcherAdded) {
            manager.removeViewImmediate(imeWatcher)
            imeWatcherAdded = false
            imeVisible = false
        }
    }

    override fun close() {
        if (closed) return
        closed = true
        pause()
        runtime?.close()
        runtime = null
    }

    private inner class SpriteView(context: Context) : View(context) {
        var frame: AtlasFrame? = null
        private val paint = Paint().apply { isFilterBitmap = false; isAntiAlias = false }
        /** Pixel art is drawn unfiltered at full size; shrunk by a non-integer factor it needs the filter. */
        var smooth: Boolean = false
            set(value) { field = value; paint.isFilterBitmap = value }
        private val source = Rect()
        private val destination = RectF()
        private var pointerId = MotionEvent.INVALID_POINTER_ID

        override fun onTouchEvent(event: MotionEvent): Boolean {
            val active = runtime ?: return false
            when (event.actionMasked) {
                MotionEvent.ACTION_OUTSIDE -> { noteInput(); return false }
                MotionEvent.ACTION_DOWN -> {
                    if (pointerId != MotionEvent.INVALID_POINTER_ID) active.up()
                    pointerId = event.getPointerId(0)
                    schedule(active.down(event.rawX / density, event.rawY / density))
                }
                MotionEvent.ACTION_MOVE -> {
                    val index = event.findPointerIndex(pointerId)
                    if (index >= 0) schedule(active.move(event.getRawX(index) / density, event.getRawY(index) / density))
                    else release(active)
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
                    if (event.getPointerId(event.actionIndex) == pointerId) {
                        active.move(event.getRawX(event.actionIndex) / density, event.getRawY(event.actionIndex) / density)
                        release(active)
                        if (event.actionMasked == MotionEvent.ACTION_UP) performClick()
                    }
                }
                MotionEvent.ACTION_CANCEL -> release(active)
            }
            render()
            return true
        }

        private fun release(active: PreviewRuntime) {
            pointerId = MotionEvent.INVALID_POINTER_ID
            schedule(active.up())
        }

        override fun performClick(): Boolean { super.performClick(); return true }

        override fun onDraw(canvas: Canvas) {
            val cell = frame ?: return
            val bitmap = if (cell.extension) images.extension else images.standard
            source.set(cell.x.toInt(), cell.y.toInt(), (cell.x + cell.width).toInt(), (cell.y + cell.height).toInt())
            destination.set(0f, 0f, width.toFloat(), height.toFloat())
            canvas.drawBitmap(bitmap, source, destination, paint)
        }
    }
}
