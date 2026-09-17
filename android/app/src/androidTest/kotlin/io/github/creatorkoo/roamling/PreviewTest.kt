// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
package io.github.creatorkoo.roamling

import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.Rect
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import android.os.SystemClock
import android.view.InputDevice
import android.view.MotionEvent
import android.view.WindowInsets
import android.view.WindowManager
import android.provider.Settings
import android.widget.Button
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import org.junit.Assert.*
import org.junit.Test
import uniffi.roamling_core.FfiPetImage
import uniffi.roamling_core.FfiDisplay
import uniffi.roamling_core.FfiPoint
import uniffi.roamling_core.FfiRect
import uniffi.roamling_android.defaultTuning

class PreviewTest {
    private val instrumentation get() = InstrumentationRegistry.getInstrumentation()

    @Test fun sharedRuntimeWalksRestsWakesAndClampsDirectContact() {
        val images = MochiImages.load()
        var now = 100.0
        val bounds = FfiRect(0.0, 24.0, 412.0, 800.0)
        try {
            PreviewRuntime(images.atlas, FfiDisplay("test", bounds, bounds), FfiPoint(150.0, 350.0), { now }, 7u).use { pet ->
                val seen = mutableSetOf<UByte>()
                val start = pet.position
                var moved = false
                // Advance a controlled clock at the actual requested cadence, using unchanged tuning.
                val deadline = now + defaultTuning().idleBeforeRest + 180.0
                while (now < deadline && pet.capability != 4.toUByte()) {
                    pet.tick()
                    seen.add(pet.capability)
                    moved = moved || kotlin.math.abs(pet.position.x - start.x) > 5.0 || kotlin.math.abs(pet.position.y - start.y) > 5.0
                    assertTrue(pet.position.x in 48.0..364.0 && pet.position.y in 76.0..772.0)
                    now += pet.interval
                }
                assertTrue("The shared core must walk", moved && (1.toUByte() in seen || 2.toUByte() in seen))
                assertEquals("The real idle threshold must lead to sleep", 4.toUByte(), pet.capability)
                assertEquals(0.5, pet.interval, 0.001)
                pet.noteInput(); pet.tick()
                assertNotEquals(4.toUByte(), pet.capability)
                val point = pet.position
                pet.down(point.x + 40, point.y + 50)
                assertTrue(pet.hasContact)
                pet.move(2000.0, -2000.0)
                assertTrue(pet.position.x in 48.0..364.0 && pet.position.y in 76.0..772.0)
                pet.up()
                assertFalse(pet.hasContact)
                // Release cannot leave a ghost finger keeping the pet awake.
                val sleepAgain = now + defaultTuning().idleBeforeRest + 180.0
                while (now < sleepAgain && pet.capability != 4.toUByte()) {
                    now += pet.interval; pet.tick()
                }
                assertEquals(4.toUByte(), pet.capability)
            }
        } finally { images.close() }
    }

    @Test fun premultipliedRgbaReachesAndroidWithoutChannelSwapOrDoubleMultiply() {
        val image = FfiPetImage(3u, 1u, byteArrayOf(
            64, 32, 16, 128.toByte(), // premultiplied half-transparent brown
            240.toByte(), 20, 10, 255.toByte(),
            0, 0, 0, 0,
        ))
        val bitmap = MochiImages.bitmap(image)
        try {
            assertTrue(bitmap.isPremultiplied)
            val half = bitmap.getPixel(0, 0)
            assertEquals(128, Color.alpha(half))
            assertEquals(128, Color.red(half))
            assertEquals(64, Color.green(half))
            assertEquals(32, Color.blue(half))
            assertEquals(Color.rgb(240, 20, 10), bitmap.getPixel(1, 0))
            assertEquals(Color.TRANSPARENT, bitmap.getPixel(2, 0))
        } finally { bitmap.recycle() }
    }

