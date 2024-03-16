use std::io::Cursor;

use crate::bundle::Bundle;

use super::FileSource;

pub struct OnlineSource {
    patch: String,
}

impl OnlineSource {
    pub fn new(patch: Option<String>) -> Self {
        let patch = patch.unwrap_or_else(|| Self::get_latest_patch());
        Self { patch }
    }

    fn get_latest_patch() -> String {
        let response = reqwest::blocking::get(
            "https://raw.githubusercontent.com/poe-tool-dev/latest-patch-version/main/latest.txt",
        )
        .unwrap();
        response.text().unwrap()
    }
}

impl FileSource for OnlineSource {
    fn get_file(&mut self, path: &str) -> Result<Option<(Bundle, Vec<u8>)>, anyhow::Error> {
        let url = format!("https://patch.poecdn.com/{}{}", self.patch, path);
        // TODO: return Ok(None) if 404 status code
        let response = reqwest::blocking::get(url)?;
        let content = response.bytes()?;
        let mut c = Cursor::new(content);
        let bundle = Bundle::parse(&mut c)?;
        let position = c.position() as usize;
        let content = c.into_inner();
        let bytes = content.into_iter().skip(position).collect::<Vec<_>>();
        Ok(Some((bundle, bytes)))
    }
}
