use crate::utils::{shared_http_client, try_download_file, LauncherError};
use crate::Launcher;
use sha1::Digest;
use std::error::Error;
use std::path::PathBuf;
use tokio::fs;
use tokio::task::JoinSet;

#[derive(Clone)]
struct AssetDownloadTask {
    name: String,
    hash: String,
    size: u64,
    object_path: PathBuf,
    object_url: String,
}

impl Launcher {
    /// Install assets for the current version
    pub async fn install_assets(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.version.profile.is_null() {
            return Err(Box::from(LauncherError(
                "Please install a version before installing assets".to_string(),
            )));
        }

        self.emit_progress("checking_assets", "", 0, 0);

        let assets_dir = self.game_dir.join("assets");
        let indexes_dir = assets_dir.join("indexes");
        let objects_dir = assets_dir.join("objects");

        fs::create_dir_all(&indexes_dir).await?;
        fs::create_dir_all(&objects_dir).await?;

        self.fix_log4j_vulnerability().await?;

        let index_path = indexes_dir.join(&format!(
            "{}.json",
            self.version.profile["assets"].as_str().unwrap()
        ));

        if !index_path.exists() {
            let index_url = self.version.profile["assetIndex"]["url"].as_str().unwrap();
            let index_data = shared_http_client()
                .get(index_url)
                .send()
                .await?
                .error_for_status()?
                .text()
                .await?;
            fs::write(&index_path, index_data).await?;
        }

        let index: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path).await?)?;

        let mut readdir = fs::read_dir(&objects_dir).await?;
        while let Some(file) = readdir.next_entry().await? {
            let path = file.path();
            if path.is_file() {
                let hash = path.file_name().unwrap().to_str().unwrap().to_string();

                if !index["objects"]
                    .as_object()
                    .unwrap()
                    .values()
                    .any(|object| object["hash"].as_str().unwrap() == &hash)
                    || format!("{:x}", sha1::Sha1::digest(&fs::read(&path).await?)) != hash
                {
                    fs::remove_file(&path).await?;
                }
            }
        }

        let mut total: u64 = 0;
        let mut current: u64 = 0;
        let mut objects_to_download: Vec<AssetDownloadTask> = vec![];

        for (name, object) in index["objects"].as_object().unwrap() {
            let object = object.as_object().unwrap();
            let hash = object["hash"].as_str().unwrap().to_string();

            let object_path = objects_dir.join(&hash[..2]).join(&hash);

            if !object_path.exists() {
                let size = object["size"].as_u64().unwrap_or(0);
                total += size;
                objects_to_download.push(AssetDownloadTask {
                    name: name.to_string(),
                    hash: hash.clone(),
                    size,
                    object_path: object_path.clone(),
                    object_url: format!(
                        "https://resources.download.minecraft.net/{}",
                        hash[..2].to_string() + "/" + &hash
                    ),
                });
            }
        }

        if !objects_to_download.is_empty() {
            self.emit_progress("downloading_assets", "", total, 0);
        }

        let legacy_assets = self.version.profile["assets"].as_str().unwrap() == "legacy"
            || self.version.profile["assets"].as_str().unwrap() == "pre-1.6";
        let resources_root = self.game_dir.join("resources");
        let mut cursor = 0usize;
        let concurrency = 16usize;
        while cursor < objects_to_download.len() {
            let end = (cursor + concurrency).min(objects_to_download.len());
            let mut set = JoinSet::new();
            for task in objects_to_download[cursor..end].iter().cloned() {
                set.spawn(async move {
                    if let Some(parent) = task.object_path.parent() {
                        fs::create_dir_all(parent).await?;
                    }
                    try_download_file(&task.object_url, &task.object_path, &task.hash, 3).await?;
                    Ok::<AssetDownloadTask, Box<dyn Error + Send + Sync>>(task)
                });
            }
            while let Some(joined) = set.join_next().await {
                let task = joined.map_err(|e| {
                    Box::new(LauncherError(format!("Asset download worker failed: {e}")))
                        as Box<dyn Error + Send + Sync>
                })??;
                current += task.size;
                self.emit_progress("downloading_assets", &task.name, total, current);
                if legacy_assets {
                    let resources_path = resources_root.join(&task.name);
                    if let Some(parent) = resources_path.parent() {
                        fs::create_dir_all(parent).await?;
                    }
                    fs::copy(&task.object_path, &resources_path).await?;
                }
            }
            cursor = end;
        }

        Ok(())
    }

    async fn fix_log4j_vulnerability(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Fix log4j vulnerability
        if self.version.profile["logging"].is_object()
            && self.version.profile["logging"]["client"].is_object()
        {
            if (self.version.id.split('.').collect::<Vec<&str>>()[1] == "18"
                && self.version.id.split('.').collect::<Vec<&str>>().len() == 3)
                || self.version.id.split('.').collect::<Vec<&str>>()[1]
                    .parse::<u32>()
                    .unwrap()
                    > 18
            {
                return Ok(());
            }

            let log4j_path = self.game_dir.join("assets").join("log_configs").join(
                self.version.profile["logging"]["client"]["file"]["id"]
                    .as_str()
                    .unwrap(),
            );

            if !log4j_path.exists() {
                let log4j_url = self.version.profile["logging"]["client"]["file"]["url"]
                    .as_str()
                    .unwrap()
                    .to_string();
                let log4j = shared_http_client()
                    .get(&log4j_url)
                    .send()
                    .await?
                    .error_for_status()?
                    .bytes()
                    .await?;
                fs::create_dir_all(log4j_path.parent().unwrap()).await?;
                fs::write(&log4j_path, log4j).await?;
            }

            let log4j_arg = self.version.profile["logging"]["client"]["argument"]
                .as_str()
                .unwrap()
                .replace("${path}", log4j_path.to_str().unwrap());
            self.args.push(log4j_arg);

            if self.version.id.split('.').collect::<Vec<&str>>()[1] == "18"
                && self.version.id.split('.').collect::<Vec<&str>>().len() == 2
                || self.version.id.split('.').collect::<Vec<&str>>()[1] == "17"
            {
                self.args
                    .push("-Dlog4j2.formatMsgNoLookups=true".to_string());
            }
        }

        Ok(())
    }
}
