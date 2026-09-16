// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
plugins {
    // AGP 9 includes Kotlin; do not apply a second Kotlin Android plugin.
    id("com.android.application") version "9.4.0" apply false
    id("com.android.library") version "9.4.0" apply false
}
