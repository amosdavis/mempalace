use anyhow::{bail, Result};
use mempalace::config::MempalaceConfig;
use mempalace::storage::{PalaceStore, TunnelRecord};
use std::path::Path;

pub fn run(from: Option<&str>, palace: Option<&str>) -> Result<()> {
    let config = MempalaceConfig::load();
    let palace_path = palace.unwrap_or(&config.palace_path);

    let sqlite_path = if let Some(f) = from {
        f.to_string()
    } else {
        let candidate = Path::new(palace_path)
            .parent()
            .unwrap_or(Path::new("."))
            .join("palace.sqlite3");
        if candidate.exists() {
            candidate.to_string_lossy().to_string()
        } else {
            bail!(
                "No SQLite file found. Specify --from <path> or place palace.sqlite3 \
                 next to the palace directory."
            );
        }
    };

    if !Path::new(&sqlite_path).exists() {
        bail!("SQLite file not found: {sqlite_path}");
    }

    println!("Migrating: {sqlite_path} → {palace_path}");

    let conn = rusqlite::Connection::open(&sqlite_path)?;
    let store = PalaceStore::open(palace_path)?;

    let mut stmt = conn.prepare(
        "SELECT id, wing, room, content, content_type, source_file, mtime_unix, chunk_index \
         FROM drawers"
    )?;

    let mut count = 0usize;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<f64>>(6)?,
            row.get::<_, Option<i64>>(7)?,
        ))
    })?;

    for row in rows {
        let (_id, wing, room, content, content_type, source_file, mtime, chunk_idx) = row?;
        store.upsert_drawer(
            &wing,
            &room,
            &content,
            &content_type,
            source_file.as_deref(),
            mtime,
            chunk_idx,
            None,
        )?;
        count += 1;
    }

    println!("  Drawers migrated: {count}");

    let tunnel_result = conn.prepare(
        "SELECT tunnel_id, from_wing, from_room, to_wing, to_room, created_at, note \
         FROM tunnels"
    );

    if let Ok(mut tunnel_stmt) = tunnel_result {
        let mut tunnel_count = 0usize;
        let rows = tunnel_stmt.query_map([], |row| {
            Ok(TunnelRecord {
                tunnel_id: row.get(0)?,
                from_wing: row.get(1)?,
                from_room: row.get(2)?,
                to_wing: row.get(3)?,
                to_room: row.get(4)?,
                created_at: row.get(5)?,
                note: row.get(6)?,
            })
        })?;

        for row in rows {
            let tunnel = row?;
            store.create_tunnel(&tunnel)?;
            tunnel_count += 1;
        }
        println!("  Tunnels migrated: {tunnel_count}");
    }

    println!("Migration complete.");
    Ok(())
}
