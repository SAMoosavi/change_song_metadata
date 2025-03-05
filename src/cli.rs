use clap::{Parser, ValueEnum};
use std::{fmt, path::PathBuf};

#[derive(Clone, Debug, ValueEnum)]
pub enum Change {
    Disable,
    Auto,
    #[clap(skip)]
    Default(String),
}

// Manually implement `ToString` for `Change`
impl fmt::Display for Change {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Change::Disable => write!(f, "disable"),
            Change::Auto => write!(f, "auto"),
            Change::Default(value) => write!(f, "default={}", value),
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Conf {
    #[arg(short, long)]
    pub file_path: PathBuf,

    #[arg(long, default_value_t =  Change::Auto)]
    pub artist: Change,

    #[arg(long, default_value_t =  Change::Auto)]
    pub album: Change,

    #[arg(long, default_value_t =  Change::Auto)]
    pub title: Change,

    #[arg(long, default_value_t = false)]
    pub remove_other_file: bool,

    #[arg(long, default_value_t = false)]
    pub remove_zip_file: bool,

    #[arg(long, default_value_t = false)]
    pub move_to_parent: bool,

    #[arg(long, default_value_t = true)]
    pub change: bool,
}

impl Conf {
    pub fn copy_from_file_path(&self, file_path: PathBuf) -> Self {
        Self {
            file_path,
            artist: self.artist.clone(),
            album: self.album.clone(),
            title: self.title.clone(),
            remove_other_file: self.remove_other_file,
            remove_zip_file: self.remove_zip_file,
            move_to_parent: self.move_to_parent,
            change: self.change,
        }
    }
}

impl fmt::Display for Conf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.file_path.display())
    }
}
