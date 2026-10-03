package com.charmorph.core.math

import android.opengl.Matrix
import com.charmorph.core.model.Vector3
import com.charmorph.core.model.Vector4
import kotlin.math.cos
import kotlin.math.sin
import kotlin.math.sqrt

object MathUtils {

    fun setIdentityM(sm: FloatArray, smOffset: Int) {
        Matrix.setIdentityM(sm, smOffset)
    }

    fun multiplyMM(result: FloatArray, lhs: FloatArray, rhs: FloatArray) {
        Matrix.multiplyMM(result, 0, lhs, 0, rhs, 0)
    }

    fun composeMatrix(pos: Vector3, rot: Vector4, scale: Vector3): FloatArray {
        val m = FloatArray(16)
        setIdentityM(m, 0)

        // 1. Translation
        Matrix.translateM(m, 0, pos.x, pos.y, pos.z)

        // 2. Rotation (Quaternion to Matrix)
        // Quaternion w, x, y, z
        val rm = FloatArray(16)
        setIdentityM(rm, 0)

        val x = rot.x
        val y = rot.y
        val z = rot.z
        val w = rot.w

        val x2 = x * x; val y2 = y * y; val z2 = z * z
        val xy = x * y; val xz = x * z; val yz = y * z
        val wx = w * x; val wy = w * y; val wz = w * z

        rm[0] = 1f - 2f * (y2 + z2)
        rm[1] = 2f * (xy + wz)
        rm[2] = 2f * (xz - wy)
        rm[3] = 0f

        rm[4] = 2f * (xy - wz)
        rm[5] = 1f - 2f * (x2 + z2)
        rm[6] = 2f * (yz + wx)
        rm[7] = 0f

        rm[8] = 2f * (xz + wy)
        rm[9] = 2f * (yz - wx)
        rm[10] = 1f - 2f * (x2 + y2)
        rm[11] = 0f

        // Combine Translation * Rotation
        val tmp = FloatArray(16)
        Matrix.multiplyMM(tmp, 0, m, 0, rm, 0)

        // 3. Scale
        Matrix.scaleM(tmp, 0, scale.x, scale.y, scale.z)

        return tmp
    }

    fun eulerToQuaternion(pitch: Float, yaw: Float, roll: Float): Vector4 {
        // Assuming XYZ order, angles in degrees
        val pRad = Math.toRadians(pitch.toDouble()) / 2.0
        val yRad = Math.toRadians(yaw.toDouble()) / 2.0
        val rRad = Math.toRadians(roll.toDouble()) / 2.0

        val sp = sin(pRad); val cp = cos(pRad)
        val sy = sin(yRad); val cy = cos(yRad)
        val sr = sin(rRad); val cr = cos(rRad)
        val x = sr * cp * cy - cr * sp * sy
        val y = cr * sp * cy + sr * cp * sy
        val z = cr * cp * sy - sr * sp * cy
        val w = cr * cp * cy + sr * sp * sy

        return Vector4(x.toFloat(), y.toFloat(), z.toFloat(), w.toFloat())
    }

    // Inverse of a matrix
    fun invertM(m: FloatArray): FloatArray {
        val inv = FloatArray(16)
        Matrix.invertM(inv, 0, m, 0)
        return inv
    }
}
