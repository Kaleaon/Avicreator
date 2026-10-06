package com.charmorph.renderer

import com.google.android.filament.Engine
import com.google.android.filament.Material
import com.google.android.filament.filamat.MaterialBuilder

object MaterialFactory {

    fun createPbrMaterial(engine: Engine): Material {
        val builder = MaterialBuilder()
            .name("DefaultPBR")
            .materialDomain(MaterialBuilder.MaterialDomain.SURFACE)
            .shading(MaterialBuilder.Shading.LIT)
            .blending(MaterialBuilder.BlendingMode.OPAQUE)

            // Attributes needed for PBR
            .require(MaterialBuilder.VertexAttribute.UV0)
            .require(MaterialBuilder.VertexAttribute.COLOR)

            // Parameters
            .samplerParameter(
                MaterialBuilder.SamplerType.SAMPLER_2D,
                MaterialBuilder.SamplerFormat.FLOAT,
                MaterialBuilder.ParameterPrecision.DEFAULT,
                "baseColorMap"
            )
            .samplerParameter(
                MaterialBuilder.SamplerType.SAMPLER_2D,
                MaterialBuilder.SamplerFormat.FLOAT,
                MaterialBuilder.ParameterPrecision.DEFAULT,
                "normalMap"
            )
            .samplerParameter(
                MaterialBuilder.SamplerType.SAMPLER_2D,
                MaterialBuilder.SamplerFormat.FLOAT,
                MaterialBuilder.ParameterPrecision.DEFAULT,
                "roughnessMap"
            )
            .uniformParameter(MaterialBuilder.UniformType.FLOAT4, "baseColorFactor")
            .uniformParameter(MaterialBuilder.UniformType.FLOAT, "roughnessFactor")
            .uniformParameter(MaterialBuilder.UniformType.FLOAT, "metallicFactor")

        val pkg = builder.build(engine)
        val buffer = pkg.buffer
        val material = Material.Builder().payload(buffer, buffer.limit()).build(engine)
        return material
    }
}
