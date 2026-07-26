package io.term.screenshot

import android.graphics.Bitmap
import android.graphics.Canvas
import android.view.View
import org.junit.Assert

fun View.captureBitmap(): Bitmap {
    Assert.assertTrue("View must have width > 0", width > 0)
    Assert.assertTrue("View must have height > 0", height > 0)
    val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(bitmap)
    draw(canvas)
    return bitmap
}
