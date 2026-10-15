//! SQLite storage. Every query in the application lives here so the rest of
//! the backend works with typed models instead of SQL.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::cache::fingerprint::{Candidate, Fingerprint};
use crate::error::{AppError, AppResult};
use crate::models::{
    Collection, Credit, HistoryEntry, RotationMode, RotationSettings, Wallpaper, WallpaperInput, WallpaperStatus,
};
use crate::theme::palette::Palette;

/// Ordered schema migrations. `PRAGMA user_version` records how many have
/// been applied; append new migrations, never edit released ones.
const MIGRATIONS: &[&str] = &[
    r#"
CREATE TABLE wallpapers (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    provider      TEXT    NOT NULL,
    provider_id   TEXT    NOT NULL,
    title         TEXT    NOT NULL DEFAULT '',
    image_url     TEXT    NOT NULL,
    thumbnail_url TEXT    NOT NULL,
    local_path    TEXT,
    width         INTEGER NOT NULL DEFAULT 0,
    height        INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT    NOT NULL,
    UNIQUE (provider, provider_id)
);

CREATE TABLE collections (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL UNIQUE COLLATE NOCASE,
    created_at TEXT NOT NULL
);

CREATE TABLE collection_wallpapers (
    collection_id INTEGER NOT NULL REFERENCES collections (id) ON DELETE CASCADE,
    wallpaper_id  INTEGER NOT NULL REFERENCES wallpapers (id) ON DELETE CASCADE,
    PRIMARY KEY (collection_id, wallpaper_id)
);
CREATE INDEX idx_collection_wallpapers_wallpaper ON collection_wallpapers (wallpaper_id);

CREATE TABLE favorites (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    wallpaper_id INTEGER NOT NULL UNIQUE REFERENCES wallpapers (id) ON DELETE CASCADE,
    created_at   TEXT    NOT NULL
);

CREATE TABLE wallpaper_history (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    wallpaper_id INTEGER NOT NULL REFERENCES wallpapers (id) ON DELETE CASCADE,
    applied_at   TEXT    NOT NULL
);
CREATE INDEX idx_wallpaper_history_applied_at ON wallpaper_history (applied_at);
CREATE INDEX idx_wallpaper_history_wallpaper ON wallpaper_history (wallpaper_id);

CREATE TABLE rotation_settings (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    collection_id    INTEGER NOT NULL UNIQUE REFERENCES collections (id) ON DELETE CASCADE,
    interval_minutes INTEGER NOT NULL CHECK (interval_minutes > 0),
    rotation_mode    TEXT    NOT NULL CHECK (rotation_mode IN ('sequential', 'random')),
    enabled          INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1))
);
-- Only one collection can rotate at a time.
CREATE UNIQUE INDEX idx_rotation_settings_single_enabled ON rotation_settings (enabled) WHERE enabled = 1;
"#,
    r#"
-- Intervals are kept in seconds, so a rotation can run faster than once a minute.
ALTER TABLE rotation_settings RENAME COLUMN interval_minutes TO interval_seconds;
UPDATE rotation_settings SET interval_seconds = interval_seconds * 60;
"#,
    r#"
-- Attribution: who made each wallpaper, its page on the provider's website
-- and its license. Search results fill these in.
ALTER TABLE wallpapers ADD COLUMN author TEXT;
ALTER TABLE wallpapers ADD COLUMN author_url TEXT;
ALTER TABLE wallpapers ADD COLUMN source_url TEXT;
ALTER TABLE wallpapers ADD COLUMN license TEXT;
ALTER TABLE wallpapers ADD COLUMN license_url TEXT;
-- Wallpapers saved earlier get their page address from their id, and the
-- photographer from titles written as "Photo by <name> on <provider>".
UPDATE wallpapers SET source_url = CASE provider
    WHEN 'wallhaven' THEN 'https://wallhaven.cc/w/' || provider_id
    WHEN 'unsplash' THEN 'https://unsplash.com/photos/' || provider_id || '?utm_source=kankulirchi&utm_medium=referral'
    WHEN 'pexels' THEN 'https://www.pexels.com/photo/' || provider_id || '/'
    WHEN 'wikimedia' THEN 'https://commons.wikimedia.org/?curid=' || provider_id
END;
UPDATE wallpapers SET author = substr(title, 10, length(title) - 21), title = 'Unsplash photo ' || provider_id
    WHERE provider = 'unsplash' AND title LIKE 'Photo by % on Unsplash';
UPDATE wallpapers SET author = substr(title, 10, length(title) - 19), title = 'Pexels photo ' || provider_id
    WHERE provider = 'pexels' AND title LIKE 'Photo by % on Pexels';
"#,
    r#"
-- The wallpaper's colors, worked out once (JSON, see theme::palette::Palette).
ALTER TABLE wallpapers ADD COLUMN palette TEXT;
"#,
    r#"
-- A fingerprint of the picture, to find copies of it (see cache::fingerprint).
ALTER TABLE wallpapers ADD COLUMN fingerprint BLOB;
"#,
];

const WALLPAPER_COLUMNS: &str = "w.id, w.provider, w.provider_id, w.title, w.image_url, w.thumbnail_url, \
     w.local_path, w.width, w.height, w.created_at, \
     EXISTS (SELECT 1 FROM favorites fav WHERE fav.wallpaper_id = w.id), \
     w.author, w.author_url, w.source_url, w.license, w.license_url";

/// History entries kept. Older ones are dropped as new ones are added, so a
/// fast rotation does not grow the history forever.
pub const HISTORY_LIMIT: u32 = 1000;

const ROTATION_COLUMNS: &str = "id, collection_id, interval_seconds, rotation_mode, enabled";

const MAX_COLLECTION_NAME: usize = 60;

/// Current time as an RFC 3339 UTC timestamp, the format used for every
/// timestamp column.
pub fn now_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn map_wallpaper(row: &Row<'_>, offset: usize) -> rusqlite::Result<Wallpaper> {
    Ok(Wallpaper {
        id: Some(row.get(offset)?),
        provider: row.get(offset + 1)?,
        provider_id: row.get(offset + 2)?,
        title: row.get(offset + 3)?,
        image_url: row.get(offset + 4)?,
        thumbnail_url: row.get(offset + 5)?,
        local_path: row.get(offset + 6)?,
        width: row.get(offset + 7)?,
        height: row.get(offset + 8)?,
        created_at: Some(row.get(offset + 9)?),
        is_favorite: row.get(offset + 10)?,
        credit: Credit {
            author: row.get(offset + 11)?,
            author_url: row.get(offset + 12)?,
            source_url: row.get(offset + 13)?,
            license: row.get(offset + 14)?,
            license_url: row.get(offset + 15)?,
        },
    })
}

