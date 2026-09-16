// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
package io.github.creatorkoo.roamling

import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.Rect
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import android.os.SystemClock
import android.provider.Settings
import android.widget.Button
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import org.junit.Assert.*
import org.junit.Test
import uniffi.roamling_core.FfiPetImage

class PreviewTest {
    private val instrumentation get() = InstrumentationRegistry.getInstrumentation()

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

    @Test fun overlayShowsHidesRecreatesAndLeavesWithTheActivity() {
        assertTrue("Run on the explicitly granted test emulator", Settings.canDrawOverlays(instrumentation.targetContext))
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            lateinit var current: MainActivity
            awaitCondition {
                var ready = false
                scenario.onActivity { current = it; ready = it.findViewById<Button>(R.id.show_mochi).isEnabled }
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
            assertPreviewAnimates(scenario)
            capture("a1-preview.png")
            val old = current
            scenario.recreate()
            awaitCondition {
                var attached = false
                scenario.onActivity { current = it; attached = it.previewIsAttached }
                attached
            }
            assertNotSame(old, current)
            assertFalse(onMain { old.previewIsAttached })
            scenario.onActivity { it.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE }
            awaitCondition {
                var attached = false
                scenario.onActivity {
                    current = it
                    attached = it.resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE && it.previewIsAttached
                }
                attached
            }
            capture("a1-preview-landscape.png")
            instrumentation.uiAutomation.executeShellCommand("input keyevent KEYCODE_HOME").use {
                android.os.ParcelFileDescriptor.AutoCloseInputStream(it).readBytes()
            }
            awaitCondition { onMain { !current.previewIsAttached } }
        }
    }

    private fun onMain(block: () -> Boolean): Boolean {
        var answer = false
        instrumentation.runOnMainSync { answer = block() }
        return answer
    }

    private fun assertPreviewAnimates(scenario: ActivityScenario<MainActivity>) {
        val bounds = Rect()
        scenario.onActivity { it.findViewById<android.view.View>(R.id.mochi_stage).getGlobalVisibleRect(bounds) }
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

    private fun awaitCondition(condition: () -> Boolean) {
        val deadline = SystemClock.elapsedRealtime() + 15_000
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
