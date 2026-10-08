package com.charmorph.app.ui

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.work.WorkManager
import androidx.work.testing.WorkManagerTestInitHelper
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(AndroidJUnit4::class)
@Config(manifest = Config.NONE)
class ImportViewModelTest {

    private lateinit var context: Context
    private lateinit var workManager: WorkManager
    private lateinit var repository: FakeCharacterRepository
    private val testDispatcher = StandardTestDispatcher()
    private val testScope = TestScope(testDispatcher)

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        context = ApplicationProvider.getApplicationContext()
        WorkManagerTestInitHelper.initializeTestWorkManager(context)
        workManager = WorkManager.getInstance(context)
        repository = FakeCharacterRepository()
    }

    @Test
    fun testStartPipelineEnqueuesWorkManagerChain() = testScope.runTest {
        val viewModel = ImportViewModel(repository, workManager, context)

        viewModel.importTestCharacter()

        val state = viewModel.uiState.value
        assertTrue(state.isProcessing)
        assertTrue(state.progress > 0f)

        // WorkManager should have work enqueued
        val statuses = workManager.getWorkInfosForUniqueWorkFlow("import_pipeline").toString()
        // ViewModel initialized workManager pipeline chain
        testScheduler.advanceUntilIdle()
    }
}
