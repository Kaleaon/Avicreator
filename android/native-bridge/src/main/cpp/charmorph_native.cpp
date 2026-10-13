#include <jni.h>
#include <string>
#include <vector>
#include <map>
#include <mutex>
#include <android/log.h>

#define LOG_TAG "CharMorphNative"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, LOG_TAG, __VA_ARGS__)

// Simple struct to hold morph target data
struct MorphTarget {
    std::vector<int> indices; // Sparse indices
    std::vector<float> deltas; // Flattened deltas (dx, dy, dz)
};

// Struct to hold mesh state
struct MeshContext {
    std::vector<float> baseVertices; // Flattened (x, y, z)
    std::vector<float> currentVertices; // Output buffer
    std::map<int, MorphTarget> morphTargets; // ID -> Morph Data
    std::mutex mutex;

    void update(const std::map<int, float>& weights) {
        std::lock_guard<std::mutex> lock(mutex);

        // Reset to base
        currentVertices = baseVertices;

        // Apply morphs
        for (auto const& [id, weight] : weights) {
            if (weight == 0.0f) continue;
            if (morphTargets.find(id) == morphTargets.end()) continue;

            const auto& morph = morphTargets[id];
            for (size_t i = 0; i < morph.indices.size(); ++i) {
                int vIdx = morph.indices[i] * 3; // stride 3
                currentVertices[vIdx] += morph.deltas[i * 3] * weight;
                currentVertices[vIdx + 1] += morph.deltas[i * 3 + 1] * weight;
                currentVertices[vIdx + 2] += morph.deltas[i * 3 + 2] * weight;
            }
        }
    }
};

extern "C" JNIEXPORT jlong JNICALL
Java_com_charmorph_nativebridge_NativeLib_createMesh(
    JNIEnv* env, jobject, jfloatArray vertices) {

    MeshContext* ctx = new MeshContext();

    jsize len = env->GetArrayLength(vertices);
    jfloat* data = env->GetFloatArrayElements(vertices, 0);

    ctx->baseVertices.assign(data, data + len);
    ctx->currentVertices.assign(data, data + len); // Init output

    env->ReleaseFloatArrayElements(vertices, data, 0);

    return reinterpret_cast<jlong>(ctx);
}

extern "C" JNIEXPORT void JNICALL
Java_com_charmorph_nativebridge_NativeLib_destroyMesh(
    JNIEnv* env, jobject, jlong meshPtr) {
    if (meshPtr == 0) return;
    delete reinterpret_cast<MeshContext*>(meshPtr);
}

extern "C" JNIEXPORT void JNICALL
Java_com_charmorph_nativebridge_NativeLib_addMorphTarget(
    JNIEnv* env, jobject,
    jlong meshPtr, jint morphId, jintArray indices, jfloatArray deltas) {

    MeshContext* ctx = reinterpret_cast<MeshContext*>(meshPtr);
    if (!ctx) return;

    MorphTarget target;

    jsize idxLen = env->GetArrayLength(indices);
    jint* idxData = env->GetIntArrayElements(indices, 0);
    target.indices.assign(idxData, idxData + idxLen);
    env->ReleaseIntArrayElements(indices, idxData, 0);

    jsize deltaLen = env->GetArrayLength(deltas);
    jfloat* deltaData = env->GetFloatArrayElements(deltas, 0);
    target.deltas.assign(deltaData, deltaData + deltaLen);
    env->ReleaseFloatArrayElements(deltas, deltaData, 0);

    std::lock_guard<std::mutex> lock(ctx->mutex);
    ctx->morphTargets[morphId] = target;
}

extern "C" JNIEXPORT void JNICALL
Java_com_charmorph_nativebridge_NativeLib_updateMorphs(
    JNIEnv* env, jobject,
    jlong meshPtr, jintArray morphIds, jfloatArray morphWeights, jobject outputBuffer) {

    MeshContext* ctx = reinterpret_cast<MeshContext*>(meshPtr);
    if (!ctx) return;

    // Parse weights map
    std::map<int, float> weightsMap;
    jsize count = env->GetArrayLength(morphIds);
    jint* ids = env->GetIntArrayElements(morphIds, 0);
    jfloat* w = env->GetFloatArrayElements(morphWeights, 0);

    for(int i=0; i<count; ++i) {
        weightsMap[ids[i]] = w[i];
    }

    env->ReleaseIntArrayElements(morphIds, ids, 0);
    env->ReleaseFloatArrayElements(morphWeights, w, 0);

    // Compute
    ctx->update(weightsMap);

    // Copy to Direct ByteBuffer
    void* bufferAddr = env->GetDirectBufferAddress(outputBuffer);
    jlong bufferCap = env->GetDirectBufferCapacity(outputBuffer);

    if (bufferAddr && bufferCap >= ctx->currentVertices.size() * sizeof(float)) {
         memcpy(bufferAddr, ctx->currentVertices.data(), ctx->currentVertices.size() * sizeof(float));
    } else {
        LOGI("Output buffer too small!");
    }
}

#include <algorithm>
#include <cmath>

extern "C" JNIEXPORT jstring JNICALL
Java_com_charmorph_nativebridge_NativeLib_stringFromJNI(
        JNIEnv* env,
        jobject /* this */) {
    std::string hello = "Hello from C++";
    return env->NewStringUTF(hello.c_str());
}

