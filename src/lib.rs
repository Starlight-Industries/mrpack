#![no_std]
#![forbid(unsafe_code)]

#[cfg(test)]
mod test;

#[cfg(all(feature = "std", feature = "alloc"))]
compile_error!("mrpack requires that either `std` (default) and `alloc` are not enabled together");

#[cfg(all(feature = "fs", feature = "alloc"))]
compile_warning::compile_warning!("mrpack does not support `fs` and `alloc` together");

#[cfg(not(any(feature = "alloc", feature = "std")))]
compile_error!("mrpack requires that either `std` (default) or `alloc` is enabled");

#[cfg(feature = "std")]
extern crate std;
#[cfg(feature = "std")]
use std::{collections::HashMap, path::PathBuf, string::String, vec::Vec};

#[cfg(all(feature = "fs", feature = "std"))]
use {
    std::{
        borrow::ToOwned,
        fs::File,
        io::{BufReader, BufWriter, Error, ErrorKind, Read, Result, Write},
        path::Path,
    },
    zip::{
        write::SimpleFileOptions as Options, CompressionMethod::Deflated, ZipArchive, ZipWriter,
    },
};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use {
    alloc::{string::String, vec::Vec},
    hashbrown::HashMap,
};

use serde::{Deserialize as De, Serialize as Ser};

// Ser De :lol:
#[derive(Ser, De, Debug)]
#[serde(rename_all = "camelCase")]
pub enum GameType {
    Minecraft,
}

#[derive(Ser, De, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Env {
    Required,
    Optional,
    Unsupported,
}

#[derive(Ser, De, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    client: Env,
    server: Env,
}

#[derive(Ser, De, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Hashes {
    sha1: String,
    sha512: String,
    #[serde(flatten)]
    optional: HashMap<String, String>,
}

#[derive(Ser, De, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Dependency {
    Minecraft,
    Forge,
    Neoforge,
    FabricLoader,
    QuiltLoader,
}

#[derive(Ser, De, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthFile {
    #[cfg(feature = "std")]
    /// The destination path of this file, relative to the Minecraft instance directory. For example, mods/MyMod.jar resolves to .minecraft/mods/MyMod.jar.
    pub path: PathBuf,
    #[cfg(not(feature = "std"))]
    /// The destination path of this file, relative to the Minecraft instance directory. For example, mods/MyMod.jar resolves to .minecraft/mods/MyMod.jar.
    pub path: String,
    /// The hashes of the file specified. This MUST contain the SHA1 hash and the SHA512 hash. Other hashes are optional, but will usually be ignored.
    pub hashes: Hashes,
    /// For files that only exist on a specific environment, this field allows that to be specified. It's an object which contains a client and server value. This uses the Modrinth client/server type specifications.
    pub env: Option<Environment>,
    /// An integer containing the size of the file, in bytes. This is mostly provided as a utility for launchers to allow use of progress bars.
    #[cfg(feature = "url")]
    /// An array containing HTTPS URLs where this file may be downloaded. URIs MUST NOT contain unencoded spaces or any other illegal characters according to RFC 3986.
    /// When uploading to Modrinth, the pack is validated so that only URIs from the following domains are allowed:
    /// - cdn.modrinth.com
    /// - github.com
    /// - raw.githubusercontent.com
    /// - gitlab.com
    pub downloads: Vec<url::Url>,
    #[cfg(not(feature = "url"))]
    /// An array containing HTTPS URLs where this file may be downloaded. URIs MUST NOT contain unencoded spaces or any other illegal characters according to RFC 3986.
    /// When uploading to Modrinth, the pack is validated so that only URIs from the following domains are allowed:
    /// - cdn.modrinth.com
    /// - github.com
    /// - raw.githubusercontent.com
    /// - gitlab.com
    pub downloads: Vec<String>,
    pub file_size: u128,
}

pub struct Bytes(pub Vec<u8>);

impl core::fmt::Debug for Bytes {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Skipped").finish()
    }
}

