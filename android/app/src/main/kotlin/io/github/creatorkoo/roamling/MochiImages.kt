// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only
package io.github.creatorkoo.roamling

import android.graphics.Bitmap
import java.nio.ByteBuffer
import uniffi.roamling_android.MascotAtlas
import uniffi.roamling_android.loadMochi
import uniffi.roamling_core.FfiPetImage

/** Owns one native decode and two Android uploads for this preview session. */
internal class MochiImages(
    val atlas: MascotAtlas,
    val standard: Bitmap,
    val extension: Bitmap,
) : AutoCloseable {
    override fun close() {
        standard.recycle()
        extension.recycle()
        atlas.destroy()
    }

    companion object {
        fun bitmap(image: FfiPetImage): Bitmap {
            val width = image.width.toInt()
            val height = image.height.toInt()
            require(width > 0 && height > 0)
            require(width.toLong() * height * 4 == image.pixels.size.toLong())
            // PetImage is tightly packed premultiplied RGBA8. Raw buffer copy
            // preserves its alpha; setPixels would premultiply a second time.
            return Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888).apply {
                copyPixelsFromBuffer(ByteBuffer.wrap(image.pixels))
                density = Bitmap.DENSITY_NONE
            }
        }

        fun load(): MochiImages {
            val atlas = checkNotNull(loadMochi()) { "Built-in Mochi decode failed" }
            var standard: Bitmap? = null
            try {
                standard = bitmap(checkNotNull(atlas.image(false)))
                return MochiImages(atlas, standard, bitmap(checkNotNull(atlas.image(true))))
            } catch (failure: Throwable) {
                standard?.recycle()
                atlas.destroy()
                throw failure
            }
        }
    }
}
