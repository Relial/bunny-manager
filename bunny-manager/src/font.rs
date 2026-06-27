use std::path::Path;

use egui::{
    FontFamily,
    epaint::text::{FontInsert, FontPriority, InsertFontFamily},
};
use tracing::{error, info};

#[derive(Debug)]
pub struct Fonts(Vec<FontData>);

impl Fonts {
    pub fn load(path: impl AsRef<Path>) -> Self {
        info!("Adding fonts");
        let mut fonts = Vec::new();
        let path = path.as_ref();
        let entries = match path.read_dir() {
            Ok(entries) => entries,
            Err(e) => {
                error!("Failed to read fonts dir at {}: {e:#}", path.display());
                return Self(fonts);
            }
        };
        for entry in entries {
            match entry {
                Ok(entry) => {
                    let entry_path = entry.path();
                    if let Some(ext) = entry_path.extension()
                        && (ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf"))
                    {
                        match std::fs::read(&entry_path) {
                            Ok(font_bytes) => {
                                if let Some(file_name) = entry_path.file_stem() {
                                    let n = file_name.to_string_lossy();
                                    fonts.push(FontData {
                                        name: n.into(),
                                        data: font_bytes,
                                    });
                                } else {
                                    error!(
                                        "Failed to extract file name from path {}",
                                        entry_path.display()
                                    );
                                }
                            }
                            Err(e) => {
                                error!(
                                    "Failed to read font file at {}: {e:#}",
                                    entry_path.display()
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading directory entry: {e:#}");
                }
            }
        }

        Self(fonts)
    }

    pub fn add_all(&self, ctx: &egui::Context) {
        for font in &self.0 {
            font.add(ctx);
        }
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|f| f.name.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct FontData {
    name: String,
    data: Vec<u8>,
}

impl FontData {
    fn add(&self, ctx: &egui::Context) {
        ctx.add_font(FontInsert::new(
            &self.name,
            egui::FontData::from_owned(self.data.clone()),
            vec![InsertFontFamily {
                family: FontFamily::Name(self.name.clone().into()),
                priority: FontPriority::Highest,
            }],
        ));
    }
}