extern "C" JNIEXPORT jfloatArray JNICALL
Java_com_charmorph_nativebridge_NativeLib_solveMorphWeights(
        JNIEnv* env,
        jobject /* this */,
        jfloatArray landmarks,
        jfloatArray baseVertices,
        jintArray morphIndices,
        jfloatArray morphDeltas) {

    jsize landmarksLen = env->GetArrayLength(landmarks);
    jsize baseLen = env->GetArrayLength(baseVertices);
    jsize indicesLen = env->GetArrayLength(morphIndices);
    jsize deltasLen = env->GetArrayLength(morphDeltas);

    if (landmarksLen == 0 || indicesLen < 2) {
        jfloatArray emptyResult = env->NewFloatArray(0);
        return emptyResult;
    }

    jfloat* lmData = env->GetFloatArrayElements(landmarks, 0);
    jfloat* baseData = env->GetFloatArrayElements(baseVertices, 0);
    jint* idxData = env->GetIntArrayElements(morphIndices, 0);
    jfloat* deltaData = env->GetFloatArrayElements(morphDeltas, 0);

    int numLandmarkCoords = idxData[0]; // e.g. 2 * N
    int numMorphs = idxData[1];          // M morph targets

    if (numMorphs <= 0 || numLandmarkCoords <= 0) {
        env->ReleaseFloatArrayElements(landmarks, lmData, 0);
        env->ReleaseFloatArrayElements(baseVertices, baseData, 0);
        env->ReleaseIntArrayElements(morphIndices, idxData, 0);
        env->ReleaseFloatArrayElements(morphDeltas, deltaData, 0);

        jfloatArray result = env->NewFloatArray(std::max(0, numMorphs));
        return result;
    }

    int K = std::min(static_cast<int>(landmarksLen), std::min(static_cast<int>(baseLen), numLandmarkCoords));
    int M = numMorphs;

    // Target displacement vector b (size K)
    std::vector<float> b(K);
    for (int k = 0; k < K; ++k) {
        b[k] = lmData[k] - baseData[k];
    }

    // Morph delta matrix A (K rows, M cols)
    std::vector<float> A(K * M, 0.0f);
    if (deltasLen >= M * K) {
        for (int m = 0; m < M; ++m) {
            for (int k = 0; k < K; ++k) {
                A[k * M + m] = deltaData[m * K + k];
            }
        }
    }

    // Normal equations: (A^T * A + lambda * I) * x = A^T * b
    std::vector<float> ATA(M * M, 0.0f);
    std::vector<float> ATb(M, 0.0f);

    for (int i = 0; i < M; ++i) {
        for (int j = 0; j < M; ++j) {
            float sum = 0.0f;
            for (int k = 0; k < K; ++k) {
                sum += A[k * M + i] * A[k * M + j];
            }
            ATA[i * M + j] = sum;
        }
        // Ridge regularization
        ATA[i * M + i] += 1e-4f;

        float sumB = 0.0f;
        for (int k = 0; k < K; ++k) {
            sumB += A[k * M + i] * b[k];
        }
        ATb[i] = sumB;
    }

    // Solve ATA * x = ATb using Gaussian elimination with partial pivoting
    std::vector<float> x(M, 0.0f);
    std::vector<float> mat = ATA;
    std::vector<float> rhs = ATb;

    for (int k = 0; k < M; ++k) {
        int pivot = k;
        float maxVal = std::abs(mat[k * M + k]);
        for (int i = k + 1; i < M; ++i) {
            float val = std::abs(mat[i * M + k]);
            if (val > maxVal) {
                maxVal = val;
                pivot = i;
            }
        }

        if (pivot != k) {
            for (int j = k; j < M; ++j) {
                std::swap(mat[k * M + j], mat[pivot * M + j]);
            }
            std::swap(rhs[k], rhs[pivot]);
        }

        float pivotVal = mat[k * M + k];
        if (std::abs(pivotVal) < 1e-7f) {
            pivotVal = 1e-7f;
            mat[k * M + k] = pivotVal;
        }

        for (int i = k + 1; i < M; ++i) {
            float factor = mat[i * M + k] / pivotVal;
            rhs[i] -= factor * rhs[k];
            for (int j = k; j < M; ++j) {
                mat[i * M + j] -= factor * mat[k * M + j];
            }
        }
    }

    // Back substitution
    for (int i = M - 1; i >= 0; --i) {
        float sum = rhs[i];
        for (int j = i + 1; j < M; ++j) {
            sum -= mat[i * M + j] * x[j];
        }
        float diag = mat[i * M + i];
        if (std::abs(diag) < 1e-7f) diag = 1e-7f;
        x[i] = sum / diag;
        // Clamp weights to [0.0, 1.0]
        if (x[i] < 0.0f) x[i] = 0.0f;
        if (x[i] > 1.0f) x[i] = 1.0f;
    }

    env->ReleaseFloatArrayElements(landmarks, lmData, 0);
    env->ReleaseFloatArrayElements(baseVertices, baseData, 0);
    env->ReleaseIntArrayElements(morphIndices, idxData, 0);
    env->ReleaseFloatArrayElements(morphDeltas, deltaData, 0);

    jfloatArray result = env->NewFloatArray(M);
    env->SetFloatArrayRegion(result, 0, M, x.data());
    return result;
}