fn map_history(row: &Row<'_>) -> rusqlite::Result<HistoryEntry> {
    Ok(HistoryEntry { id: row.get(0)?, applied_at: row.get(1)?, wallpaper: map_wallpaper(row, 2)? })
}

fn map_rotation(row: &Row<'_>) -> rusqlite::Result<RotationSettings> {
    let mode: String = row.get(3)?;
    let rotation_mode = mode
        .parse::<RotationMode>()
        .map_err(|error| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(error)))?;
    Ok(RotationSettings {
        id: row.get(0)?,
        collection_id: row.get(1)?,
        interval_seconds: row.get(2)?,
        rotation_mode,
        enabled: row.get(4)?,
    })
}

fn is_unique_violation(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    )
}

fn validate_collection_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("Give the collection a name.".into()));
    }
    if name.chars().count() > MAX_COLLECTION_NAME {
        return Err(AppError::Validation(format!("Collection names can be up to {MAX_COLLECTION_NAME} characters.")));
    }
    Ok(name.to_owned())
}

fn duplicate_name_error(name: &str) -> AppError {
    AppError::Validation(format!("A collection named \"{name}\" already exists."))
}

fn collection_member_ids(conn: &Connection, collection_id: i64) -> rusqlite::Result<Vec<i64>> {
    let mut statement =
        conn.prepare_cached("SELECT wallpaper_id FROM collection_wallpapers WHERE collection_id = ?1")?;
    let ids = statement.query_map([collection_id], |row| row.get(0))?.collect();
    ids
}

/// Clears the local path of each of `wallpaper_ids` whose download nothing
/// uses any more: it is in no collection, not a favorite and not the
/// wallpaper on screen. Returns those wallpapers, with the paths they had.
fn forget_unused_downloads(conn: &Connection, wallpaper_ids: &[i64]) -> rusqlite::Result<Vec<Wallpaper>> {
    let mut statement = conn.prepare_cached(&format!(
        "SELECT {WALLPAPER_COLUMNS} FROM wallpapers w
         WHERE w.id = ?1
           AND w.local_path IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM collection_wallpapers cw WHERE cw.wallpaper_id = w.id)
           AND NOT EXISTS (SELECT 1 FROM favorites f WHERE f.wallpaper_id = w.id)
           AND w.id IS NOT (SELECT h.wallpaper_id FROM wallpaper_history h
                            ORDER BY h.applied_at DESC, h.id DESC LIMIT 1)"
    ))?;
    let mut unused = Vec::new();
    for id in wallpaper_ids {
        if let Some(wallpaper) = statement.query_row([id], |row| map_wallpaper(row, 0)).optional()? {
            unused.push(wallpaper);
        }
    }
    for wallpaper in &unused {
        conn.execute("UPDATE wallpapers SET local_path = NULL WHERE id = ?1", [wallpaper.id])?;
    }
    Ok(unused)
}

/// See [`Database::upsert_wallpaper`]. Returns the wallpaper's id.
fn upsert_wallpaper_id(conn: &Connection, input: &WallpaperInput) -> rusqlite::Result<i64> {
    conn.prepare_cached(
        "INSERT INTO wallpapers
             (provider, provider_id, title, image_url, thumbnail_url, width, height, created_at,
              author, author_url, source_url, license, license_url)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT (provider, provider_id) DO UPDATE SET
             title = CASE WHEN excluded.title <> '' THEN excluded.title ELSE wallpapers.title END,
             image_url = excluded.image_url,
             thumbnail_url = excluded.thumbnail_url,
             author = COALESCE(excluded.author, wallpapers.author),
             author_url = COALESCE(excluded.author_url, wallpapers.author_url),
             source_url = COALESCE(excluded.source_url, wallpapers.source_url),
             license = COALESCE(excluded.license, wallpapers.license),
             license_url = COALESCE(excluded.license_url, wallpapers.license_url),
             width = CASE WHEN wallpapers.local_path IS NULL AND excluded.width > 0
                          THEN excluded.width ELSE wallpapers.width END,
             height = CASE WHEN wallpapers.local_path IS NULL AND excluded.height > 0
                           THEN excluded.height ELSE wallpapers.height END
         RETURNING id",
    )?
    .query_row(
        params![
            input.provider,
            input.provider_id,
            input.title.trim(),
            input.image_url,
            input.thumbnail_url,
            input.width,
            input.height,
            now_timestamp(),
            input.credit.author,
            input.credit.author_url,
            input.credit.source_url,
            input.credit.license,
            input.credit.license_url,
        ],
        |row| row.get(0),
    )
}

