package com.charmorph.storage

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.room.TypeConverters
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase
import com.charmorph.storage.dao.CharacterDao
import com.charmorph.storage.entity.CharacterGeometryEntity
import com.charmorph.storage.entity.CharacterMetadataEntity
import com.charmorph.storage.entity.Converters

@Database(
    entities = [CharacterMetadataEntity::class, CharacterGeometryEntity::class],
    version = 2,
    exportSchema = false
)
@TypeConverters(Converters::class)
abstract class AppDatabase : RoomDatabase() {
    abstract fun characterDao(): CharacterDao

    companion object {
        private const val DATABASE_NAME = "charmorph_db"

        val MIGRATION_1_2 = object : Migration(1, 2) {
            override fun migrate(db: SupportSQLiteDatabase) {
                db.execSQL(
                    """
                    CREATE TABLE IF NOT EXISTS `character_metadata` (
                        `id` TEXT NOT NULL,
                        `name` TEXT NOT NULL,
                        `thumbnailPath` TEXT,
                        `lastModified` INTEGER NOT NULL,
                        `morphWeights` TEXT NOT NULL,
                        PRIMARY KEY(`id`)
                    )
                    """.trimIndent()
                )

                db.execSQL(
                    """
                    CREATE TABLE IF NOT EXISTS `character_geometry` (
                        `characterId` TEXT NOT NULL,
                        `meshData` TEXT NOT NULL,
                        `skeletonData` TEXT NOT NULL,
                        PRIMARY KEY(`characterId`),
                        FOREIGN KEY(`characterId`) REFERENCES `character_metadata`(`id`) ON UPDATE NO ACTION ON DELETE CASCADE
                    )
                    """.trimIndent()
                )

                db.execSQL(
                    """
                    INSERT INTO `character_metadata` (`id`, `name`, `thumbnailPath`, `lastModified`, `morphWeights`)
                    SELECT `id`, `name`, `thumbnailPath`, `lastModified`, `morphWeights` FROM `characters`
                    """.trimIndent()
                )

                db.execSQL(
                    """
                    INSERT INTO `character_geometry` (`characterId`, `meshData`, `skeletonData`)
                    SELECT `id`, `meshData`, `skeletonData` FROM `characters`
                    """.trimIndent()
                )

                db.execSQL("DROP TABLE IF EXISTS `characters`")
            }
        }

        @Volatile
        private var INSTANCE: AppDatabase? = null

        fun getInstance(context: Context): AppDatabase {
            return INSTANCE ?: synchronized(this) {
                val instance = Room.databaseBuilder(
                    context.applicationContext,
                    AppDatabase::class.java,
                    DATABASE_NAME
                )
                    .addMigrations(MIGRATION_1_2)
                    .build()
                INSTANCE = instance
                instance
            }
        }
    }
}
