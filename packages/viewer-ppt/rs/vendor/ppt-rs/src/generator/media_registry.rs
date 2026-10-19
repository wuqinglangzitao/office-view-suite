//! Deduplicate embedded slide media by byte content.

/// Assigns stable 1-based `imageN` part numbers, reusing prior entries for identical bytes.
///
/// Also stores non-image media (video/audio) bytes for `mediaN` parts in a
/// separate numbering band so image part numbering is never disturbed.
#[derive(Default)]
pub struct MediaRegistry {
    entries: Vec<(Vec<u8>, String)>,
    media_entries: Vec<(Vec<u8>, String)>,
}

impl MediaRegistry {
    pub fn image_number(&mut self, bytes: &[u8], ext: &str) -> usize {
        if let Some(num) = self.lookup_number(bytes, ext) {
            return num;
        }
        self.entries.push((bytes.to_vec(), ext.to_string()));
        self.entries.len()
    }

    pub fn lookup_number(&self, bytes: &[u8], ext: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|(existing, existing_ext)| {
                existing.as_slice() == bytes && existing_ext == ext
            })
            .map(|i| i + 1)
    }

    pub fn files(&self) -> &[(Vec<u8>, String)] {
        &self.entries
    }

    pub fn extensions(&self) -> Vec<String> {
        self.entries.iter().map(|(_, ext)| ext.clone()).collect()
    }

    /// Register non-image media bytes (video/audio); returns the 1-based
    /// `mediaN` part number, deduplicating identical bytes.
    pub fn media_number(&mut self, bytes: &[u8], ext: &str) -> usize {
        if let Some(num) = self.media_lookup_number(bytes, ext) {
            return num;
        }
        self.media_entries.push((bytes.to_vec(), ext.to_string()));
        self.media_entries.len()
    }

    pub fn media_lookup_number(&self, bytes: &[u8], ext: &str) -> Option<usize> {
        self.media_entries
            .iter()
            .position(|(existing, existing_ext)| {
                existing.as_slice() == bytes && existing_ext == ext
            })
            .map(|i| i + 1)
    }

    pub fn media_files(&self) -> &[(Vec<u8>, String)] {
        &self.media_entries
    }

    /// Deduplicated extensions of registered non-image media.
    pub fn media_extensions(&self) -> Vec<String> {
        let mut exts: Vec<String> = Vec::new();
        for (_, ext) in &self.media_entries {
            if !exts.iter().any(|e| e == ext) {
                exts.push(ext.clone());
            }
        }
        exts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_identical_image_bytes() {
        let mut registry = MediaRegistry::default();
        let a = registry.image_number(b"same", "png");
        let b = registry.image_number(b"other", "jpg");
        let c = registry.image_number(b"same", "png");
        assert_eq!(a, 1);
        assert_eq!(b, 2);
        assert_eq!(c, 1);
        assert_eq!(registry.files().len(), 2);
    }

    #[test]
    fn media_band_is_independent_of_images() {
        let mut registry = MediaRegistry::default();
        registry.image_number(b"img", "png");
        let m1 = registry.media_number(b"video-bytes", "mp4");
        let m2 = registry.media_number(b"audio-bytes", "mp3");
        let m3 = registry.media_number(b"video-bytes", "mp4");
        assert_eq!((m1, m2, m3), (1, 2, 1));
        assert_eq!(registry.media_files().len(), 2);
        assert_eq!(
            registry.media_extensions(),
            vec!["mp4".to_string(), "mp3".to_string()]
        );
        // Image numbering untouched
        assert_eq!(registry.lookup_number(b"img", "png"), Some(1));
    }
}