fn collection_not_found(id: i64) -> AppError {
    AppError::NotFound(format!("Collection {id} no longer exists."))
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> AppResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> AppResult<Self> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;")?;
        migrate(&mut conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn conn(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| AppError::Internal("The database is unavailable after an earlier failure.".into()))
    }

    // ----- wallpapers -------------------------------------------------------

    /// Inserts a wallpaper or refreshes the stored metadata of an existing
    /// one, identified by provider and provider id. Dimensions measured from
    /// a downloaded file are kept.
    pub fn upsert_wallpaper(&self, input: &WallpaperInput) -> AppResult<Wallpaper> {
        let id = {
            let conn = self.conn()?;
            upsert_wallpaper_id(&conn, input)?
        };
        self.get_wallpaper(id)
    }

    pub fn get_wallpaper(&self, id: i64) -> AppResult<Wallpaper> {
        let conn = self.conn()?;
        conn.query_row(&format!("SELECT {WALLPAPER_COLUMNS} FROM wallpapers w WHERE w.id = ?1"), [id], |row| {
            map_wallpaper(row, 0)
        })
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("Wallpaper {id} no longer exists.")))
    }

    pub fn find_wallpaper(&self, provider: &str, provider_id: &str) -> AppResult<Option<Wallpaper>> {
        let conn = self.conn()?;
        Ok(conn
            .query_row(
                &format!("SELECT {WALLPAPER_COLUMNS} FROM wallpapers w WHERE w.provider = ?1 AND w.provider_id = ?2"),
                [provider, provider_id],
                |row| map_wallpaper(row, 0),
            )
            .optional()?)
    }

    /// Ids and provider ids of the provider's wallpapers saved without an
    /// author or license.
    pub fn wallpapers_missing_credit(&self, provider: &str) -> AppResult<Vec<(i64, String)>> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(
            "SELECT id, provider_id FROM wallpapers
             WHERE provider = ?1 AND author IS NULL AND license IS NULL ORDER BY id",
        )?;
        let rows = statement.query_map([provider], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// Fills in the parts of a wallpaper's credit that `credit` has.
    pub fn add_credit(&self, id: i64, credit: &Credit) -> AppResult<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE wallpapers
             SET author = COALESCE(?2, author),
                 author_url = COALESCE(?3, author_url),
                 source_url = COALESCE(?4, source_url),
                 license = COALESCE(?5, license),
                 license_url = COALESCE(?6, license_url)
             WHERE id = ?1",
            params![id, credit.author, credit.author_url, credit.source_url, credit.license, credit.license_url],
        )?;
        Ok(())
    }

    /// The wallpaper's colors, if they were worked out before.
    pub fn palette(&self, id: i64) -> AppResult<Option<Palette>> {
        let conn = self.conn()?;
        let json: Option<String> = conn
            .query_row("SELECT palette FROM wallpapers WHERE id = ?1", [id], |row| row.get(0))
            .optional()?
            .flatten();
        // A palette from an older format is worked out again.
        Ok(json.and_then(|json| serde_json::from_str(&json).ok()))
    }

    pub fn set_palette(&self, id: i64, palette: &Palette) -> AppResult<()> {
        let conn = self.conn()?;
        conn.execute("UPDATE wallpapers SET palette = ?2 WHERE id = ?1", params![id, serde_json::to_string(palette)?])?;
        Ok(())
    }

    pub fn set_local_file(&self, id: i64, local_path: Option<&str>, width: u32, height: u32) -> AppResult<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE wallpapers
             SET local_path = ?2,
                 width = CASE WHEN ?3 > 0 THEN ?3 ELSE width END,
                 height = CASE WHEN ?4 > 0 THEN ?4 ELSE height END
             WHERE id = ?1",
            params![id, local_path, width, height],
        )?;
        Ok(())
    }

    /// Fills in the database id, local file and favorite flag of provider
    /// results that are already stored.
    pub fn annotate(&self, wallpapers: &mut [Wallpaper]) -> AppResult<()> {
        let conn = self.conn()?;
        let mut statement = conn.prepare_cached(&format!(
            "SELECT {WALLPAPER_COLUMNS} FROM wallpapers w WHERE w.provider = ?1 AND w.provider_id = ?2"
        ))?;
        for wallpaper in wallpapers.iter_mut() {
            let stored = statement
                .query_row([&wallpaper.provider, &wallpaper.provider_id], |row| map_wallpaper(row, 0))
                .optional()?;
            if let Some(stored) = stored {
                wallpaper.id = stored.id;
                wallpaper.local_path = stored.local_path;
                wallpaper.is_favorite = stored.is_favorite;
                wallpaper.created_at = stored.created_at;
            }
        }
        Ok(())
    }

    pub fn wallpaper_status(&self, provider: &str, provider_id: &str) -> AppResult<WallpaperStatus> {
        let wallpaper = self.find_wallpaper(provider, provider_id)?;
        let collection_ids = match wallpaper.as_ref().and_then(|w| w.id) {
            Some(id) => {
                let conn = self.conn()?;
                let mut statement = conn.prepare(
                    "SELECT collection_id FROM collection_wallpapers WHERE wallpaper_id = ?1 ORDER BY collection_id",
                )?;
                let ids = statement.query_map([id], |row| row.get(0))?.collect::<Result<_, _>>()?;
                ids
            }
            None => Vec::new(),
        };
        Ok(WallpaperStatus { wallpaper, collection_ids })
    }

    // ----- collections ------------------------------------------------------

    pub fn list_collections(&self) -> AppResult<Vec<Collection>> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(
            "SELECT c.id, c.name, c.created_at,
                    (SELECT COUNT(*) FROM collection_wallpapers cw WHERE cw.collection_id = c.id)
             FROM collections c
             ORDER BY c.name COLLATE NOCASE",
        )?;
        let mut collections = statement
            .query_map([], |row| {
                Ok(Collection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get(2)?,
                    wallpaper_count: row.get(3)?,
                    preview_thumbnails: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for collection in &mut collections {
            collection.preview_thumbnails = preview_thumbnails(&conn, collection.id)?;
        }
        Ok(collections)
    }

    pub fn get_collection(&self, id: i64) -> AppResult<Collection> {
        let conn = self.conn()?;
        let mut collection = conn
            .query_row(
                "SELECT c.id, c.name, c.created_at,
                        (SELECT COUNT(*) FROM collection_wallpapers cw WHERE cw.collection_id = c.id)
                 FROM collections c WHERE c.id = ?1",
                [id],
                |row| {
                    Ok(Collection {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        created_at: row.get(2)?,
                        wallpaper_count: row.get(3)?,
                        preview_thumbnails: Vec::new(),
                    })
                },
            )
            .optional()?
            .ok_or_else(|| collection_not_found(id))?;
        collection.preview_thumbnails = preview_thumbnails(&conn, id)?;
        Ok(collection)
    }

    pub fn create_collection(&self, name: &str) -> AppResult<Collection> {
        let name = validate_collection_name(name)?;
        let id = {
            let conn = self.conn()?;
            match conn
                .execute("INSERT INTO collections (name, created_at) VALUES (?1, ?2)", params![name, now_timestamp()])
            {
                Ok(_) => conn.last_insert_rowid(),
                Err(error) if is_unique_violation(&error) => return Err(duplicate_name_error(&name)),
                Err(error) => return Err(error.into()),
            }
        };
        self.get_collection(id)
    }

    pub fn rename_collection(&self, id: i64, name: &str) -> AppResult<Collection> {
        let name = validate_collection_name(name)?;
        {
            let conn = self.conn()?;
            match conn.execute("UPDATE collections SET name = ?2 WHERE id = ?1", params![id, name]) {
                Ok(0) => return Err(collection_not_found(id)),
                Ok(_) => {}
                Err(error) if is_unique_violation(&error) => return Err(duplicate_name_error(&name)),
                Err(error) => return Err(error.into()),
            }
        }
        self.get_collection(id)
    }

    /// Deletes a collection, and forgets the downloads that only it used:
    /// those of its wallpapers that are in no other collection, not a
    /// favorite and not the current wallpaper. Returns those wallpapers, with
    /// the local paths they had, so the caller can delete the files. The
    /// wallpapers stay in the history and are downloaded again if applied.
    pub fn delete_collection(&self, id: i64) -> AppResult<Vec<Wallpaper>> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let members = collection_member_ids(&tx, id)?;
        if tx.execute("DELETE FROM collections WHERE id = ?1", [id])? == 0 {
            return Err(collection_not_found(id));
        }
        let unused = forget_unused_downloads(&tx, &members)?;
        tx.commit()?;
        Ok(unused)
    }

    /// Removes wallpapers from a collection as repeats of pictures it already
    /// has, and forgets their downloads if nothing else uses them, as
    /// [`Self::delete_collection`] does. Returns how many were removed and
    /// the forgotten downloads.
    pub fn remove_repeats(&self, collection_id: i64, wallpaper_ids: &[i64]) -> AppResult<(usize, Vec<Wallpaper>)> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut removed = 0;
        for wallpaper_id in wallpaper_ids {
            removed += tx.execute(
                "DELETE FROM collection_wallpapers WHERE collection_id = ?1 AND wallpaper_id = ?2",
                [collection_id, *wallpaper_id],
            )?;
        }
        let unused = forget_unused_downloads(&tx, wallpaper_ids)?;
        tx.commit()?;
        Ok((removed, unused))
    }

    /// Ids of the collections a wallpaper is in.
    pub fn collections_containing(&self, wallpaper_id: i64) -> AppResult<Vec<i64>> {
        let conn = self.conn()?;
        let mut statement = conn.prepare("SELECT collection_id FROM collection_wallpapers WHERE wallpaper_id = ?1")?;
        let ids = statement.query_map([wallpaper_id], |row| row.get(0))?.collect::<Result<_, _>>()?;
        Ok(ids)
    }

    /// The wallpaper's fingerprint, if it was worked out before.
    pub fn fingerprint(&self, id: i64) -> AppResult<Option<Fingerprint>> {
        let conn = self.conn()?;
        let bytes: Option<Vec<u8>> = conn
            .query_row("SELECT fingerprint FROM wallpapers WHERE id = ?1", [id], |row| row.get(0))
            .optional()?
            .flatten();
        Ok(bytes.and_then(|bytes| Fingerprint::from_bytes(&bytes)))
    }

    pub fn set_fingerprint(&self, id: i64, fingerprint: &Fingerprint) -> AppResult<()> {
        let conn = self.conn()?;
        conn.execute("UPDATE wallpapers SET fingerprint = ?2 WHERE id = ?1", params![id, fingerprint.to_bytes()])?;
        Ok(())
    }

    /// The collection's wallpapers that have a fingerprint, in the order they
    /// were added.
    pub fn fingerprinted_wallpapers(&self, collection_id: i64) -> AppResult<Vec<Candidate>> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(
            "SELECT w.id, w.width, w.height, w.fingerprint
             FROM collection_wallpapers cw JOIN wallpapers w ON w.id = cw.wallpaper_id
             WHERE cw.collection_id = ?1 AND w.fingerprint IS NOT NULL
             ORDER BY cw.rowid",
        )?;
        let rows = statement.query_map([collection_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, u32>(1)?, row.get::<_, u32>(2)?, row.get::<_, Vec<u8>>(3)?))
        })?;
        let mut candidates = Vec::new();
        for row in rows {
            let (wallpaper_id, width, height, bytes) = row?;
            if let Some(fingerprint) = Fingerprint::from_bytes(&bytes) {
                candidates.push(Candidate { wallpaper_id, width, height, fingerprint });
            }
        }
        Ok(candidates)
    }

    /// Wallpapers of a collection in the order they were added. Sequential
    /// rotation follows this order.
    pub fn collection_wallpapers(&self, collection_id: i64) -> AppResult<Vec<Wallpaper>> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(&format!(
            "SELECT {WALLPAPER_COLUMNS}
             FROM collection_wallpapers cw
             JOIN wallpapers w ON w.id = cw.wallpaper_id
             WHERE cw.collection_id = ?1
             ORDER BY cw.rowid"
        ))?;
        let wallpapers =
            statement.query_map([collection_id], |row| map_wallpaper(row, 0))?.collect::<Result<_, _>>()?;
        Ok(wallpapers)
    }

    pub fn add_to_collection(&self, collection_id: i64, wallpaper_id: i64) -> AppResult<()> {
        let conn = self.conn()?;
        let exists: bool =
            conn.query_row("SELECT EXISTS (SELECT 1 FROM collections WHERE id = ?1)", [collection_id], |row| {
                row.get(0)
            })?;
        if !exists {
            return Err(collection_not_found(collection_id));
        }
        conn.execute(
            "INSERT OR IGNORE INTO collection_wallpapers (collection_id, wallpaper_id) VALUES (?1, ?2)",
            [collection_id, wallpaper_id],
        )?;
        Ok(())
    }

    /// Saves wallpapers into the collection called `name`, creating it if no
    /// collection has that name. Wallpapers already in it keep their place;
    /// new ones are added in the order given. Either everything is saved or
    /// nothing is.
    pub fn save_to_collection(&self, name: &str, wallpapers: &[WallpaperInput]) -> AppResult<Collection> {
        let name = validate_collection_name(name)?;
        let id = {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            let existing: Option<i64> =
                tx.query_row("SELECT id FROM collections WHERE name = ?1", [&name], |row| row.get(0)).optional()?;
            let collection_id = match existing {
                Some(id) => id,
                None => {
                    tx.execute(
                        "INSERT INTO collections (name, created_at) VALUES (?1, ?2)",
                        params![name, now_timestamp()],
                    )?;
                    tx.last_insert_rowid()
                }
            };
            for input in wallpapers {
                let wallpaper_id = upsert_wallpaper_id(&tx, input)?;
                tx.execute(
                    "INSERT OR IGNORE INTO collection_wallpapers (collection_id, wallpaper_id) VALUES (?1, ?2)",
                    [collection_id, wallpaper_id],
                )?;
            }
            tx.commit()?;
            collection_id
        };
        self.get_collection(id)
    }

    pub fn remove_from_collection(&self, collection_id: i64, wallpaper_id: i64) -> AppResult<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM collection_wallpapers WHERE collection_id = ?1 AND wallpaper_id = ?2",
            [collection_id, wallpaper_id],
        )?;
        Ok(())
    }

    // ----- favorites --------------------------------------------------------

    pub fn list_favorites(&self) -> AppResult<Vec<Wallpaper>> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(&format!(
            "SELECT {WALLPAPER_COLUMNS}
             FROM favorites f
             JOIN wallpapers w ON w.id = f.wallpaper_id
             ORDER BY f.created_at DESC, f.id DESC"
        ))?;
        let wallpapers = statement.query_map([], |row| map_wallpaper(row, 0))?.collect::<Result<_, _>>()?;
        Ok(wallpapers)
    }

    pub fn add_favorite(&self, wallpaper_id: i64) -> AppResult<()> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR IGNORE INTO favorites (wallpaper_id, created_at) VALUES (?1, ?2)",
            params![wallpaper_id, now_timestamp()],
        )?;
        Ok(())
    }

    pub fn remove_favorite(&self, wallpaper_id: i64) -> AppResult<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM favorites WHERE wallpaper_id = ?1", [wallpaper_id])?;
        Ok(())
    }

    // ----- history ----------------------------------------------------------

    pub fn add_history(&self, wallpaper_id: i64) -> AppResult<HistoryEntry> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO wallpaper_history (wallpaper_id, applied_at) VALUES (?1, ?2)",
            params![wallpaper_id, now_timestamp()],
        )?;
        let id = conn.last_insert_rowid();
        conn.execute(
            "DELETE FROM wallpaper_history WHERE id IN (
                 SELECT id FROM wallpaper_history ORDER BY applied_at DESC, id DESC LIMIT -1 OFFSET ?1)",
            [HISTORY_LIMIT],
        )?;
        Ok(conn.query_row(
            &format!(
                "SELECT h.id, h.applied_at, {WALLPAPER_COLUMNS}
                 FROM wallpaper_history h JOIN wallpapers w ON w.id = h.wallpaper_id
                 WHERE h.id = ?1"
            ),
            [id],
            map_history,
        )?)
    }

    /// Whether the wallpaper has ever been applied.
    pub fn has_history(&self, wallpaper_id: i64) -> AppResult<bool> {
        let conn = self.conn()?;
        Ok(conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM wallpaper_history WHERE wallpaper_id = ?1)",
            [wallpaper_id],
            |row| row.get(0),
        )?)
    }

    /// Applied wallpapers, newest first.
    pub fn list_history(&self, limit: u32, offset: u32) -> AppResult<Vec<HistoryEntry>> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(&format!(
            "SELECT h.id, h.applied_at, {WALLPAPER_COLUMNS}
             FROM wallpaper_history h JOIN wallpapers w ON w.id = h.wallpaper_id
             ORDER BY h.applied_at DESC, h.id DESC
             LIMIT ?1 OFFSET ?2"
        ))?;
        let entries = statement.query_map([limit, offset], map_history)?.collect::<Result<_, _>>()?;
        Ok(entries)
    }

    /// The most recent application of each wallpaper, newest first.
    pub fn recent_wallpapers(&self, limit: u32) -> AppResult<Vec<HistoryEntry>> {
        let conn = self.conn()?;
        let mut statement = conn.prepare(&format!(
            "SELECT h.id, h.applied_at, {WALLPAPER_COLUMNS}
             FROM wallpaper_history h JOIN wallpapers w ON w.id = h.wallpaper_id
             WHERE h.id = (SELECT h2.id FROM wallpaper_history h2
                           WHERE h2.wallpaper_id = h.wallpaper_id
                           ORDER BY h2.applied_at DESC, h2.id DESC LIMIT 1)
             ORDER BY h.applied_at DESC, h.id DESC
             LIMIT ?1"
        ))?;
        let entries = statement.query_map([limit], map_history)?.collect::<Result<_, _>>()?;
        Ok(entries)
    }

    pub fn latest_history(&self) -> AppResult<Option<HistoryEntry>> {
        Ok(self.list_history(1, 0)?.into_iter().next())
    }

    /// The wallpaper of this collection that was applied most recently.
    pub fn last_applied_in_collection(&self, collection_id: i64) -> AppResult<Option<i64>> {
        let conn = self.conn()?;
        Ok(conn
            .query_row(
                "SELECT h.wallpaper_id
                 FROM wallpaper_history h
                 JOIN collection_wallpapers cw ON cw.wallpaper_id = h.wallpaper_id AND cw.collection_id = ?1
                 ORDER BY h.applied_at DESC, h.id DESC
                 LIMIT 1",
                [collection_id],
                |row| row.get(0),
            )
            .optional()?)
    }

    // ----- rotation settings ------------------------------------------------

    pub fn rotation_settings(&self, collection_id: i64) -> AppResult<Option<RotationSettings>> {
        let conn = self.conn()?;
        Ok(conn
            .query_row(
                &format!("SELECT {ROTATION_COLUMNS} FROM rotation_settings WHERE collection_id = ?1"),
                [collection_id],
                map_rotation,
            )
            .optional()?)
    }

    pub fn enabled_rotation(&self) -> AppResult<Option<RotationSettings>> {
        let conn = self.conn()?;
        Ok(conn
            .query_row(&format!("SELECT {ROTATION_COLUMNS} FROM rotation_settings WHERE enabled = 1"), [], map_rotation)
            .optional()?)
    }

    /// Stores the interval and mode for a collection without changing
    /// whether its rotation is running.
    pub fn save_rotation_settings(
        &self,
        collection_id: i64,
        interval_seconds: u32,
        mode: RotationMode,
    ) -> AppResult<RotationSettings> {
        let conn = self.conn()?;
        let exists: bool =
            conn.query_row("SELECT EXISTS (SELECT 1 FROM collections WHERE id = ?1)", [collection_id], |row| {
                row.get(0)
            })?;
        if !exists {
            return Err(collection_not_found(collection_id));
        }
        Ok(conn.query_row(
            &format!(
                "INSERT INTO rotation_settings (collection_id, interval_seconds, rotation_mode, enabled)
                 VALUES (?1, ?2, ?3, 0)
                 ON CONFLICT (collection_id) DO UPDATE SET
                     interval_seconds = excluded.interval_seconds,
                     rotation_mode = excluded.rotation_mode
                 RETURNING {ROTATION_COLUMNS}"
            ),
            params![collection_id, interval_seconds, mode.as_str()],
            map_rotation,
        )?)
    }

    /// Makes this collection the one that rotates, stopping any other.
    pub fn enable_rotation(&self, collection_id: i64) -> AppResult<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("UPDATE rotation_settings SET enabled = 0 WHERE collection_id <> ?1", [collection_id])?;
        let updated =
            tx.execute("UPDATE rotation_settings SET enabled = 1 WHERE collection_id = ?1", [collection_id])?;
        if updated == 0 {
            return Err(AppError::NotFound("Save rotation settings for this collection first.".into()));
        }
        tx.commit()?;
        Ok(())
    }

    pub fn disable_rotation(&self) -> AppResult<()> {
        let conn = self.conn()?;
        conn.execute("UPDATE rotation_settings SET enabled = 0 WHERE enabled = 1", [])?;
        Ok(())
    }
}

