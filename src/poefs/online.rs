use crate::bundle::Bundle;

use super::FileSource;

pub struct OnlineSource;

impl FileSource for OnlineSource {
    fn get_file(&mut self, _path: &str) -> Result<Option<(Bundle, Vec<u8>)>, anyhow::Error> {
        todo!()
    }
}
