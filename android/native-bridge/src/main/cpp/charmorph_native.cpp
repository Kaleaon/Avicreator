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

    // Stub implementation as before
    int morphCount = 10;
    std::vector<float> resultWeights(morphCount, 0.5f);
    jfloatArray result = env->NewFloatArray(morphCount);
    env->SetFloatArrayRegion(result, 0, morphCount, resultWeights.data());
    return result;
}
