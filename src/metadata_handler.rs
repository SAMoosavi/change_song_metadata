use lofty::{
    config::WriteOptions,
    file::{AudioFile, TaggedFile, TaggedFileExt},
    tag::Accessor,
};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use regex::Regex;
use zip::{read::ZipArchive, result::ZipError};

use std::{
    error::Error,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{
    cli::{Change, Conf},
    utilities::*,
};

trait MyStr {
    fn clean(&self) -> String;
}

impl MyStr for &str {
    fn clean(&self) -> String {
        self.replace('.', "-")
            .replace("128", "")
            .replace("192", "")
            .replace("320", "")
            .replace("()", "")
            .replace("[]", "")
            .trim()
            .to_lowercase()
    }
}
enum ArchiveType {
    Zip,
    Unsupported,
}
impl ArchiveType {
    fn from_str(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "zip" => Self::Zip,
            _ => Self::Unsupported,
        }
    }
}
fn archive_type(path: &Path) -> ArchiveType {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map_or(ArchiveType::Unsupported, |ext| ArchiveType::from_str(ext))
}

fn is_archive_file(path: &Path) -> bool {
    const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "rar", "tar", "7z", "gz"];

    path.extension().and_then(|ext| ext.to_str()).map_or(false, |ext| {
        ARCHIVE_EXTENSIONS.iter().any(|&x| ext.eq_ignore_ascii_case(x))
    })
}
const DEFAULT_ALBUM_NAME: &str = "single songs";

pub fn change_metadata(conf: &Conf) -> Result<(), Box<dyn Error>> {
    let path = &conf.file_path;

    if path.is_dir() {
        handle_dir(conf)
    } else if path.is_file() {
        if is_archive_file(path) {
            match archive_type(path) {
                ArchiveType::Zip => zip_handler(conf).and_then(|new_dir| {
                    let new_conf = conf.copy_from_file_path(new_dir);
                    handle_dir(&new_conf)
                }),
                ArchiveType::Unsupported => {
                    println!(
                        "⚠️  The file '{}' is an archive, but its format is unsupported. Please ensure it's a valid archive file.",
                        path.display()
                    );

                    Ok(())
                },
            }
        } else if is_audio_file(path) {
            song_handler(conf)
        } else if conf.remove_other_file {
            fs::remove_file(path).map_err(|e| e.into())
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}

fn handle_dir(conf: &Conf) -> Result<(), Box<dyn Error>> {
    fs::read_dir(&conf.file_path)?
        .par_bridge()
        .filter_map(Result::ok)
        .for_each(|entry| {
            let path = entry.path();
            let _ = change_metadata(&conf.copy_from_file_path(path));
        });
    Ok(())
}

fn move_directory_contents(src: &Path, dst: &Path) -> Result<(), Box<dyn Error>> {
    if !src.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            format!("Source path is not a directory: {}", src.display()),
        )
        .into());
    }

    create_dir_if_not_exists(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let new_path = dst.join(path.file_name().unwrap());
            fs::rename(&path, &new_path)?;
        } else if path.is_dir() {
            move_directory_contents(&path, dst)?;
            fs::remove_dir(path)?;
        }
    }
    Ok(())
}

fn zip_handler(conf: &Conf) -> Result<PathBuf, Box<dyn Error>> {
    let path = &conf.file_path;

    let name = path
        .file_stem()
        .and_then(|t| t.to_str())
        .ok_or(ZipError::FileNotFound)?;

    let parts: Vec<_> = name.split('-').map(|s| s.trim().to_lowercase()).collect();

    let (artist_name, album_name) = match parts.as_slice() {
        [artist] => (artist.as_str(), DEFAULT_ALBUM_NAME),
        [artist, album, ..] => (artist.as_str(), album.as_str()),
        _ => return Err(ZipError::FileNotFound.into()),
    };

    let artist_dir = path.parent().ok_or(ZipError::FileNotFound)?.join(artist_name);

    let album_dir = artist_dir.join(album_name);
    create_dir_if_not_exists(&artist_dir).and_then(|_| create_dir_if_not_exists(&album_dir))?;

    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    archive.extract(&album_dir)?;

    move_directory_contents(&album_dir, &album_dir)?;

    if conf.remove_zip_file {
        fs::remove_file(path)?;
    }

    Ok(album_dir)
}

