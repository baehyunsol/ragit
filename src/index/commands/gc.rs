use super::Index;
use crate::chunk;
use crate::error::Error;
use crate::index::LOG_DIR_NAME;
use crate::uid::Uid;
use ragit_fs::{
    WriteMode,
    exists,
    file_name,
    parent,
    read_dir,
    remove_file,
    set_extension,
    write_string,
};
use std::collections::HashSet;

impl Index {
    /// `rag gc --logs`
    ///
    /// It returns how many files it removed.
    pub fn gc_logs(&mut self) -> Result<usize, Error> {
        let logs_at = Index::get_rag_path(
            &self.root_dir,
            &LOG_DIR_NAME.to_string(),
        )?;

        if !exists(&logs_at) {
            return Ok(0);
        }

        let mut count = 0;

        for file in read_dir(&logs_at, false)? {
            count += 1;
            remove_file(&file)?;
        }

        Ok(count)
    }

    /// `rag gc --images`
    ///
    /// It returns how many files it removed.
    pub fn gc_images(&mut self) -> Result<usize, Error> {
        let mut all_images = HashSet::new();
        let mut count = 0;

        for chunk_file in self.get_all_chunk_files()? {
            for image in chunk::load_from_file(&chunk_file)?.images {
                all_images.insert(image);
            }
        }

        for image_file in self.get_all_image_files()? {
            let uid = Uid::from_prefix_and_suffix(
                &file_name(&parent(&image_file)?)?,
                &file_name(&image_file)?,
            )?;

            if !all_images.contains(&uid) {
                remove_file(&image_file)?;
                remove_file(&set_extension(&image_file, "json")?)?;
                count += 1;
            }
        }

        if count > 0 {
            self.reset_uid(true /* save_to_file */)?;
        }

        Ok(count)
    }

    /// `rag gc --audit`
    pub fn gc_audit(&mut self) -> Result<(), Error> {
        let usages_at = Index::get_rag_path(
            &self.root_dir,
            &"usages.json".to_string(),
        )?;

        Ok(write_string(
            &usages_at,
            "{}",
            WriteMode::CreateOrTruncate,
        )?)
    }
}
