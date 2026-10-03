package com.charmorph.renderer

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.util.AttributeSet
import android.view.View
import com.charmorph.core.model.Skeleton

class SkeletonVisualizer @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : View(context, attrs, defStyleAttr) {

    private var skeleton: Skeleton? = null

    private val bonePaint = Paint().apply {
        color = Color.GREEN
        strokeWidth = 5f
        isAntiAlias = true
    }

    private val jointPaint = Paint().apply {
        color = Color.RED
        style = Paint.Style.FILL
        isAntiAlias = true
    }

    fun setSkeleton(skeleton: Skeleton?) {
        this.skeleton = skeleton
        invalidate()
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val currentSkeleton = skeleton ?: return

        // Simplified 2D projection for debug
        // In a real 3D app, we would need to project 3D points to 2D screen coordinates
        // using the Camera's View-Projection matrix.
        // Since we don't have easy access to that in this View overlay without communicating
        // with Filament, we will just draw a placeholder text or basic structure if available.

        // For now, since we can't accurately overlay 3D without shared matrices,
        // we'll just indicate skeleton presence.

        canvas.drawText("Skeleton Visualizer: ${currentSkeleton.bones.size} bones", 50f, 50f, bonePaint.apply { textSize = 40f })
    }

    // To implement real 3D lines, we should actually use Filament's Line rendering
    // inside FilamentController, not a separate Android View.
}
