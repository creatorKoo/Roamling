// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
package io.github.creatorkoo.roamling

import android.app.KeyguardManager
import android.app.NotificationManager
import android.content.Intent
import android.os.ParcelFileDescriptor
import android.os.SystemClock
import android.widget.Button
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import org.junit.Assert.*
import org.junit.Test

class CompanionServiceTest {
    private val instrumentation get() = InstrumentationRegistry.getInstrumentation()
    private val context get() = instrumentation.targetContext
    private val manager get() = context.getSystemService(NotificationManager::class.java)

    @Test fun homeOtherAppsNotificationControlsAndScreenLockHaveOneOwner() {
        var session: CompanionService? = null
        try {
            ActivityScenario.launch(MainActivity::class.java).use { scenario ->
                await {
                    var ready = false
                    scenario.onActivity { session = it.companionService; ready = it.findViewById<Button>(R.id.show_mochi).isEnabled }
                    ready
                }
                scenario.onActivity { it.findViewById<Button>(R.id.show_mochi).performClick() }
                await { onMain { session?.visible == true } }
                val service = checkNotNull(session)
                val overlay = checkNotNull(service.overlay)
                shell("input keyevent KEYCODE_HOME")
                shell("am start -a android.settings.SETTINGS")
                await { instrumentation.uiAutomation.rootInActiveWindow?.packageName == "com.android.settings" }
                await { onMain { service.visible } }
                assertEquals(1, manager.activeNotifications.count { it.id == CompanionService.NOTIFICATION_ID })
                shell("cmd statusbar expand-notifications")
                await { instrumentation.uiAutomation.rootInActiveWindow?.packageName == "com.android.systemui" }
                capture("a3-notification.png")
                shell("cmd statusbar collapse")

                notificationAction(0) // real notification's Hide PendingIntent
                await { onMain { service.hidden && !service.visible } }
                val pausedTicks = onMainValue { overlay.tickCount }
                SystemClock.sleep(700)
                assertEquals(pausedTicks, onMainValue { overlay.tickCount })
                assertEquals(onMainValue { overlay.position }, onMainValue { service.savedPosition() })

                shell("input keyevent KEYCODE_SLEEP")
                await { onMain { service.locked } }
                shell("input keyevent KEYCODE_WAKEUP")
                unlockBySwipe()
                await { onMain { !service.locked } }
                assertTrue(onMain { service.hidden && !service.visible })
                notificationAction(0) // Show must work while the Activity is stopped
                await { onMain { service.visible && !service.hidden } }

                shell("input keyevent KEYCODE_SLEEP")
                await { onMain { service.locked && !service.visible } }
                val asleepTicks = onMainValue { overlay.tickCount }
                SystemClock.sleep(700)
                assertEquals(asleepTicks, onMainValue { overlay.tickCount })
                shell("input keyevent KEYCODE_WAKEUP")
                await { context.getSystemService(KeyguardManager::class.java).isKeyguardLocked }
                assertFalse(onMain { service.visible })
                unlockBySwipe()
                await(message = { onMainValue { "Unlock: running=${service.running}, hidden=${service.hidden}, locked=${service.locked}, visible=${service.visible}, error=${service.error}" } }) {
                    onMain { service.visible && !service.locked }
                }
                assertSame(overlay, onMainValue { service.overlay })

                notificationAction(1) // Stop
                await { onMain { !service.running && !overlay.isShowing } }
                await { manager.activeNotifications.none { it.id == CompanionService.NOTIFICATION_ID } }
                SystemClock.sleep(700)
                assertFalse(onMain { service.running })
            }
            ActivityScenario.launch(MainActivity::class.java).use { scenario ->
                await {
                    var ready = false
                    scenario.onActivity { session = it.companionService; ready = session != null }
                    ready
                }
                val service = checkNotNull(session)
                assertFalse(onMain { service.running }) // opening the app never restarts it
                val saved = onMainValue { service.savedPosition() }
                assertNotNull(saved)
                scenario.onActivity { it.findViewById<Button>(R.id.show_mochi).performClick() }
                await { onMain { service.visible } }
                val restored = onMainValue { checkNotNull(service.overlay).position }
                assertEquals(checkNotNull(saved).x, checkNotNull(restored).x, 1.0)
                assertEquals(saved.y, restored.y, 1.0)
            }
        } finally {
            shell("input keyevent KEYCODE_WAKEUP")
            shell("wm dismiss-keyguard")
            shell("cmd statusbar collapse")
            instrumentation.runOnMainSync { session?.stopCompanion() }
        }
    }

    @Test fun revokedOverlayStopsTheSession() {
        var session: CompanionService? = null
        try {
            ActivityScenario.launch(MainActivity::class.java).use { scenario ->
                await {
                    var ready = false
                    scenario.onActivity { session = it.companionService; ready = session != null }
                    ready
                }
                scenario.onActivity { it.findViewById<Button>(R.id.show_mochi).performClick() }
                await { onMain { session!!.visible } }
                shell("cmd appops set ${context.packageName} SYSTEM_ALERT_WINDOW ignore")
                await { onMain { !session!!.running && !session!!.visible } }
                await { manager.activeNotifications.none { it.id == CompanionService.NOTIFICATION_ID } }
            }
        } finally {
            shell("cmd appops set ${context.packageName} SYSTEM_ALERT_WINDOW allow")
            instrumentation.runOnMainSync { session?.stopCompanion() }
        }
    }

    private fun notificationAction(index: Int) {
        manager.activeNotifications.single { it.id == CompanionService.NOTIFICATION_ID }.notification.actions[index].actionIntent.send()
    }

    private fun shell(command: String): String = instrumentation.uiAutomation.executeShellCommand(command).use {
        ParcelFileDescriptor.AutoCloseInputStream(it).bufferedReader().readText()
    }

    private fun <T> onMainValue(block: () -> T): T {
        var answer: T? = null
        instrumentation.runOnMainSync { answer = block() }
        @Suppress("UNCHECKED_CAST")
        return answer as T
    }
    private fun onMain(block: () -> Boolean): Boolean = onMainValue(block)
    private fun await(message: () -> String = { "Companion service condition timed out" }, condition: () -> Boolean) {
        val deadline = SystemClock.elapsedRealtime() + 15_000
        while (SystemClock.elapsedRealtime() < deadline) {
            if (condition()) return
            SystemClock.sleep(100)
        }
        fail(message())
    }
    private fun unlockBySwipe() {
        SystemClock.sleep(600) // wait for the real keyguard window after screen-on
        val screen = checkNotNull(instrumentation.uiAutomation.takeScreenshot())
        val width = screen.width
        val height = screen.height
        screen.recycle()
        shell("input swipe ${width / 2} ${height * 4 / 5} ${width / 2} ${height / 5} 400")
        await { !context.getSystemService(KeyguardManager::class.java).isKeyguardLocked }
    }
    private fun capture(name: String) {
        SystemClock.sleep(600)
        val bitmap = checkNotNull(instrumentation.uiAutomation.takeScreenshot())
        try { File(context.getExternalFilesDir(null), name).outputStream().use { bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) } }
        finally { bitmap.recycle() }
    }
}
