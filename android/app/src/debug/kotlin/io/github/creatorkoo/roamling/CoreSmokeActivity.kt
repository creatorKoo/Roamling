// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
package io.github.creatorkoo.roamling

import android.app.Activity
import android.os.Bundle
import android.os.SystemClock
import android.util.Log
import android.view.WindowInsets
import uniffi.roamling_android.defaultTuning
import uniffi.roamling_core.FfiDisplay
import uniffi.roamling_core.FfiRect
import uniffi.roamling_core.FfiTickInput
import uniffi.roamling_core.PetLoop

/** Exercises the same native boundary A1 will render, without an overlay. */
class CoreSmokeActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // The runner matches this launch, never a stale PASS from logcat.
        val token = intent.getStringExtra("smoke_token").orEmpty()
        require(token.matches(Regex("[a-zA-Z0-9-]{1,64}")))
        try {
            val metrics = windowManager.currentWindowMetrics
            val density = resources.displayMetrics.density.toDouble()
            val insets = metrics.windowInsets.getInsetsIgnoringVisibility(
                WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout()
            )
            val bounds = metrics.bounds
            val frame = FfiRect(
                (bounds.left + insets.left) / density,
                (bounds.top + insets.top) / density,
                (bounds.width() - insets.left - insets.right) / density,
                (bounds.height() - insets.top - insets.bottom) / density,
            )
            check(frame.width > 0.0 && frame.height > 0.0)
            val x = frame.x + frame.width / 2
            val y = frame.y + frame.height / 2
            // Reads a record from Android's UniFFI component, then passes it
            // into the core component; both must load from the same .so.
            val runtime = PetLoop(x, y, defaultTuning(), 1uL)
            val result = try {
                runtime.setDisplays(listOf(FfiDisplay("android", frame, frame)))
                runtime.setObjectSize(96.0, 104.0)
                runtime.setFlags(false, false, false)
                val now = SystemClock.elapsedRealtime() / 1000.0
                runtime.beginTick(now)
                val tick = runtime.finishTick(FfiTickInput(
                    now, -10_000.0, -10_000.0, false, 0.0,
                    false, false, false, null, false, false,
                ))
                check(tick.x.isFinite() && tick.y.isFinite())
                check(tick.x == x && tick.y == y) { "A stationary tick moved the pet" }
                // Requests are advisory; permission is enforced by the shell.
                // A0 has no capture provider and deliberately ignores them.
                "A0 PASS token=$token x=${tick.x} y=${tick.y} state=${tick.state}"
            } finally {
                runtime.destroy()
            }
            Log.i(TAG, result)
        } catch (failure: Throwable) {
            Log.e(TAG, "A0 FAIL token=$token", failure)
            throw failure
        } finally {
            finish()
        }
    }

    companion object { private const val TAG = "RoamlingA0" }
}
