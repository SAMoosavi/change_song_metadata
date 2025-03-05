use lofty::{error, file::TaggedFileExt, tag::Accessor};
use rayon::prelude::*;

use std::{error::Error, fs, io};

use crate::{cli::Conf, utilities::*};

pub fn organize_file(conf: &Conf) -> Result<(), Box<dyn Error>> {
    organize_media_files(conf)
}

fn organize_media_files(conf: &Conf) -> Result<(), Box<dyn Error>> {
    let path = &conf.file_path;

    match (path.is_dir(), path.is_file() && is_audio_file(path)) {
        (true, _) => process_media_directory(conf)
            .map_err(|e| format!("Failed to process directory '{}': {}", path.display(), e).into()),
        (_, true) => process_single_audio_file(conf)
            .map_err(|e| format!("Failed to process audio file '{}': {}", path.display(), e).into()),
        _ => {
            let msg = format!(
                "Path '{}' is neither a directory nor supported audio file",
                path.display()
            );
            Err(msg.into())
        },
    }
}

fn process_media_directory(conf: &Conf) -> Result<(), Box<dyn Error>> {
    fs::read_dir(&conf.file_path)?
        .par_bridge()
        .filter_map(Result::ok)
        .for_each(|entry| {
            if let Err(e) = organize_media_files(&conf.copy_from_file_path(entry.path())) {
                println!("{}", e);
            }
        });
    Ok(())
}

fn process_single_audio_file(conf: &Conf) -> Result<(), Box<dyn Error>> {
    const DEFAULT_ALBUM_NAME: &str = "single songs";

    let parent = conf.file_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("File has no parent directory: {}", conf),
        )
    })?;

    let mut tagged_file = lofty::read_from_path(&conf.file_path)?;
    match tagged_file.primary_tag_mut() {
        Some(tag) => {
            let album_name = match tag.album() {
                Some(x) => x.to_string(),
                None => DEFAULT_ALBUM_NAME.to_string(),
            };
            let dir_path = parent.join(album_name);
            create_dir_if_not_exists(&dir_path)?;
            fs::rename(&conf.file_path, dir_path.join(conf.file_path.file_name().unwrap()))?;
            Ok(())
        },
        None => Err(error::LoftyError::new(error::ErrorKind::UnsupportedTag).into()),
    }
}
