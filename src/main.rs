use lofty::{
    config::WriteOptions,
    error,
    file::{AudioFile, TaggedFileExt},
    tag::Accessor,
};
use rayon::prelude::*;
use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};
use zip::{read::ZipArchive, result::ZipResult};

#[derive(Clone)]
enum Change {
    Disable,
    Auto,
    Default(String),
}

struct Conf {
    file_path: PathBuf,
    artist: Change,
    album: Change,
    remove_other_file: bool,
    remove_zip_file: bool,
    move_to_parent: bool,
}

impl Conf {
    fn copy_from_file_path(&self, file_path: PathBuf) -> Self {
        Self {
            file_path,
            artist: self.artist.clone(),
            album: self.album.clone(),
            remove_other_file: self.remove_other_file,
            remove_zip_file: self.remove_zip_file,
            move_to_parent: self.move_to_parent,
        }
    }
    fn display(&self) -> std::path::Display<'_> {
        self.file_path.display()
    }
}

trait MyStr {
    fn clear_str(&mut self) -> String;
}

impl MyStr for &str {
    fn clear_str(&mut self) -> String {
        self.replace("_", "-")
            .replace(".", "-")
            .replace("128", "")
            .replace("320", "")
            .replace("()", "")
            .replace("[]", "")
            .to_lowercase()
    }
}

fn is_zip_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

fn is_song(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("mp3"))
        .unwrap_or(false)
}

fn create_dir_if_not_exists(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)
    } else {
        Ok(())
    }
}

fn move_to(src: &Path, dst: &Path) -> ZipResult<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let new_path = dst.join(path.file_name().unwrap());
            fs::rename(&path, &new_path)?;
        } else if path.is_dir() {
            move_to(&path, dst)?;
            fs::remove_dir(path)?;
        }
    }
    Ok(())
}

fn zip_handler(conf: &Conf) -> ZipResult<PathBuf> {
    let path = &conf.file_path;

    let name = path.file_stem().and_then(|t| t.to_str()).unwrap();
    let album_name: Vec<_> = name.split('-').map(|s| s.trim()).collect();
    let singer_dir = path.parent().unwrap().join(album_name[0]);
    create_dir_if_not_exists(&singer_dir)?;
    let album_dir = singer_dir.join(album_name[1]);
    create_dir_if_not_exists(&album_dir)?;

    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    archive.extract(&album_dir)?;
    move_to(&album_dir, &album_dir)?;

    if conf.remove_zip_file {
        fs::remove_file(path).unwrap();
    }

    Ok(album_dir)
}

fn song_handler(conf: &Conf) -> error::Result<()> {
    let parent = conf.file_path.parent().unwrap();
    let dir: Vec<_> = parent.iter().rev().collect();
    let album_name = dir[0].to_str().unwrap().clear_str();

    let artist_name = dir[1].to_str().unwrap().clear_str();

    let title_name = conf
        .file_path
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .clear_str()
        .split('-')
        .map(|s| s.trim())
        .filter(|f| f != &"" && f != &artist_name)
        .collect::<Vec<_>>()
        .join(" ");

    let mut tagged_file = lofty::read_from_path(&conf.file_path)?;
    if let Some(tag) = tagged_file.primary_tag_mut() {
        match &conf.album {
            Change::Disable => {}
            Change::Auto => tag.set_album(album_name),
            Change::Default(s) => tag.set_album(s.to_string()),
        }
        match &conf.artist {
            Change::Disable => {}
            Change::Auto => tag.set_artist(artist_name),
            Change::Default(s) => tag.set_artist(s.to_string()),
        }
        tag.set_title(title_name);
        tag.remove_comment();
    }

    tagged_file.save_to_path(&conf.file_path, WriteOptions::default())?;

    if conf.move_to_parent {
        fs::rename(
            &conf.file_path,
            parent
                .parent()
                .unwrap()
                .join(conf.file_path.file_name().unwrap()),
        )?;
    }

    Ok(())
}

fn dir_handler(conf: &Conf) -> io::Result<()> {
    let entries: Vec<_> = fs::read_dir(&conf.file_path)?
        .filter_map(Result::ok)
        .collect();

    entries.par_iter().for_each(|entry| {
        let path = entry.path();
        handler(&conf.copy_from_file_path(path))
    });
    Ok(())
}

fn handler(conf: &Conf) {
    let path = &conf.file_path;

    if path.is_dir() {
        if let Err(e) = dir_handler(conf) {
            eprintln!("Error processing directory {}: {}", path.display(), e);
        }
    } else if path.is_file() {
        if is_zip_file(path) {
            match zip_handler(conf) {
                Err(e) => eprintln!("Error extracting ZIP file {}: {}", path.display(), e),
                Ok(new_dir) => {
                    let new_conf = conf.copy_from_file_path(new_dir);
                    if let Err(e) = dir_handler(&new_conf) {
                        eprintln!("Error processing directory {}: {}", new_conf.display(), e);
                    }
                }
            }
        } else if is_song(path) {
            if let Err(e) = song_handler(conf) {
                eprintln!("Error handling song file {}: {}", path.display(), e);
            }
        } else if conf.remove_other_file {
            if let Err(e) = fs::remove_file(path) {
                eprintln!("Error deleting file {}: {}", path.display(), e);
            }
        }
    }
}

fn main() -> io::Result<()> {
    let base_path = "/home/moosavi/Downloads/";

    let conf = Conf {
        file_path: base_path.into(),
        album: Change::Auto,
        artist: Change::Auto,
        remove_other_file: true,
        remove_zip_file: true,
        move_to_parent: true,
    };

    handler(&conf);
    println!("Metadata updated successfully!");
    Ok(())
}
