package com.charmorph.storage

import android.content.Context
import androidx.room.Room
import androidx.sqlite.db.SupportSQLiteDatabase
import androidx.sqlite.db.SupportSQLiteOpenHelper
import androidx.sqlite.db.framework.FrameworkSQLiteOpenHelperFactory
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.Skeleton
import com.charmorph.storage.entity.CharacterGeometryEntity
import com.charmorph.storage.entity.CharacterMetadataEntity
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config

@RunWith(AndroidJUnit4::class)
@Config(manifest = Config.NONE)
class CharacterDatabaseTest {

    private lateinit var database: AppDatabase

    @Before
    fun createDb() {
        database = Room.inMemoryDatabaseBuilder(
            ApplicationProvider.getApplicationContext(),
            AppDatabase::class.java
        ).allowMainThreadQueries().build()
    }

    @After
    fun closeDb() {
        database.close()
    }

    @Test
    fun testMetadataAndGeometrySeparation() = runBlocking {
        val dao = database.characterDao()

        val metadata = CharacterMetadataEntity(
            id = "char-1",
            name = "Hero",
            thumbnailPath = "/thumb/hero.png",
            lastModified = 1000L,
            morphWeights = mapOf("fat" to 0.2f)
        )

        val geometry = CharacterGeometryEntity(
            characterId = "char-1",
            meshData = Mesh("m1", "HeroMesh", emptyList(), emptyList(), emptyList(), emptyList()),
            skeletonData = Skeleton(emptyList())
        )

        dao.insertCharacter(metadata, geometry)

        // Verify metadata query fetches metadata only
        val metadataList = dao.getAllMetadata().first()
        assertEquals(1, metadataList.size)
        assertEquals("Hero", metadataList[0].name)
        assertEquals(mapOf("fat" to 0.2f), metadataList[0].morphWeights)

        // Verify targeted morph weights update
        dao.updateMorphWeights("char-1", mapOf("fat" to 0.8f, "muscle" to 0.5f), 2000L)

        val updatedMetadata = dao.getMetadataById("char-1")
        assertNotNull(updatedMetadata)
        assertEquals(mapOf("fat" to 0.8f, "muscle" to 0.5f), updatedMetadata!!.morphWeights)
        assertEquals(2000L, updatedMetadata.lastModified)

        // Geometry remains intact
        val fetchedGeometry = dao.getGeometryById("char-1")
        assertNotNull(fetchedGeometry)
        assertEquals("HeroMesh", fetchedGeometry!!.meshData.name)
    }

    @Test
    fun testMigrationFromV1ToV2() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val dbFile = context.getDatabasePath("test_migration.db")
        dbFile.delete()

        // 1. Create v1 schema in SQLite database
        val sqliteDb = FrameworkSQLiteOpenHelperFactory().create(
            SupportSQLiteOpenHelper.Configuration.builder(context)
                .name("test_migration.db")
                .callback(object : SupportSQLiteOpenHelper.Callback(1) {
                    override fun onCreate(db: SupportSQLiteDatabase) {
                        db.execSQL(
                            """
                            CREATE TABLE IF NOT EXISTS `characters` (
                                `id` TEXT NOT NULL,
                                `name` TEXT NOT NULL,
                                `thumbnailPath` TEXT,
                                `lastModified` INTEGER NOT NULL,
                                `meshData` TEXT NOT NULL,
                                `skeletonData` TEXT NOT NULL,
                                `morphWeights` TEXT NOT NULL,
                                PRIMARY KEY(`id`)
                            )
                            """.trimIndent()
                        )
                    }

                    override fun onUpgrade(db: SupportSQLiteDatabase, oldVersion: Int, newVersion: Int) {}
                })
                .build()
        ).writableDatabase

        // Insert legacy v1 record
        sqliteDb.execSQL(
            """
            INSERT INTO `characters` VALUES (
                'v1-char', 'V1 Hero', '/thumb/v1.png', 500,
                '{"id":"m1","name":"Mesh1","vertices":[],"normals":[],"uvs":[],"indices":[]}',
                '{"bones":[]}',
                '{"body_fat":0.3}'
            )
            """.trimIndent()
        )

        // 2. Execute MIGRATION_1_2
        AppDatabase.MIGRATION_1_2.migrate(sqliteDb)

        // 3. Verify data in new tables
        val metadataCursor = sqliteDb.query("SELECT * FROM character_metadata WHERE id = 'v1-char'")
        assert(metadataCursor.moveToFirst())
        val nameIndex = metadataCursor.getColumnIndex("name")
        assertEquals("V1 Hero", metadataCursor.getString(nameIndex))
        metadataCursor.close()

        val geometryCursor = sqliteDb.query("SELECT * FROM character_geometry WHERE characterId = 'v1-char'")
        assert(geometryCursor.moveToFirst())
        val meshIndex = geometryCursor.getColumnIndex("meshData")
        assert(geometryCursor.getString(meshIndex).contains("Mesh1"))
        geometryCursor.close()

        sqliteDb.close()
    }
}