    @Test fun overlayShowsHidesRecreatesAndContinuesBeyondTheActivity() {
        assertTrue("Run on the explicitly granted test emulator", Settings.canDrawOverlays(instrumentation.targetContext))
        var runningSession: CompanionService? = null
        try {
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            lateinit var current: MainActivity
            awaitCondition {
                var ready = false
                scenario.onActivity {
                    current = it; runningSession = it.companionService
                    ready = it.findViewById<Button>(R.id.show_mochi).isEnabled
                }
                ready
            }
            repeat(2) {
                scenario.onActivity { it.findViewById<Button>(R.id.show_mochi).performClick() }
                awaitCondition { onMain { current.previewIsAttached && current.findViewById<Button>(R.id.hide_mochi).isEnabled } }
                assertFalse(onMain { current.findViewById<Button>(R.id.show_mochi).isEnabled })
                scenario.onActivity { it.findViewById<Button>(R.id.hide_mochi).performClick() }
                assertFalse(onMain { current.previewIsAttached })
                assertTrue(onMain { current.findViewById<Button>(R.id.show_mochi).isEnabled })
            }
            scenario.onActivity { it.findViewById<Button>(R.id.show_mochi).performClick() }
            awaitCondition { onMain { current.previewIsAttached } }
            var first = Rect()
            scenario.onActivity { first = checkNotNull(it.previewOverlay).boundsOnScreen }
            assertPreviewAnimates(scenario)
            // Capture the starting position before screenshot sampling, which
            // can outlast a short first walk. A natural rest may last 40 seconds.
            awaitCondition(timeoutMs = (defaultTuning().wanderPause * 1000).toLong() + 15_000) { onMain {
                val next = checkNotNull(current.previewOverlay).boundsOnScreen
                kotlin.math.abs(next.left - first.left) + kotlin.math.abs(next.top - first.top) > 10
            } }
            assertSystemDrag(scenario)
            assertCancelAndMultipleContacts(scenario)
            capture("a3-preview.png")
            val old = current
            val originalOverlay = current.previewOverlay
            scenario.recreate()
            awaitCondition {
                var attached = false
                scenario.onActivity { current = it; attached = it.previewIsAttached }
                attached
            }
            assertNotSame(old, current)
            assertFalse(onMain { old.previewIsAttached })
            assertSame(originalOverlay, current.previewOverlay)
            scenario.onActivity { it.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE }
            awaitCondition {
                var attached = false
                scenario.onActivity {
                    current = it
                    attached = it.resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE && it.previewIsAttached
                }
                attached
            }
            assertWithinSafeScreen(scenario)
            capture("a3-preview-landscape.png")
            val session = checkNotNull(current.companionService)
            val activeOverlay = checkNotNull(current.previewOverlay)
            instrumentation.uiAutomation.executeShellCommand("input keyevent KEYCODE_HOME").use {
                android.os.ParcelFileDescriptor.AutoCloseInputStream(it).readBytes()
            }
            // Home can rotate the display back to portrait and replace the
            // landscape window. Follow its service owner, not the old View.
            awaitCondition { onMain { !current.previewIsAttached && session.visible && session.running } }
            awaitCondition { instrumentation.uiAutomation.rootInActiveWindow?.packageName?.toString()?.contains("launcher") == true }
            capture("a3-home.png")
            instrumentation.runOnMainSync { session.stopCompanion() }
            assertFalse(onMain { activeOverlay.isShowing })
        }
        } finally { instrumentation.runOnMainSync { runningSession?.stopCompanion() } }
    }

    private fun assertSystemDrag(scenario: ActivityScenario<MainActivity>) {
        var before = Rect()
        var density = 1f
        scenario.onActivity {
            before = checkNotNull(it.previewOverlay).boundsOnScreen
            density = it.resources.displayMetrics.density
        }
        val downTime = SystemClock.uptimeMillis()
        fun inject(action: Int, x: Float, y: Float) {
            val event = MotionEvent.obtain(downTime, SystemClock.uptimeMillis(), action, x, y, 0)
            event.source = InputDevice.SOURCE_TOUCHSCREEN
            try { assertTrue(instrumentation.uiAutomation.injectInputEvent(event, true)) }
            finally { event.recycle() }
        }
        val x = before.exactCenterX()
        val y = before.exactCenterY()
        // Move inward from either screen half, well away from system gestures.
        val targetX = x + (if (before.left > 150 * density) -70 else 70) * density
        val targetY = y - 60 * density
        inject(MotionEvent.ACTION_DOWN, x, y)
        repeat(12) { step ->
            SystemClock.sleep(25)
            inject(MotionEvent.ACTION_MOVE, x + (targetX - x) * (step + 1) / 12, y + (targetY - y) * (step + 1) / 12)
        }
        inject(MotionEvent.ACTION_UP, targetX, targetY)
        instrumentation.waitForIdleSync()
        scenario.onActivity {
            val overlay = checkNotNull(it.previewOverlay)
            assertFalse(overlay.hasContact)
            val after = overlay.boundsOnScreen
            assertEquals(before.left + targetX - x, after.left.toFloat(), 4f)
            assertEquals(before.top + targetY - y, after.top.toFloat(), 4f)
        }
        assertWithinSafeScreen(scenario)
    }

    private fun assertCancelAndMultipleContacts(scenario: ActivityScenario<MainActivity>) {
        scenario.onActivity {
            val overlay = checkNotNull(it.previewOverlay)
            val bounds = overlay.boundsOnScreen
            val time = SystemClock.uptimeMillis()
            fun send(action: Int, count: Int = 1) {
                val properties = Array(count) { id -> MotionEvent.PointerProperties().apply { this.id = id; toolType = MotionEvent.TOOL_TYPE_FINGER } }
                val coords = Array(count) { id -> MotionEvent.PointerCoords().apply {
                    x = bounds.exactCenterX() + id * 4; y = bounds.exactCenterY() + id * 4; pressure = 1f; size = 1f
                } }
                val event = MotionEvent.obtain(time, SystemClock.uptimeMillis(), action, count, properties, coords,
                    0, 0, 1f, 1f, 0, 0, InputDevice.SOURCE_TOUCHSCREEN, 0)
                try { overlay.dispatchContact(event) } finally { event.recycle() }
            }
            send(MotionEvent.ACTION_DOWN)
            assertTrue(overlay.hasContact)
            send(MotionEvent.ACTION_POINTER_DOWN or (1 shl MotionEvent.ACTION_POINTER_INDEX_SHIFT), 2)
            send(MotionEvent.ACTION_POINTER_UP, 2) // original finger leaves; second cannot take ownership
            assertFalse(overlay.hasContact)
            send(MotionEvent.ACTION_MOVE) // no new DOWN, no new grab
            assertFalse(overlay.hasContact)
            send(MotionEvent.ACTION_DOWN)
            send(MotionEvent.ACTION_CANCEL)
            assertFalse(overlay.hasContact)
        }
    }

    private fun assertWithinSafeScreen(scenario: ActivityScenario<MainActivity>) {
        scenario.onActivity {
            val metrics = it.getSystemService(WindowManager::class.java).currentWindowMetrics
            val safe = Rect(metrics.bounds)
            val insets = metrics.windowInsets.getInsetsIgnoringVisibility(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
            safe.inset(insets)
            val bounds = checkNotNull(it.previewOverlay).boundsOnScreen
            assertTrue("$bounds must be inside $safe", safe.contains(bounds))
        }
    }

    private fun onMain(block: () -> Boolean): Boolean {
        var answer = false
        instrumentation.runOnMainSync { answer = block() }
        return answer
    }

    private fun assertPreviewAnimates(scenario: ActivityScenario<MainActivity>) {
        val bounds = Rect()
        scenario.onActivity { bounds.set(checkNotNull(it.previewOverlay).boundsOnScreen) }
        fun frame(): Bitmap {
            val screen = checkNotNull(instrumentation.uiAutomation.takeScreenshot())
            return try {
                Bitmap.createBitmap(screen, bounds.left, bounds.top, bounds.width(), bounds.height())
            } finally { screen.recycle() }
        }
        SystemClock.sleep(500) // settle first composition before comparing actual stage pixels
        val first = frame()
        try {
            awaitCondition {
                val next = frame()
                try { !first.sameAs(next) } finally { next.recycle() }
            }
        } finally { first.recycle() }
    }

    private fun awaitCondition(timeoutMs: Long = 15_000, condition: () -> Boolean) {
        val deadline = SystemClock.elapsedRealtime() + timeoutMs
        while (SystemClock.elapsedRealtime() < deadline) {
            if (condition()) return
            SystemClock.sleep(100)
        }
        fail("Preview condition timed out")
    }

    private fun capture(name: String) {
        SystemClock.sleep(300) // allow the attached window's first composition
        val screenshot = checkNotNull(instrumentation.uiAutomation.takeScreenshot())
        try {
            File(instrumentation.targetContext.getExternalFilesDir(null), name).outputStream().use {
                check(screenshot.compress(Bitmap.CompressFormat.PNG, 100, it))
            }
        } finally { screenshot.recycle() }
    }
}
