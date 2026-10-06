package com.charmorph.app.ui

import android.view.View
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.viewinterop.AndroidView
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config

@RunWith(AndroidJUnit4::class)
@Config(manifest = Config.NONE)
class EditorScreenTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @Test
    fun testViewportAndroidViewAccessibilitySemantics() {
        composeTestRule.setContent {
            AndroidView(
                modifier = Modifier
                    .fillMaxSize()
                    .semantics { contentDescription = "3D Character Viewport" },
                factory = { context -> View(context) }
            )
        }

        composeTestRule
            .onNodeWithContentDescription("3D Character Viewport")
            .assertIsDisplayed()
    }

    @Test
    fun testMorphSliderAccessibilitySemantics() {
        val morphState = MorphState(
            name = "body_fat",
            displayName = "Body Fat",
            category = "Body",
            value = 0.5f,
            min = 0.0f,
            max = 1.0f
        )

        composeTestRule.setContent {
            MorphSlider(
                morph = morphState,
                onValueChange = {}
            )
        }

        composeTestRule
            .onNodeWithContentDescription("Body Fat")
            .assertIsDisplayed()
    }

    @Test
    fun testBoneControlAccessibilitySemantics() {
        val boneState = BoneState(
            id = 1,
            name = "Head",
            pitch = 10f,
            yaw = 20f,
            roll = 30f
        )

        composeTestRule.setContent {
            BoneControl(
                bone = boneState,
                onUpdate = { _, _, _ -> }
            )
        }

        composeTestRule
            .onNodeWithContentDescription("Head X axis pitch")
            .assertIsDisplayed()

        composeTestRule
            .onNodeWithContentDescription("Head Y axis yaw")
            .assertIsDisplayed()

        composeTestRule
            .onNodeWithContentDescription("Head Z axis roll")
            .assertIsDisplayed()
    }
}
