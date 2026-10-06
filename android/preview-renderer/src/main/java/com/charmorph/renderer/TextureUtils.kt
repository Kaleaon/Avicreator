package com.charmorph.renderer

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import com.google.android.filament.Engine
import com.google.android.filament.Texture
import com.google.android.filament.android.TextureHelper
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.nio.ByteBuffer

object TextureUtils {

    suspend fun loadTextureFromUri(context: Context, engine: Engine, uri: Uri, isSrgb: Boolean = true): Texture? = withContext(Dispatchers.IO) {
        try {
            val inputStream = context.contentResolver.openInputStream(uri)
            val bitmap = BitmapFactory.decodeStream(inputStream)
            inputStream?.close()

            if (bitmap != null) {
                loadTextureFromBitmap(engine, bitmap, isSrgb)
            } else {
                null
            }
        } catch (e: Exception) {
            e.printStackTrace()
            null
        }
    }

    fun loadTextureFromBitmap(engine: Engine, bitmap: Bitmap, isSrgb: Boolean = true): Texture {
        val format = if (isSrgb) Texture.InternalFormat.SRGB8_A8 else Texture.InternalFormat.RGBA8

        val texture = Texture.Builder()
            .width(bitmap.width)
            .height(bitmap.height)
            .levels(1)
            .format(format)
            .sampler(Texture.Sampler.SAMPLER_2D)
            .build(engine)

        TextureHelper.setBitmap(engine, texture, 0, bitmap)
        return texture
    }
}
