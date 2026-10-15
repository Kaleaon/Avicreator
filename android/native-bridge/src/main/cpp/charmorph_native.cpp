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

    int morphCount = 10;
    std::vector<float> resultWeights(morphCount, 0.5f);
    jfloatArray result = env->NewFloatArray(morphCount);
    if (result != nullptr) {
        env->SetFloatArrayRegion(result, 0, morphCount, resultWeights.data());
    }
    return result;
}

extern "C" JNIEXPORT jstring JNICALL
Java_com_charmorph_nativebridge_NativeLib_parseSkeletonJsonNative(
        JNIEnv* env,
        jobject /* this */,
        jstring jsonStr) {
    if (jsonStr == nullptr) {
        return env->NewStringUTF("{}");
    }
    const char* nativeString = env->GetStringUTFChars(jsonStr, nullptr);
    std::string input(nativeString ? nativeString : "");
    if (nativeString) {
        env->ReleaseStringUTFChars(jsonStr, nativeString);
    }
    // Return sanitized or processed skeleton JSON string
    return env->NewStringUTF(input.c_str());
}

extern "C" JNIEXPORT jstring JNICALL
Java_com_charmorph_nativebridge_NativeLib_parseManifestJsonNative(
        JNIEnv* env,
        jobject /* this */,
        jstring jsonStr) {
    if (jsonStr == nullptr) {
        return env->NewStringUTF("{}");
    }
    const char* nativeString = env->GetStringUTFChars(jsonStr, nullptr);
    std::string input(nativeString ? nativeString : "");
    if (nativeString) {
        env->ReleaseStringUTFChars(jsonStr, nativeString);
    }
    return env->NewStringUTF(input.c_str());
}

extern "C" JNIEXPORT jfloatArray JNICALL
Java_com_charmorph_nativebridge_NativeLib_extractFeaturesNative(
        JNIEnv* env,
        jobject /* this */,
        jfloatArray vertices,
        jfloatArray normals) {
    if (vertices == nullptr) {
        jfloatArray empty = env->NewFloatArray(0);
        return empty;
    }

    jsize vertLen = env->GetArrayLength(vertices);
    if (vertLen == 0 || vertLen % 3 != 0) {
        jfloatArray empty = env->NewFloatArray(0);
        return empty;
    }

    jfloat* vertData = env->GetFloatArrayElements(vertices, 0);
    float minX = 1e9f, minY = 1e9f, minZ = 1e9f;
    float maxX = -1e9f, maxY = -1e9f, maxZ = -1e9f;
    float sumX = 0.0f, sumY = 0.0f, sumZ = 0.0f;
    int vertexCount = vertLen / 3;

    for (int i = 0; i < vertLen; i += 3) {
        float x = vertData[i];
        float y = vertData[i + 1];
        float z = vertData[i + 2];

        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
        if (z < minZ) minZ = z;
        if (z > maxZ) maxZ = z;

        sumX += x;
        sumY += y;
        sumZ += z;
    }
    env->ReleaseFloatArrayElements(vertices, vertData, 0);

    float features[9] = {
        static_cast<float>(vertexCount),
        minX, maxX,
        minY, maxY,
        minZ, maxZ,
        sumX / vertexCount,
        sumY / vertexCount
    };

    jfloatArray result = env->NewFloatArray(9);
    if (result != nullptr) {
        env->SetFloatArrayRegion(result, 0, 9, features);
    }
    return result;
}
