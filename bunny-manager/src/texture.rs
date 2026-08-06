use std::{path::Path, sync::Arc};

use anyhow::{Context as _, Result};
use egui::{ColorImage, Context, SizeHint, load::ImagePoll};
use tracing::{debug, error};

#[derive(Default, Debug)]
pub struct TextureLoader {
    entries: Vec<LoadEntry>,
}

impl TextureLoader {
    pub fn new(textures_path: impl AsRef<Path>) -> Result<Self> {
        let path = textures_path.as_ref();
        let dir = path
            .read_dir()
            .with_context(|| format!("Failed to read plugin dir at {}", path.display()))?;
        let entries: Vec<LoadEntry> = dir
            .filter_map(|entry| {
                entry
                    .map_err(|e| error!("Error reading directory entry: {e:#}"))
                    .ok()
            })
            .filter(|entry| entry.file_type().is_ok_and(|f| f.is_file()))
            .filter_map(|entry| {
                let path = entry.path();
                path.canonicalize()
                    .map_err(|e| {
                        error!(
                            "Failed to convert path at {} to absolute: {e:#}",
                            path.display()
                        )
                    })
                    .map(|absolute_path| {
                        (
                            entry.file_name().to_string_lossy().to_string(),
                            absolute_path,
                        )
                    })
                    .ok()
            })
            .map(|(file_name, absolute_path)| {
                let mut uri = String::from("file://");
                uri.push_str(&absolute_path.to_string_lossy());
                LoadEntry {
                    uri,
                    file_name,
                    data: None,
                    progress: LoadProgress::Pending,
                }
            })
            .collect();
        Ok(Self { entries })
    }

    pub fn load_all(&mut self, ctx: &Context) -> Option<Vec<LoadResult>> {
        self.entries
            .iter_mut()
            .all(|entry| {
                if entry.pending() {
                    entry.load(ctx);
                }
                !entry.pending()
            })
            .then(|| self.collect())
    }

    pub fn collect(&mut self) -> Vec<LoadResult> {
        self.entries
            .iter_mut()
            .filter_map(|entry| {
                entry.data.take().map(|data| {
                    debug!("Loaded texture {}", &entry.file_name);
                    LoadResult {
                        file_name: std::mem::take(&mut entry.file_name),
                        data,
                    }
                })
            })
            .collect()
    }
}

#[derive(Debug)]
struct LoadEntry {
    uri: String,
    file_name: String,
    data: Option<Arc<ColorImage>>,
    progress: LoadProgress,
}

impl LoadEntry {
    #[inline]
    fn pending(&self) -> bool {
        self.progress == LoadProgress::Pending
    }

    fn load(&mut self, ctx: &Context) {
        match ctx.try_load_image(&self.uri, SizeHint::default()) {
            Ok(poll) => {
                if let ImagePoll::Ready { image } = poll {
                    self.data = Some(image.clone());
                    self.progress = LoadProgress::Done;
                }
            }
            Err(e) => {
                error!("Image load error for uri {}: {e:#}", &self.uri);
                self.progress = LoadProgress::Error;
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum LoadProgress {
    Pending,
    Done,
    Error,
}

pub struct LoadResult {
    pub file_name: String,
    pub data: Arc<ColorImage>,
}