fn song_handler(conf: &Conf) -> Result<(), Box<dyn Error>> {
    let parent = conf.file_path.parent().ok_or("File has no parent directory")?;

    let mut tagged_file = lofty::read_from_path(&conf.file_path).or_else(|_| {
        let file_path = &conf.file_path;
        let tmp_file = &conf.file_path.with_extension("fix.mp3");

        Command::new("ffmpeg")
            .args([
                "-i",
                file_path.to_str().unwrap_or_default(),
                "-codec",
                "copy",
                &tmp_file.to_str().unwrap_or_default(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        fs::rename(&tmp_file, file_path)?;

        lofty::read_from_path(file_path)
    })?;

    process_tags(conf, &mut tagged_file, parent)?;

    save_and_move_file(conf, tagged_file, parent)
}

fn get_name_and_track_number(filename: &str, artist_name: &str) -> (Option<u32>, String) {
    let filename = filename.replace(artist_name, "").replace('-', " ").trim().to_string();

    // Pattern 1: 01) my song
    static RE1: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?x)(?P<track>\d{2})[^a-zA-Z]*(?P<name>[a-zA-Z0-9\(\)\[\],\s]+)").unwrap());
    if let Some(captures) = RE1.captures(&filename) {
        let track = captures.name("track").map(|m| m.as_str().parse().unwrap());
        let name = captures.name("name").unwrap().as_str().trim().to_string();
        return (track, name);
    }

    // Pattern 2: some text - name
    static RE2: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?x)[a-zA-Z\& ]*-(?P<name>[a-zA-Z0-9 ]*)").unwrap());
    if let Some(captures) = RE2.captures(&filename) {
        let name = captures.name("name").unwrap().as_str().trim().to_string();
        return (None, name);
    }

    // Pattern 3: some_text_here (replace underscores and handle a specific word)
    let replaced = filename.replace('_', " ");

    let final_string = if replaced.contains(artist_name) {
        replaced.replace(artist_name, "").trim().to_string()
    } else {
        replaced.trim().to_string()
    };

    (None, final_string)
}

fn process_tags(conf: &Conf, tagged_file: &mut TaggedFile, parent: &Path) -> Result<(), Box<dyn Error>> {
    let dir_components: Vec<_> = parent.iter().rev().filter_map(|s| s.to_str()).collect();

    let artist_name = dir_components.get(1).map_or("unknown".to_string(), |s| s.clean());

    let tag = tagged_file.primary_tag_mut().ok_or("No primary tag found")?;

    match &conf.album {
        Change::Disable => {},
        Change::Auto => {
            let album_name = dir_components
                .get(0)
                .map_or(DEFAULT_ALBUM_NAME.to_string(), |s| s.clean());

            let album_name = if album_name == DEFAULT_ALBUM_NAME {
                format!("{DEFAULT_ALBUM_NAME} - {artist_name}")
            } else {
                album_name
            };
            tag.set_album(album_name);
        },
        Change::Default(s) => tag.set_album(s.to_string()),
    }
    match &conf.artist {
        Change::Disable => {},
        Change::Auto => tag.set_artist(artist_name.to_string()),
        Change::Default(s) => tag.set_artist(s.to_string()),
    }
    match &conf.title {
        Change::Disable => {},
        Change::Auto => {
            let title = conf
                .file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.clean())
                .unwrap();

            let (track, title) = get_name_and_track_number(&title, &artist_name);

            track.inspect(|track| tag.set_track(*track));
            tag.set_title(title.split_whitespace().collect::<Vec<&str>>().join(" "))
        },
        Change::Default(s) => tag.set_artist(s.to_string()),
    }
    tag.remove_comment();

    Ok(())
}

fn save_and_move_file(conf: &Conf, tagged_file: TaggedFile, parent: &Path) -> Result<(), Box<dyn Error>> {
    match tagged_file.save_to_path(&conf.file_path, WriteOptions::default()) {
        Ok(_) => {
            println!("✅ Successfully updated metadata for: {}", conf);

            if conf.move_to_parent {
                let new_parent = parent.parent().ok_or("No grandparent directory")?;

                let new_path = new_parent.join(conf.file_path.file_name().ok_or("Invalid filename")?);

                fs::rename(&conf.file_path, new_path)?;
            }

            Ok(())
        },
        Err(_) => {
            let file_path = &conf.file_path;
            let tmp_file = &conf.file_path.with_extension("fix.mp3");

            Command::new("ffmpeg")
                .args([
                    "-i",
                    file_path.to_str().unwrap_or_default(),
                    "-map_metadata",
                    "-1",
                    "-c:a",
                    "copy",
                    &tmp_file.to_str().unwrap_or_default(),
                    "-y",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;

            fs::rename(tmp_file, file_path)?;

            song_handler(conf)
        },
    }
}
