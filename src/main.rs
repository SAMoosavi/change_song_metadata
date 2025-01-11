use lofty::{
    config::WriteOptions,
    error::Result,
    file::{AudioFile, TaggedFileExt},
    tag::{Accessor, Tag},
};
use std::{fs, path::Path};

fn change_metadata(tag: &mut Tag) {
    tag.set_title("Khabe Baroon".to_string());
    tag.set_artist("Siavash Ghomayshi".to_string());
    tag.set_album("Single Song".to_string());
}

fn main() -> Result<()> {
    let base_path = "/home/moosavi/Downloads/";

    if Path::new(base_path).is_dir() {
        for entry in fs::read_dir(base_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let mut tagged_file = lofty::read_from_path(&path)?;
                if let Some(tag) = tagged_file.primary_tag_mut() {
                    change_metadata(tag);
                }

                tagged_file.save_to_path(&path, WriteOptions::default())?;
            }
        }
    } else {
        let mut tagged_file = lofty::read_from_path(base_path)?;

        if let Some(tag) = tagged_file.primary_tag_mut() {
            change_metadata(tag);
        }

        tagged_file.save_to_path(&base_path, WriteOptions::default())?;
    }
    println!("Metadata updated successfully!");
    Ok(())
}
