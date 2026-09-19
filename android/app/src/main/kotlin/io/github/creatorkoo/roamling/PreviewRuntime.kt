// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
package io.github.creatorkoo.roamling

import android.os.SystemClock
import uniffi.roamling_android.AtlasFrame
import uniffi.roamling_android.MascotAtlas
import uniffi.roamling_android.Player
import uniffi.roamling_android.defaultTuning
import uniffi.roamling_core.FfiDisplay
import uniffi.roamling_core.FfiInteractionOutput
import uniffi.roamling_core.FfiPoint
import uniffi.roamling_core.FfiTickInput
import uniffi.roamling_core.PetLoop
import kotlin.math.hypot

/** Main-thread adapter: the native runtime owns every behavior and timing rule. */
internal class PreviewRuntime(
    atlas: MascotAtlas,
    display: FfiDisplay,
    initial: FfiPoint,
    private val clock: () -> Double = { SystemClock.elapsedRealtimeNanos() / 1_000_000_000.0 },
    seed: ULong = SystemClock.elapsedRealtimeNanos().toULong(),
    initialScale: Double = MAX_SCALE,
) : AutoCloseable {
    private val pet = PetLoop(initial.x, initial.y, defaultTuning(), seed)
    private val player = Player(atlas)
    private var lastTouchAt = clock()
    private var finger: FfiPoint? = null
    private var downAt: FfiPoint? = null
    /** The user's size choice. World units are dp, so this carries nothing about density. */
    var scale: Double = clampScale(initialScale)
        private set
    val width: Double get() = BASE_WIDTH * scale
    val height: Double get() = BASE_HEIGHT * scale
    var frame: AtlasFrame? = null
        private set
    var capability: UByte = 0u
        private set
    var positionNeedsSaving = false
        private set
    val position: FfiPoint get() = pet.position()
    val hasContact: Boolean get() = finger != null
    val interval: Double get() = pet.preferredTickInterval(clock())

    init {
        pet.setObjectSize(width, height)
        pet.handleDisplayChange(listOf(display), initial.x, initial.y, clock())
        // Wire order is declared once in core's PET_CAPABILITIES; no frame timings here.
        pet.setAnimationDurations(player.duration(13u), player.duration(14u))
    }

    fun noteInput() { lastTouchAt = clock() }

    fun tick() {
        val now = clock()
        val point = finger
        val origin = position
        pet.beginTick(now)
        val result = pet.finishTick(FfiTickInput(
            now = now, pointerX = point?.x ?: -10_000.0, pointerY = point?.y ?: -10_000.0,
            primaryButtonDown = point != null, userIdleDuration = (now - lastTouchAt).coerceAtLeast(0.0),
            captureAuthorized = false, focusAuthorized = false, didQueryFocus = false, queriedFocus = null,
            pointerIsOverPet = point != null && kotlin.math.abs(point.x - origin.x) <= width / 2 &&
                kotlin.math.abs(point.y - origin.y) <= height / 2,
            affectionHeld = false,
        ))
        capability = result.capability
        positionNeedsSaving = result.persistPosition
        frame = player.advance(capability, result.deltaTime * result.locomotionRate)
    }

    fun down(x: Double, y: Double): Double {
        noteInput()
        finger = FfiPoint(x, y)
        downAt = FfiPoint(x, y)
        val result = pet.touchDown(x, y, clock())
        if (result.setInteractionEnabled == false) {
            finger = null
            downAt = null
        }
        return apply(result)
    }

    fun move(x: Double, y: Double): Double {
        val start = downAt ?: return interval
        noteInput()
        finger = FfiPoint(x, y)
        val result = pet.pointerDragged(x, y, hypot(x - start.x, y - start.y), clock())
        // Use the core's clamp during contact too, not just at drop.
        pet.setScale(width, height)
        return apply(result)
    }

    fun up(): Double {
        val point = finger ?: return interval
        noteInput()
        finger = null
        downAt = null
        // The core tracks its own drag threshold; Kotlin does not duplicate it.
        return apply(pet.pointerUp(point.x, point.y, false, clock()))
    }

    private fun apply(result: FfiInteractionOutput): Double {
        capability = result.capability
        positionNeedsSaving = result.persistPosition
        frame = player.advance(capability, 0.0)
        return result.rescheduleAfter ?: interval
    }

    /** A new footprint changes where the pet may stand; the core clamps it. */
    fun resize(newScale: Double) {
        scale = clampScale(newScale)
        pet.setScale(width, height)
    }

    /** The area the pet may use changed -- rotation, a fold, or the keyboard. */
    fun setWorld(display: FfiDisplay, roaming: Boolean) {
        val here = position
        pet.handleDisplayChange(listOf(display), here.x, here.y, clock())
        pet.setRoamingEnabled(roaming, clock())
    }

    fun setHidden(hidden: Boolean) {
        if (hidden) up() else noteInput()
        pet.setHidden(hidden)
    }

    override fun close() {
        pet.destroy()
        player.destroy()
    }

    companion object {
        const val BASE_WIDTH = 96.0
        const val BASE_HEIGHT = 104.0
        const val MIN_SCALE = 0.1
        const val MAX_SCALE = 1.0
        fun clampScale(value: Double): Double =
            if (value.isFinite()) value.coerceIn(MIN_SCALE, MAX_SCALE) else MAX_SCALE
    }
}