fn preview_thumbnails(conn: &Connection, collection_id: i64) -> AppResult<Vec<String>> {
    let mut statement = conn.prepare_cached(
        "SELECT w.thumbnail_url
         FROM collection_wallpapers cw JOIN wallpapers w ON w.id = cw.wallpaper_id
         WHERE cw.collection_id = ?1
         ORDER BY cw.rowid DESC
         LIMIT 4",
    )?;
    let thumbnails = statement.query_map([collection_id], |row| row.get(0))?.collect::<Result<_, _>>()?;
    Ok(thumbnails)
}

fn migrate(conn: &mut Connection) -> AppResult<()> {
    let applied: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let applied = usize::try_from(applied).unwrap_or(0);
    if applied > MIGRATIONS.len() {
        return Err(AppError::Internal("The library database was created by a newer version of Kankulirchi.".into()));
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(applied) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (index + 1) as i64)?;
        tx.commit()?;
        log::info!("applied database migration {}", index + 1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn input(id: &str) -> WallpaperInput {
        WallpaperInput {
            provider: "wallhaven".into(),
            provider_id: id.into(),
            title: format!("Wallhaven {id}"),
            image_url: format!("https://w.wallhaven.cc/full/xx/wallhaven-{id}.jpg"),
            thumbnail_url: format!("https://th.wallhaven.cc/lg/xx/{id}.jpg"),
            width: 1920,
            height: 1080,
            credit: Default::default(),
        }
    }

    fn db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn migration_converts_rotation_intervals_to_seconds() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute_batch(
            "INSERT INTO collections (id, name, created_at) VALUES (1, 'Lakes', '2026-10-01T00:00:00.000Z');
             INSERT INTO rotation_settings (collection_id, interval_minutes, rotation_mode, enabled)
             VALUES (1, 15, 'random', 1);",
        )
        .unwrap();

        migrate(&mut conn).unwrap();
        let db = Database { conn: Mutex::new(conn) };
        let settings = db.enabled_rotation().unwrap().unwrap();
        assert_eq!(settings.interval_seconds, 900);
        assert_eq!(settings.rotation_mode, RotationMode::Random);

        // The CHECK constraint follows the renamed column.
        let conn = db.conn().unwrap();
        assert!(conn.execute("UPDATE rotation_settings SET interval_seconds = 0", []).is_err());
    }

    #[test]
    fn migrations_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.db");
        drop(Database::open(&path).unwrap());
        let db = Database::open(&path).unwrap();
        assert!(db.list_collections().unwrap().is_empty());
    }

    #[test]
    fn upsert_returns_the_same_row_for_the_same_provider_id() {
        let db = db();
        let first = db.upsert_wallpaper(&input("a")).unwrap();
        let mut changed = input("a");
        changed.title = "Renamed".into();
        let second = db.upsert_wallpaper(&changed).unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(second.title, "Renamed");
    }

    #[test]
    fn measured_dimensions_survive_upserts() {
        let db = db();
        let wallpaper = db.upsert_wallpaper(&input("a")).unwrap();
        let id = wallpaper.id.unwrap();
        db.set_local_file(id, Some("/tmp/a.jpg"), 3840, 2160).unwrap();
        let again = db.upsert_wallpaper(&input("a")).unwrap();
        assert_eq!((again.width, again.height), (3840, 2160));
        assert_eq!(again.local_path.as_deref(), Some("/tmp/a.jpg"));
    }

    #[test]
    fn annotate_marks_stored_results() {
        let db = db();
        let stored = db.upsert_wallpaper(&input("a")).unwrap();
        db.add_favorite(stored.id.unwrap()).unwrap();

        let mut results = vec![
            crate::providers::ProviderWallpaper {
                provider_id: "a".into(),
                title: String::new(),
                image_url: String::new(),
                thumbnail_url: String::new(),
                width: 0,
                height: 0,
                credit: Default::default(),
            }
            .into_wallpaper("wallhaven"),
            crate::providers::ProviderWallpaper {
                provider_id: "b".into(),
                title: String::new(),
                image_url: String::new(),
                thumbnail_url: String::new(),
                width: 0,
                height: 0,
                credit: Default::default(),
            }
            .into_wallpaper("wallhaven"),
        ];
        db.annotate(&mut results).unwrap();
        assert_eq!(results[0].id, stored.id);
        assert!(results[0].is_favorite);
        assert_eq!(results[1].id, None);
        assert!(!results[1].is_favorite);
    }

    #[test]
    fn collection_lifecycle() {
        let db = db();
        let collection = db.create_collection("  Forest  ").unwrap();
        assert_eq!(collection.name, "Forest");

        assert!(matches!(db.create_collection("forest"), Err(AppError::Validation(_))));
        assert!(matches!(db.create_collection("   "), Err(AppError::Validation(_))));

        let renamed = db.rename_collection(collection.id, "Calm").unwrap();
        assert_eq!(renamed.name, "Calm");

        let a = db.upsert_wallpaper(&input("a")).unwrap().id.unwrap();
        let b = db.upsert_wallpaper(&input("b")).unwrap().id.unwrap();
        db.add_to_collection(collection.id, b).unwrap();
        db.add_to_collection(collection.id, a).unwrap();
        db.add_to_collection(collection.id, a).unwrap();

        let ids: Vec<_> = db.collection_wallpapers(collection.id).unwrap().into_iter().map(|w| w.id.unwrap()).collect();
        assert_eq!(ids, [b, a], "wallpapers keep the order they were added in");
        assert_eq!(db.get_collection(collection.id).unwrap().wallpaper_count, 2);

        let status = db.wallpaper_status("wallhaven", "a").unwrap();
        assert_eq!(status.collection_ids, [collection.id]);

        db.remove_from_collection(collection.id, b).unwrap();
        assert_eq!(db.get_collection(collection.id).unwrap().wallpaper_count, 1);

        db.delete_collection(collection.id).unwrap();
        assert!(matches!(db.get_collection(collection.id), Err(AppError::NotFound(_))));
        assert!(matches!(db.delete_collection(collection.id), Err(AppError::NotFound(_))));
        assert!(matches!(db.add_to_collection(collection.id, a), Err(AppError::NotFound(_))));
    }

    fn provider_ids(db: &Database, collection_id: i64) -> Vec<String> {
        db.collection_wallpapers(collection_id).unwrap().into_iter().map(|w| w.provider_id).collect()
    }

    #[test]
    fn save_to_collection_creates_the_collection_in_order() {
        let db = db();
        let collection = db.save_to_collection(" mountains ", &[input("b"), input("a"), input("b")]).unwrap();
        assert_eq!(collection.name, "mountains");
        assert_eq!(collection.wallpaper_count, 2);
        assert_eq!(provider_ids(&db, collection.id), ["b", "a"]);
    }

    #[test]
    fn save_to_collection_adds_to_an_existing_collection() {
        let db = db();
        let existing = db.create_collection("Mountains").unwrap();
        let a = db.upsert_wallpaper(&input("a")).unwrap().id.unwrap();
        db.add_to_collection(existing.id, a).unwrap();

        let saved = db.save_to_collection("mountains", &[input("c"), input("a")]).unwrap();
        assert_eq!(saved.id, existing.id, "names match regardless of case");
        assert_eq!(saved.name, "Mountains");
        assert_eq!(provider_ids(&db, saved.id), ["a", "c"]);
        assert_eq!(db.list_collections().unwrap().len(), 1);
    }

    #[test]
    fn save_to_collection_rejects_an_empty_name_before_saving() {
        let db = db();
        assert!(matches!(db.save_to_collection("  ", &[input("a")]), Err(AppError::Validation(_))));
        assert!(db.find_wallpaper("wallhaven", "a").unwrap().is_none());
    }

    #[test]
    fn favorites_add_and_remove() {
        let db = db();
        let id = db.upsert_wallpaper(&input("a")).unwrap().id.unwrap();
        db.add_favorite(id).unwrap();
        db.add_favorite(id).unwrap();
        assert_eq!(db.list_favorites().unwrap().len(), 1);
        assert!(db.get_wallpaper(id).unwrap().is_favorite);
        db.remove_favorite(id).unwrap();
        assert!(db.list_favorites().unwrap().is_empty());
    }

    #[test]
    fn migration_credits_wallpapers_saved_earlier() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.execute_batch(MIGRATIONS[1]).unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        conn.execute_batch(
            "INSERT INTO wallpapers (provider, provider_id, title, image_url, thumbnail_url, width, height, created_at)
             VALUES ('unsplash', 'eOLpJytrbsQ', 'Photo by Kalen Emsley on Unsplash', 'https://images.unsplash.com/1',
                     'https://images.unsplash.com/1t', 1, 1, '2026-10-01T00:00:00.000Z'),
                    ('pexels', '42', 'Photo by Ada on Pexels', 'https://images.pexels.com/1',
                     'https://images.pexels.com/1t', 1, 1, '2026-10-01T00:00:00.000Z'),
                    ('wikimedia', '113030211', 'Mountains', 'https://upload.wikimedia.org/1',
                     'https://upload.wikimedia.org/1t', 1, 1, '2026-10-01T00:00:00.000Z');",
        )
        .unwrap();

        migrate(&mut conn).unwrap();
        let db = Database { conn: Mutex::new(conn) };
        let unsplash = db.find_wallpaper("unsplash", "eOLpJytrbsQ").unwrap().unwrap();
        assert_eq!(unsplash.title, "Unsplash photo eOLpJytrbsQ");
        assert_eq!(unsplash.credit.author.as_deref(), Some("Kalen Emsley"));
        assert_eq!(
            unsplash.credit.source_url.as_deref(),
            Some("https://unsplash.com/photos/eOLpJytrbsQ?utm_source=kankulirchi&utm_medium=referral")
        );
        let pexels = db.find_wallpaper("pexels", "42").unwrap().unwrap();
        assert_eq!((pexels.title.as_str(), pexels.credit.author.as_deref()), ("Pexels photo 42", Some("Ada")));
        let commons = db.find_wallpaper("wikimedia", "113030211").unwrap().unwrap();
        assert_eq!(commons.title, "Mountains");
        assert_eq!(commons.credit.source_url.as_deref(), Some("https://commons.wikimedia.org/?curid=113030211"));
    }

    #[test]
    fn repeats_are_removed_and_downloads_nothing_uses_are_forgotten() {
        let db = db();
        let collection = db.create_collection("Space").unwrap().id;
        let other = db.create_collection("Stars").unwrap().id;
        let mut ids = HashMap::new();
        for name in ["only_here", "also_elsewhere", "favorite"] {
            let id = db.upsert_wallpaper(&input(name)).unwrap().id.unwrap();
            db.add_to_collection(collection, id).unwrap();
            db.set_local_file(id, Some(&format!("/cache/wallhaven-{name}.jpg")), 0, 0).unwrap();
            ids.insert(name, id);
        }
        db.add_to_collection(other, ids["also_elsewhere"]).unwrap();
        db.add_favorite(ids["favorite"]).unwrap();

        let (removed, unused) =
            db.remove_repeats(collection, &[ids["only_here"], ids["also_elsewhere"], ids["favorite"]]).unwrap();
        assert_eq!(removed, 3);
        assert_eq!(unused.iter().map(|wallpaper| wallpaper.id.unwrap()).collect::<Vec<_>>(), [ids["only_here"]]);
        assert_eq!(db.get_wallpaper(ids["only_here"]).unwrap().local_path, None);
        assert!(db.get_wallpaper(ids["also_elsewhere"]).unwrap().local_path.is_some());
        assert_eq!(db.get_collection(collection).unwrap().wallpaper_count, 0);
        // Removing them again changes nothing.
        assert_eq!(db.remove_repeats(collection, &[ids["only_here"]]).unwrap().0, 0);
    }

    #[test]
    fn fingerprints_are_listed_in_the_order_wallpapers_were_added() {
        let db = db();
        let collection = db.create_collection("Space").unwrap().id;
        let picture = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(64, 36, |x, y| {
            image::Rgb([x as u8 * 4, y as u8 * 7, 90])
        }));
        let fingerprint = Fingerprint::of(&picture);
        let (b, a, unprinted) = (
            db.upsert_wallpaper(&input("b")).unwrap().id.unwrap(),
            db.upsert_wallpaper(&input("a")).unwrap().id.unwrap(),
            db.upsert_wallpaper(&input("c")).unwrap().id.unwrap(),
        );
        for id in [b, a, unprinted] {
            db.add_to_collection(collection, id).unwrap();
        }
        assert_eq!(db.fingerprint(a).unwrap(), None);
        db.set_fingerprint(a, &fingerprint).unwrap();
        db.set_fingerprint(b, &fingerprint).unwrap();
        assert_eq!(db.fingerprint(a).unwrap(), Some(fingerprint));

        let listed: Vec<_> =
            db.fingerprinted_wallpapers(collection).unwrap().iter().map(|c| (c.wallpaper_id, c.width)).collect();
        assert_eq!(listed, [(b, 1920), (a, 1920)]);
        assert_eq!(db.collections_containing(a).unwrap(), [collection]);
    }

    #[test]
    fn missing_credits_can_be_filled_in() {
        let db = db();
        let bare = db.upsert_wallpaper(&input("bare")).unwrap().id.unwrap();
        let mut credited = input("credited");
        credited.credit.license = Some("CC BY 4.0".into());
        db.upsert_wallpaper(&credited).unwrap();

        assert_eq!(db.wallpapers_missing_credit("wallhaven").unwrap(), [(bare, "bare".to_string())]);
        assert!(db.wallpapers_missing_credit("unsplash").unwrap().is_empty());

        let credit = Credit { author: Some("Ada".into()), license: Some("CC BY-SA 4.0".into()), ..Credit::default() };
        db.add_credit(bare, &credit).unwrap();
        assert_eq!(db.get_wallpaper(bare).unwrap().credit, credit);
        assert!(db.wallpapers_missing_credit("wallhaven").unwrap().is_empty());
    }

    #[test]
    fn palettes_are_stored_and_unreadable_ones_ignored() {
        let db = db();
        let id = db.upsert_wallpaper(&input("a")).unwrap().id.unwrap();
        assert_eq!(db.palette(id).unwrap(), None);
        let palette = Palette {
            colors: vec![crate::theme::palette::PaletteColor { hex: "#1e64c8".into(), share: 0.6 }],
            vivid: "#1e64c8".into(),
            accent_color: "blue".into(),
            dark: false,
        };
        db.set_palette(id, &palette).unwrap();
        assert_eq!(db.palette(id).unwrap(), Some(palette));
        db.conn().unwrap().execute("UPDATE wallpapers SET palette = '{\"old\": 1}' WHERE id = ?1", [id]).unwrap();
        assert_eq!(db.palette(id).unwrap(), None);
    }

    #[test]
    fn credits_are_stored_and_kept_when_a_result_has_none() {
        let db = db();
        let mut credited = input("a");
        credited.credit = Credit {
            author: Some("Ada".into()),
            source_url: Some("https://wallhaven.cc/w/a".into()),
            ..Credit::default()
        };
        db.upsert_wallpaper(&credited).unwrap();
        let stored = db.upsert_wallpaper(&input("a")).unwrap();
        assert_eq!(stored.credit, credited.credit);
    }

    #[test]
    fn history_tracks_applications() {
        let db = db();
        let a = db.upsert_wallpaper(&input("a")).unwrap().id.unwrap();
        let b = db.upsert_wallpaper(&input("b")).unwrap().id.unwrap();
        db.add_history(a).unwrap();
        db.add_history(b).unwrap();
        let last = db.add_history(a).unwrap();

        let history = db.list_history(10, 0).unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].id, last.id);

        let recent: Vec<_> =
            db.recent_wallpapers(10).unwrap().into_iter().map(|entry| entry.wallpaper.id.unwrap()).collect();
        assert_eq!(recent, [a, b]);
        assert_eq!(db.latest_history().unwrap().unwrap().wallpaper.id, Some(a));
    }

    #[test]
    fn history_keeps_only_the_newest_entries() {
        let db = db();
        let a = db.upsert_wallpaper(&input("a")).unwrap().id.unwrap();
        let b = db.upsert_wallpaper(&input("b")).unwrap().id.unwrap();
        let first = db.add_history(a).unwrap();
        for _ in 0..HISTORY_LIMIT {
            db.add_history(a).unwrap();
        }
        let last = db.add_history(b).unwrap();

        let history = db.list_history(HISTORY_LIMIT + 10, 0).unwrap();
        assert_eq!(history.len(), HISTORY_LIMIT as usize);
        assert_eq!(history[0].id, last.id);
        assert!(history.iter().all(|entry| entry.id != first.id), "the oldest entries are dropped");
    }

    #[test]
    fn deleting_a_collection_frees_downloads_nothing_else_uses() {
        let db = db();
        let deleted = db.create_collection("Fog").unwrap().id;
        let other = db.create_collection("Lakes").unwrap().id;
        let mut ids = HashMap::new();
        for name in ["only", "shared", "favorite", "current", "applied_before", "not_downloaded"] {
            let id = db.upsert_wallpaper(&input(name)).unwrap().id.unwrap();
            db.add_to_collection(deleted, id).unwrap();
            if name != "not_downloaded" {
                db.set_local_file(id, Some(&format!("/cache/wallhaven-{name}.jpg")), 0, 0).unwrap();
            }
            ids.insert(name, id);
        }
        db.add_to_collection(other, ids["shared"]).unwrap();
        db.add_favorite(ids["favorite"]).unwrap();
        db.add_history(ids["applied_before"]).unwrap();
        db.add_history(ids["current"]).unwrap();

        let mut freed: Vec<_> = db
            .delete_collection(deleted)
            .unwrap()
            .into_iter()
            .map(|wallpaper| (wallpaper.provider_id, wallpaper.local_path.unwrap()))
            .collect();
        freed.sort();
        assert_eq!(
            freed,
            [
                ("applied_before".to_string(), "/cache/wallhaven-applied_before.jpg".to_string()),
                ("only".to_string(), "/cache/wallhaven-only.jpg".to_string()),
            ]
        );

        let local_path = |name: &str| db.get_wallpaper(ids[name]).unwrap().local_path;
        assert_eq!(local_path("only"), None);
        assert_eq!(local_path("applied_before"), None);
        for kept in ["shared", "favorite", "current"] {
            assert!(local_path(kept).is_some(), "{kept} keeps its download");
        }
        assert_eq!(db.list_history(10, 0).unwrap().len(), 2, "history is kept");
    }

    #[test]
    fn last_applied_in_collection_ignores_other_wallpapers() {
        let db = db();
        let collection = db.create_collection("Calm").unwrap().id;
        let a = db.upsert_wallpaper(&input("a")).unwrap().id.unwrap();
        let outside = db.upsert_wallpaper(&input("z")).unwrap().id.unwrap();
        db.add_to_collection(collection, a).unwrap();
        assert_eq!(db.last_applied_in_collection(collection).unwrap(), None);
        db.add_history(a).unwrap();
        db.add_history(outside).unwrap();
        assert_eq!(db.last_applied_in_collection(collection).unwrap(), Some(a));
    }

    #[test]
    fn only_one_rotation_is_enabled() {
        let db = db();
        let first = db.create_collection("One").unwrap().id;
        let second = db.create_collection("Two").unwrap().id;
        db.save_rotation_settings(first, 900, RotationMode::Sequential).unwrap();
        db.save_rotation_settings(second, 3600, RotationMode::Random).unwrap();

        db.enable_rotation(first).unwrap();
        assert_eq!(db.enabled_rotation().unwrap().unwrap().collection_id, first);

        db.enable_rotation(second).unwrap();
        let enabled = db.enabled_rotation().unwrap().unwrap();
        assert_eq!(enabled.collection_id, second);
        assert_eq!(enabled.rotation_mode, RotationMode::Random);
        assert!(!db.rotation_settings(first).unwrap().unwrap().enabled);

        // Saving settings keeps the enabled flag.
        let saved = db.save_rotation_settings(second, 45, RotationMode::Sequential).unwrap();
        assert!(saved.enabled);
        assert_eq!(saved.interval_seconds, 45);

        db.disable_rotation().unwrap();
        assert!(db.enabled_rotation().unwrap().is_none());
    }

    #[test]
    fn deleting_a_collection_removes_its_rotation() {
        let db = db();
        let collection = db.create_collection("One").unwrap().id;
        db.save_rotation_settings(collection, 900, RotationMode::Sequential).unwrap();
        db.enable_rotation(collection).unwrap();
        db.delete_collection(collection).unwrap();
        assert!(db.enabled_rotation().unwrap().is_none());
    }
}
