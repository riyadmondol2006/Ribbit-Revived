package com.riyadm.ribbit.revived

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Rect
import android.graphics.Typeface
import java.nio.ByteBuffer

/**
 * Rasterises printable ASCII with the system's heaviest sans-serif face into an ALPHA_8 atlas.
 * The Rust side turns the coverage into a signed distance field so text scales and outlines cleanly.
 */
class FontAtlas private constructor(
    val width: Int,
    val height: Int,
    val pixels: ByteArray,
    val glyphs: FloatArray,
    val fontPx: Float,
) {
    companion object {
        private const val FIRST = 32
        private const val LAST = 126
        private const val FONT_PX = 64f
        private const val PAD = 10

        fun build(): FontAtlas {
            val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                val black = Typeface.create("sans-serif-black", Typeface.NORMAL)
                typeface = if (android.os.Build.VERSION.SDK_INT >= 28) Typeface.create(black, 900, false) else black
                textSize = FONT_PX
                color = 0xFFFFFFFF.toInt()
            }
            val atlasW = 1024
            val count = LAST - FIRST + 1
            val glyphs = FloatArray(count * 7)
            val bounds = Rect()
            val fm = paint.fontMetricsInt
            val cellH = (fm.descent - fm.ascent) + PAD * 2

            // First pass: lay out cells.
            var x = 0
            var y = 0
            val placements = IntArray(count * 2)
            val widths = IntArray(count)
            for (i in 0 until count) {
                val s = (FIRST + i).toChar().toString()
                paint.getTextBounds(s, 0, 1, bounds)
                val w = maxOf(bounds.width(), 1) + PAD * 2
                widths[i] = w
                if (x + w > atlasW) {
                    x = 0
                    y += cellH
                }
                placements[i * 2] = x
                placements[i * 2 + 1] = y
                x += w
            }
            var atlasH = 64
            while (atlasH < y + cellH) atlasH *= 2

            val bitmap = Bitmap.createBitmap(atlasW, atlasH, Bitmap.Config.ALPHA_8)
            val canvas = Canvas(bitmap)
            for (i in 0 until count) {
                val s = (FIRST + i).toChar().toString()
                paint.getTextBounds(s, 0, 1, bounds)
                val cx = placements[i * 2]
                val cy = placements[i * 2 + 1]
                val baseline = cy + PAD - fm.ascent
                val penX = cx + PAD - bounds.left
                canvas.drawText(s, penX.toFloat(), baseline.toFloat(), paint)
                val o = i * 7
                glyphs[o] = cx.toFloat()
                glyphs[o + 1] = (cy + PAD + (bounds.top - fm.ascent) - PAD).toFloat()
                glyphs[o + 2] = widths[i].toFloat()
                glyphs[o + 3] = (maxOf(bounds.height(), 1) + PAD * 2).toFloat()
                glyphs[o + 4] = (bounds.left - PAD).toFloat()
                glyphs[o + 5] = (bounds.top - PAD).toFloat()
                glyphs[o + 6] = paint.measureText(s)
            }
            val buf = ByteBuffer.allocate(atlasW * atlasH)
            bitmap.copyPixelsToBuffer(buf)
            bitmap.recycle()
            return FontAtlas(atlasW, atlasH, buf.array(), glyphs, FONT_PX)
        }
    }
}
