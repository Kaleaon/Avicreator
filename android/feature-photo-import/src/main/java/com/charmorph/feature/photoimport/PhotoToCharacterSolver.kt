package com.charmorph.feature.photoimport

import android.graphics.Bitmap
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.Resource
import com.charmorph.nativebridge.NativeLib
import com.google.android.gms.tasks.Tasks
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.face.FaceDetection
import com.google.mlkit.vision.face.FaceDetectorOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

class PhotoToCharacterSolver(
    private val landmarkDetector: ((Bitmap) -> List<Pair<Float, Float>>)? = null,
    private val nativeLib: NativeLib = NativeLib()
) {

    suspend fun solveFromImage(bitmap: Bitmap, baseMesh: Mesh): Resource<Map<String, Float>> = withContext(Dispatchers.Default) {
        try {
            // 1. Detect Landmarks (using ML Kit or custom detector)
            val landmarks = detectLandmarks(bitmap)

            if (landmarks.isEmpty()) {
                return@withContext Resource.Error(IllegalArgumentException("No face detected in photo"))
            }

            // 2. Solve for morph weights using least-squares optimization
            val weights = solveWeights(landmarks, baseMesh)

            Resource.Success(weights)
        } catch (e: Exception) {
            Resource.Error(e)
        }
    }

    private fun detectLandmarks(bitmap: Bitmap): List<Pair<Float, Float>> {
        if (landmarkDetector != null) {
            return landmarkDetector.invoke(bitmap)
        }
        return try {
            val options = FaceDetectorOptions.Builder()
                .setPerformanceMode(FaceDetectorOptions.PERFORMANCE_MODE_ACCURATE)
                .setLandmarkMode(FaceDetectorOptions.LANDMARK_MODE_ALL)
                .build()
            val detector = FaceDetection.getClient(options)
            val image = InputImage.fromBitmap(bitmap, 0)
            val task = detector.process(image)
            val faces = Tasks.await(task)
            if (faces.isNullOrEmpty()) {
                emptyList()
            } else {
                val face = faces[0]
                val result = mutableListOf<Pair<Float, Float>>()
                for (lm in face.allLandmarks) {
                    val pt = lm.position
                    result.add(Pair(pt.x, pt.y))
                }
                result
            }
        } catch (e: Exception) {
            emptyList()
        }
    }

    private fun solveWeights(landmarks: List<Pair<Float, Float>>, mesh: Mesh): Map<String, Float> {
        val morphNames = if (mesh.morphTargets.isNotEmpty()) {
            mesh.morphTargets.map { it.name }
        } else {
            listOf("cheek_fullness", "jaw_width", "nose_length", "chin_length", "eye_size")
        }

        val N = landmarks.size
        val M = morphNames.size

        if (N == 0 || M == 0) {
            return morphNames.associateWith { 0.0f }
        }

        // Normalize detected landmarks
        var minX = Float.MAX_VALUE
        var maxX = -Float.MAX_VALUE
        var minY = Float.MAX_VALUE
        var maxY = -Float.MAX_VALUE

        for (lm in landmarks) {
            if (lm.first < minX) minX = lm.first
            if (lm.first > maxX) maxX = lm.first
            if (lm.second < minY) minY = lm.second
            if (lm.second > maxY) maxY = lm.second
        }

        val rangeX = if (maxX > minX) maxX - minX else 1.0f
        val rangeY = if (maxY > minY) maxY - minY else 1.0f

        val landmarksArray = FloatArray(2 * N)
        val baseArray = FloatArray(2 * N)

        for (i in 0 until N) {
            val normX = (landmarks[i].first - minX) / rangeX
            val normY = (landmarks[i].second - minY) / rangeY

            landmarksArray[2 * i] = normX
            landmarksArray[2 * i + 1] = normY

            // Reference base landmark positions around center (0.5, 0.5)
            val angle = (2.0 * Math.PI * i) / N
            val refX = 0.5f + 0.3f * Math.cos(angle).toFloat()
            val refY = 0.5f + 0.3f * Math.sin(angle).toFloat()

            baseArray[2 * i] = refX
            baseArray[2 * i + 1] = refY
        }

        // Build morph deltas matrix (M morphs x 2N coordinates)
        val deltasArray = FloatArray(M * 2 * N)
        for (m in 0 until M) {
            for (i in 0 until N) {
                val idx = m * (2 * N) + 2 * i
                // Pattern for each morph target
                when (m % 5) {
                    0 -> { // cheek fullness (horizontal expansion)
                        deltasArray[idx] = if (i % 2 == 0) 0.15f else -0.15f
                        deltasArray[idx + 1] = 0.0f
                    }
                    1 -> { // jaw width
                        deltasArray[idx] = if (i < N / 2) 0.2f else -0.2f
                        deltasArray[idx + 1] = 0.05f
                    }
                    2 -> { // nose length
                        deltasArray[idx] = 0.0f
                        deltasArray[idx + 1] = 0.18f
                    }
                    3 -> { // chin length
                        deltasArray[idx] = 0.0f
                        deltasArray[idx + 1] = 0.25f
                    }
                    4 -> { // eye size
                        deltasArray[idx] = 0.1f
                        deltasArray[idx + 1] = 0.1f
                    }
                }
            }
        }

        val morphIndices = intArrayOf(2 * N, M)

        val weightsArray = try {
            nativeLib.solveMorphWeights(landmarksArray, baseArray, morphIndices, deltasArray)
        } catch (e: Throwable) {
            solveMorphWeightsKotlin(landmarksArray, baseArray, morphIndices, deltasArray)
        }

        val result = mutableMapOf<String, Float>()
        for (i in 0 until M) {
            result[morphNames[i]] = weightsArray.getOrElse(i) { 0.0f }
        }
        return result
    }

    private fun solveMorphWeightsKotlin(
        landmarks: FloatArray,
        baseVertices: FloatArray,
        morphIndices: IntArray,
        morphDeltas: FloatArray
    ): FloatArray {
        val K = minOf(landmarks.size, baseVertices.size, morphIndices.getOrElse(0) { 0 })
        val M = morphIndices.getOrElse(1) { 0 }
        if (K <= 0 || M <= 0) return FloatArray(maxOf(0, M))

        val b = FloatArray(K) { i -> landmarks[i] - baseVertices[i] }
        val A = FloatArray(K * M) { 0f }
        if (morphDeltas.size >= M * K) {
            for (m in 0 until M) {
                for (k in 0 until K) {
                    A[k * M + m] = morphDeltas[m * K + k]
                }
            }
        }

        val ATA = FloatArray(M * M)
        val ATb = FloatArray(M)

        for (i in 0 until M) {
            for (j in 0 until M) {
                var sum = 0f
                for (k in 0 until K) {
                    sum += A[k * M + i] * A[k * M + j]
                }
                ATA[i * M + j] = sum
            }
            ATA[i * M + i] += 1e-4f

            var sumB = 0f
            for (k in 0 until K) {
                sumB += A[k * M + i] * b[k]
            }
            ATb[i] = sumB
        }

        val x = FloatArray(M)
        val mat = ATA.copyOf()
        val rhs = ATb.copyOf()

        for (k in 0 until M) {
            var pivot = k
            var maxVal = Math.abs(mat[k * M + k])
            for (i in k + 1 until M) {
                val v = Math.abs(mat[i * M + k])
                if (v > maxVal) {
                    maxVal = v
                    pivot = i
                }
            }
            if (pivot != k) {
                for (j in k until M) {
                    val temp = mat[k * M + j]
                    mat[k * M + j] = mat[pivot * M + j]
                    mat[pivot * M + j] = temp
                }
                val tempR = rhs[k]
                rhs[k] = rhs[pivot]
                rhs[pivot] = tempR
            }

            var pivotVal = mat[k * M + k]
            if (Math.abs(pivotVal) < 1e-7f) {
                pivotVal = 1e-7f;
                mat[k * M + k] = pivotVal
            }

            for (i in k + 1 until M) {
                val factor = mat[i * M + k] / pivotVal
                rhs[i] -= factor * rhs[k]
                for (j in k until M) {
                    mat[i * M + j] -= factor * mat[k * M + j]
                }
            }
        }

        for (i in M - 1 downTo 0) {
            var sum = rhs[i]
            for (j in i + 1 until M) {
                sum -= mat[i * M + j] * x[j]
            }
            var diag = mat[i * M + i]
            if (Math.abs(diag) < 1e-7f) diag = 1e-7f
            var valX = sum / diag
            if (valX < 0f) valX = 0f
            if (valX > 1f) valX = 1f
            x[i] = valX
        }

        return x
    }
}
