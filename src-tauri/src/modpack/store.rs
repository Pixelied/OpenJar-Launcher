use crate::modpack::types::{InstanceModpackLinkState, LockSnapshot, ModpackSpec, ModpackStoreV1, ResolutionPlan};
use std::fs;
use std::path::{Path, PathBuf};

const STORE_FILE: &str = "store.v1.json";
const STORE_DIR: &str = "modpack_maker";
const MAX_PLANS: usize = 250;
const MAX_LOCK_SNAPSHOTS: usize = 250;

pub fn store_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path_resolver()
        .app_data_dir()
        .ok_or_else(|| "cannot resolve app data dir".to_string())?;
    Ok(base.join(STORE_DIR).join(STORE_FILE))
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir modpack store dir failed: {e}"))?;
    }
    Ok(())
}

pub fn read_store(app: &tauri::AppHandle) -> Result<ModpackStoreV1, String> {
    let path = store_path(app)?;
    if !path.exists() {
        return Ok(ModpackStoreV1::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("read modpack store failed: {e}"))?;
    let mut store: ModpackStoreV1 =
        serde_json::from_str(&raw).map_err(|e| format!("parse modpack store failed: {e}"))?;
    if store.version == 0 {
        store.version = 1;
    }
    Ok(store)
}

pub fn write_store(app: &tauri::AppHandle, store: &ModpackStoreV1) -> Result<(), String> {
    let path = store_path(app)?;
    ensure_parent(&path)?;
    let mut next = store.clone();
    next.version = 1;

    // Keep store bounded.
    if next.plans.len() > MAX_PLANS {
        let drop_count = next.plans.len().saturating_sub(MAX_PLANS);
        next.plans.drain(0..drop_count);
    }
    if next.lock_snapshots.len() > MAX_LOCK_SNAPSHOTS {
        let drop_count = next.lock_snapshots.len().saturating_sub(MAX_LOCK_SNAPSHOTS);
        next.lock_snapshots.drain(0..drop_count);
    }

    let raw = serde_json::to_string_pretty(&next)
        .map_err(|e| format!("serialize modpack store failed: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("write modpack store failed: {e}"))
}

pub fn upsert_spec(store: &mut ModpackStoreV1, spec: ModpackSpec) {
    if let Some(found) = store.specs.iter_mut().find(|s| s.id == spec.id) {
        *found = spec;
    } else {
        store.specs.push(spec);
    }
    store
        .specs
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
}

pub fn remove_spec(store: &mut ModpackStoreV1, spec_id: &str) {
    store.specs.retain(|s| s.id != spec_id);
    store.plans.retain(|p| p.modpack_id != spec_id);
    store
        .instance_links
        .retain(|l| l.modpack_id != spec_id);
}

pub fn get_spec(store: &ModpackStoreV1, spec_id: &str) -> Option<ModpackSpec> {
    store.specs.iter().find(|s| s.id == spec_id).cloned()
}

pub fn add_plan(store: &mut ModpackStoreV1, plan: ResolutionPlan) {
    store.plans.push(plan);
}

pub fn get_plan(store: &ModpackStoreV1, plan_id: &str) -> Option<ResolutionPlan> {
    store.plans.iter().find(|p| p.id == plan_id).cloned()
}

pub fn set_instance_link(store: &mut ModpackStoreV1, link: InstanceModpackLinkState) {
    if let Some(found) = store
        .instance_links
        .iter_mut()
        .find(|l| l.instance_id == link.instance_id)
    {
        *found = link;
    } else {
        store.instance_links.push(link);
    }
}

pub fn get_instance_link(store: &ModpackStoreV1, instance_id: &str) -> Option<InstanceModpackLinkState> {
    store
        .instance_links
        .iter()
        .find(|l| l.instance_id == instance_id)
        .cloned()
}

pub fn add_lock_snapshot(store: &mut ModpackStoreV1, snapshot: LockSnapshot) {
    store.lock_snapshots.push(snapshot);
}

pub fn get_lock_snapshot(store: &ModpackStoreV1, id: &str) -> Option<LockSnapshot> {
    store.lock_snapshots.iter().find(|s| s.id == id).cloned()
}