#[derive(Ser, De, Debug)]
#[serde(rename_all = "camelCase")]
/// The Modrinth modpack format (.mrpack) is a simple format that lets you store modpacks. This is the only format of modpack that can be uploaded to Modrinth.
pub struct ModrinthModpack {
    /// The version of the format, stored as a number. The current value at the time of writing is 1.
    pub format_version: u128,
    /// The game of the modpack, stored as a string. The only available type is minecraft.
    pub game: GameType,
    /// A unique identifier for this specific version of the modpack.
    pub version_id: String,
    /// Human-readable name of the modpack.
    pub name: String,
    /// A short description of this modpack.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// The files array contains a list of files for the modpack that needs to be downloaded. Each item in this array contains the following: [`ModrinthFile`]
    pub files: Vec<ModrinthFile>,
    /// This object contains a list of IDs and version numbers that launchers will use in order to know what to install.
    pub dependencies: HashMap<Dependency, String>,
    #[serde(skip)]
    #[cfg(all(feature = "fs", feature = "std"))]
    pub overrides: HashMap<PathBuf, Bytes>,
    #[serde(skip)]
    #[cfg(all(feature = "fs", feature = "std"))]
    pub server_overrides: HashMap<PathBuf, Bytes>,
    #[serde(skip)]
    #[cfg(all(feature = "fs", feature = "alloc"))]
    pub overrides: HashMap<String, Bytes>,
    #[serde(skip)]
    #[cfg(all(feature = "fs", feature = "alloc"))]
    pub server_overrides: HashMap<String, Bytes>,
}

#[cfg(all(feature = "std", feature = "fs"))]
impl ModrinthModpack {
    /// Read a path, deserialise the .mrpack file if it exists.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if path.extension().unwrap_or_default() != "mrpack" {
            return Err(Error::new(
                ErrorKind::InvalidData,
                std::format!("Path: {path:?} must have extension '.mrpack'"),
            ));
        }
        Self::from_reader(File::open(path)?)
    }
    /// Deserialise a .mrpack file.
    pub fn from_reader<R: Read + std::io::Seek>(reader: R) -> Result<Self> {
        let mut buf = BufReader::new(reader);
        let mut archive = ZipArchive::new(&mut buf)?;
        if archive.is_empty() {
            return Err(Error::new(ErrorKind::NotFound, "Invalid .mrpack: Empty"));
        }
        let mut index: ModrinthModpack = match archive.index_for_path("./modrinth.index.json") {
            Some(idx) => {
                let mut tmp = archive.by_index(idx)?;
                if !tmp.is_file() {
                    return Err(Error::new(
                        ErrorKind::IsADirectory,
                        "Invalid .mrpack: 'modrinth.index.json' should be a file.",
                    ));
                }
                serde_json::from_reader(&mut tmp)?
            }
            None => return Err(Error::new(
                ErrorKind::NotFound,
                "Invalid .mrpack: Does not contain 'modrinth.index.json' in the root directory.",
            )),
        };

        for ref path in archive
            .file_names()
            .map(|name| name.to_owned())
            .collect::<Vec<String>>()
        {
            if let Some(("overrides", file)) = path.split_once("/") {
                let mut buf = Vec::new();
                archive.by_name(path)?.read_to_end(&mut buf)?;
                index.overrides.insert(file.try_into().unwrap(), Bytes(buf));
            }
        }
        Ok(index)
    }
    /// Serialise a .mrpack file to a file
    /// If no compression level is given, a default value will be used.
    pub fn to_file<P: AsRef<Path>>(
        &self,
        path: P,
        pretty: bool,
        compression_level: Option<i64>,
    ) -> Result<()> {
        let path = path.as_ref();
        let mut writer = BufWriter::new(File::create(path)?);
        let mut archive = ZipWriter::new(&mut writer);
        let options = Options::default()
            .compression_method(Deflated)
            .compression_level(compression_level);
        let index = if pretty {
            let mut buf = Vec::new();
            let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
            let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
            self.serialize(&mut ser)?;
            buf
        } else {
            let mut buf = Vec::new();
            let formatter = serde_json::ser::CompactFormatter;
            let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
            self.serialize(&mut ser)?;
            buf
        };
        archive.start_file("modrinth.index.json", options)?;
        archive.write(&index)?;
        for file in self.overrides.iter().chain(self.server_overrides.iter()) {
            archive.start_file_from_path(file.0, options)?;
            archive.write(&file.1 .0)?;
        }
        archive.finish()?;
        Ok(())
    }
    #[cfg(feature = "resolve")]
    pub fn resolve<P: AsRef<Path>>(&self, _path: P) -> Result<()> {
        todo!()
    }
}
