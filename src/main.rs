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

fn is_zip_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("zip"))
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
            move_to(&path, &dst)?;
            fs::remove_dir(&path)?;
        }
    }
    Ok(())
}

fn extract_zip_to_dir(zip_path: &Path, dest_dir: &Path) -> ZipResult<()> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    archive.extract(dest_dir)?;
    move_to(dest_dir, dest_dir)?;
    fs::remove_file(&zip_path).unwrap();
    Ok(())
}

fn zip_handler(path: &Path) -> ZipResult<PathBuf> {
    let name = path.file_stem().and_then(|t| t.to_str()).unwrap();
    let album_name: Vec<_> = name.split('-').map(|s| s.trim()).collect();
    let singer_dir = path.parent().unwrap().join(album_name[0]);
    create_dir_if_not_exists(&singer_dir)?;
    let album_dir = singer_dir.join(album_name[1]);
    create_dir_if_not_exists(&album_dir)?;

    extract_zip_to_dir(&path, &album_dir)?;
    Ok(album_dir)
}

fn is_song(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("mp3"))
        .unwrap_or(false)
}

trait MyStr {
    fn clear_str(&mut self) -> String;
}

impl MyStr for &str {
    fn clear_str(&mut self) -> String {
        self.replace("_", "-")
            .replace(".", "-")
            .replace(" ", "-")
            .replace("128", "")
            .replace("320", "")
            .replace("()", "")
            .replace("[]", "")
            .to_lowercase()
    }
}

fn song_handler(path: &Path) -> error::Result<()> {
    let parent = path.parent().unwrap();
    let dir: Vec<_> = parent.iter().rev().collect();
    let album_name = dir[0].to_str().unwrap().clear_str();

    let artist_name = dir[1].to_str().unwrap().clear_str();

    let title_name = path
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

    let mut tagged_file = lofty::read_from_path(&path)?;
    if let Some(tag) = tagged_file.primary_tag_mut() {
        tag.set_artist(artist_name);
        tag.set_album(album_name);
        tag.set_title(title_name);
        tag.set_comment("".to_string());
    }

    tagged_file.save_to_path(&path, WriteOptions::default())?;

    fs::rename(
        &path,
        parent.parent().unwrap().join(path.file_name().unwrap()),
    )?;

    Ok(())
}

fn dir_handler(dir_path: &Path) -> io::Result<()> {
    let entries: Vec<_> = fs::read_dir(dir_path)?.filter_map(Result::ok).collect();

    entries.par_iter().for_each(|entry| {
        let path = entry.path();
        if path.is_dir() {
            if let Err(e) = dir_handler(&path) {
                eprintln!("Error processing directory {}: {}", path.display(), e);
            }
        } else if path.is_file() {
            if is_zip_file(&path) {
                match zip_handler(&path) {
                    Err(e) => eprintln!("Error extracting ZIP file {}: {}", path.display(), e),
                    Ok(new_dir) => {
                        if let Err(e) = dir_handler(&new_dir) {
                            eprintln!("Error processing directory {}: {}", new_dir.display(), e);
                        }
                    }
                }
            } else if is_song(&path) {
                if let Err(e) = song_handler(&path) {
                    eprintln!("Error handling song file {}: {}", path.display(), e);
                }
            } else {
                if let Err(e) = fs::remove_file(&path) {
                    eprintln!("Error deleting file {}: {}", path.display(), e);
                }
            }
        }
    });
    Ok(())
}

fn main() -> io::Result<()> {
    let base_path = "/media/moosavi/files/music/Ramesh";

    if Path::new(base_path).is_dir() {
        dir_handler(Path::new(base_path))?;
    } else {
        let mut tagged_file = lofty::read_from_path(base_path).expect("bad");

        if let Some(tag) = tagged_file.primary_tag_mut() {
            tag.set_title("Khabe Baroon".to_string());
            tag.set_artist("Siavash Ghomayshi".to_string());
            tag.set_album("Single Song".to_string());
        }

        tagged_file
            .save_to_path(&base_path, WriteOptions::default())
            .expect("bad");
    }
    println!("Metadata updated successfully!");
    Ok(())
}
