use crate::error::MpError;
use crate::storage::{PalaceStore, TunnelRecord};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};
use sha2::{Sha256, Digest};
use chrono::Utc;

pub struct PalaceGraph;

impl PalaceGraph {
    pub fn graph_stats(store: &PalaceStore) -> Result<Value, MpError> {
        let wings = store.list_wings()?;
        let mut total_rooms = 0usize;
        for w in &wings {
            total_rooms += store.list_rooms(w)?.len();
        }
        let tunnels = store.list_tunnels(None)?;
        Ok(serde_json::json!({
            "total_wings":   wings.len(),
            "total_rooms":   total_rooms,
            "total_tunnels": tunnels.len(),
            "wings":         wings,
        }))
    }

    pub fn traverse(
        store: &PalaceStore,
        start_wing: &str,
        start_room: &str,
        max_hops: usize,
    ) -> Result<Value, MpError> {
        let max_hops = max_hops.clamp(1, 10);
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(String, String, usize)> = VecDeque::new();
        let mut rooms_data: Vec<Value> = Vec::new();

        queue.push_back((start_wing.to_string(), start_room.to_string(), 0));

        while let Some((wing, room, hops)) = queue.pop_front() {
            let key = format!("{wing}/{room}");
            if visited.contains(&key) || hops > max_hops { continue; }
            visited.insert(key);

            let drawers = store.list_drawers(Some(&wing), Some(&room), Some(20))?;
            rooms_data.push(serde_json::json!({
                "wing": wing, "room": room, "hop": hops,
                "drawer_count": drawers.len(),
            }));

            if hops < max_hops {
                for t in store.list_tunnels(Some(&wing))? {
                    if t.from_wing == wing && t.from_room == room {
                        queue.push_back((t.to_wing, t.to_room, hops + 1));
                    } else if t.to_wing == wing && t.to_room == room {
                        queue.push_back((t.from_wing, t.from_room, hops + 1));
                    }
                }
            }
        }

        Ok(serde_json::json!({
            "start_wing": start_wing, "start_room": start_room,
            "max_hops": max_hops,
            "rooms_visited": rooms_data,
            "total_visited": rooms_data.len(),
        }))
    }

    pub fn find_tunnels(store: &PalaceStore) -> Result<Value, MpError> {
        let tunnels = store.list_tunnels(None)?;
        let total = tunnels.len();
        let items: Vec<Value> = tunnels.iter().map(|t| serde_json::json!({
            "tunnel_id": t.tunnel_id,
            "from": format!("{}/{}", t.from_wing, t.from_room),
            "to":   format!("{}/{}", t.to_wing,   t.to_room),
            "note": t.note,
        })).collect();
        Ok(serde_json::json!({ "tunnels": items, "total": total }))
    }

    pub fn create_tunnel(
        store: &PalaceStore,
        from_wing: &str,
        from_room: &str,
        to_wing: &str,
        to_room: &str,
        note: Option<&str>,
    ) -> Result<String, MpError> {
        let mut h = Sha256::new();
        h.update(from_wing.as_bytes());
        h.update(from_room.as_bytes());
        h.update(to_wing.as_bytes());
        h.update(to_room.as_bytes());
        let tunnel_id = hex::encode(h.finalize())[..16].to_string();

        store.create_tunnel(&TunnelRecord {
            tunnel_id: tunnel_id.clone(),
            from_wing:  from_wing.to_string(),
            from_room:  from_room.to_string(),
            to_wing:    to_wing.to_string(),
            to_room:    to_room.to_string(),
            created_at: Utc::now().timestamp_millis() as f64 / 1000.0,
            note:       note.map(|s| s.to_string()),
        })?;
        Ok(tunnel_id)
    }

    pub fn delete_tunnel(store: &PalaceStore, tunnel_id: &str) -> Result<bool, MpError> {
        store.delete_tunnel(tunnel_id)
    }

    pub fn follow_tunnels(store: &PalaceStore, wing: &str, room: &str) -> Result<Value, MpError> {
        let tunnels = store.list_tunnels(Some(wing))?;
        let connected: Vec<Value> = tunnels.iter()
            .filter(|t| {
                (t.from_wing == wing && t.from_room == room)
                || (t.to_wing == wing && t.to_room == room)
            })
            .map(|t| {
                let (dw, dr) = if t.from_wing == wing && t.from_room == room {
                    (&t.to_wing, &t.to_room)
                } else {
                    (&t.from_wing, &t.from_room)
                };
                serde_json::json!({
                    "tunnel_id": t.tunnel_id,
                    "dest_wing": dw, "dest_room": dr,
                    "note": t.note,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "wing": wing, "room": room,
            "connections": connected,
            "total": connected.len(),
        }))
    }
}
