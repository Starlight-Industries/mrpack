#![no_std]
#![forbid(unsafe_code)]

#[cfg(test)]
mod test;

#[cfg(all(feature = "std", feature = "alloc"))]
compile_error!("mrpack requires only one of `std` (default) or `alloc` is enabled");

#[cfg(all(feature = "fs", feature = "alloc"))]
compile_error!("mrpack requires only one of `fs` (default) or `alloc` is enabled");

#[cfg(all(feature = "resolve", feature = "alloc"))]
compile_error!("mrpack requires only one of `resolve` or `alloc` is enabled");

#[cfg(not(any(feature = "alloc", feature = "std")))]
compile_error!("mrpack requires that one of `std` (default) or `alloc` is enabled");

#[cfg(feature = "std")]
extern crate std;
#[cfg(feature = "std")]
use std::{collections::BTreeMap, string::String, vec::Vec};

#[cfg(all(feature = "fs", feature = "std"))]
use {
	serde_json::{ser::PrettyFormatter, Serializer},
	std::{
		fs::File as StdFile,
		io::{copy, BufReader, BufWriter, Error, ErrorKind, Read, Result, Seek, Write},
		path::{Path, PathBuf},
		string::ToString,
	},
	zip::{
		write::SimpleFileOptions as Options, CompressionMethod::Deflated, ZipArchive, ZipWriter,
	},
};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::{collections::BTreeMap, string::String, vec::Vec};

use {
	core::fmt::{Display, Formatter, Result as FmtResult},
	serde::{Deserialize as De, Serialize as Ser},
};

// Ser De :lol:
#[derive(Ser, De, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GameType {
	Minecraft,
}

#[derive(Ser, De, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Env {
	Required,
	Optional,
	Unsupported,
}

#[derive(Ser, De, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
	client: Env,
	server: Env,
}

#[derive(Ser, De, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Hashes {
	sha1: String,
	sha512: String,
	#[serde(flatten)]
	optional: BTreeMap<String, String>,
}

#[derive(Ser, De, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Dependency {
	Minecraft,
	Forge,
	Neoforge,
	FabricLoader,
	QuiltLoader,
}

#[derive(Ser, De, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct File {
	/// The destination path of this file, relative to the Minecraft instance directory. For example, mods/MyMod.jar resolves to .minecraft/mods/MyMod.jar.
	pub path: String,
	/// The hashes of the file specified. This MUST contain the SHA1 hash and the SHA512 hash. Other hashes are optional, but will usually be ignored.
	pub hashes: Hashes,
	/// For files that only exist on a specific environment, this field allows that to be specified. It's an object which contains a client and server value. This uses the Modrinth client/server type specifications.
	pub env: Option<Environment>,
	/// An array containing HTTPS URLs where this file may be downloaded. URIs MUST NOT contain unencoded spaces or any other illegal characters according to RFC 3986.
	/// When uploading to Modrinth, the pack is validated so that only URIs from the following domains are allowed:
	/// - github.com
	/// - raw.githubusercontent.com
	/// - gitlab.com
	pub downloads: Vec<String>,
	/// An integer containing the size of the file, in bytes. This is mostly provided as a utility for launchers to allow use of progress bars.
	pub file_size: u64,
}

impl Display for Modpack {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		let mut builder = f.debug_struct("Modpack");
		builder
			.field("format_version", &self.format_version)
			.field("game", &self.game)
			.field("version_id", &self.version_id)
			.field("name", &self.name)
			.field("dependencies", &self.dependencies)
			.field("name", &self.name)
			.field("summary", &self.summary)
			.field("files", &self.files);

		#[cfg(all(feature = "fs", feature = "std"))]
		{
			builder.field("overrides", &self.filesystem.keys())
		};

		builder.finish()
	}
}

#[derive(Ser, De, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
/// The Modrinth modpack format (.mrpack) is a simple format that lets you store modpacks. This is the only format of modpack that can be uploaded to Modrinth.
pub struct Modpack {
	/// The version of the format, stored as a number. The current value at the time of writing is 1.
	pub format_version: u64,
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
	pub files: Vec<File>,
	/// This object contains a list of IDs and version numbers that launchers will use in order to know what to install.
	pub dependencies: BTreeMap<Dependency, String>,
	#[serde(skip)]
	#[cfg(all(feature = "fs", feature = "std"))]
	pub filesystem: BTreeMap<PathBuf, Vec<u8>>,
}

#[cfg(all(feature = "std", feature = "fs"))]
impl Modpack {
	/// Read a path, deserialise the .mrpack file if it exists.
	pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
		let path = path.as_ref();
		if path.extension().is_none_or(|ext| ext != "mrpack") {
			return Err(Error::new(
				ErrorKind::InvalidData,
				std::format!("Path: {path:?} must have extension '.mrpack'"),
			));
		}
		Self::from_reader(BufReader::new(StdFile::open(path)?))
	}
	/// Deserialise a .mrpack file.<br>
	/// Recommended to first create [`std::io::BufReader`] from your Reader where possible.
	pub fn from_reader<R: Read + Seek>(mut reader: R) -> Result<Self> {
		let mut archive = ZipArchive::new(&mut reader)?;

		if archive.is_empty() {
			return Err(Error::new(ErrorKind::NotFound, "Invalid .mrpack: Empty"));
		}
		// performance consideration:
		// replacing this with file -> copy into memory -> deserialise
		// may help for files with realllllyyyy big indices
		let mut index: Modpack = serde_json::from_reader(archive.by_name("modrinth.index.json")?)?;

		for ref path in archive
			.file_names()
			.map(|name| name.to_string())
			.collect::<Vec<String>>()
		{
			if path == "modrinth.index.json" {
				continue;
			}
			let mut buf = Vec::new();
			copy(&mut archive.by_name(path)?, &mut buf)?;
			let path: PathBuf = path.into();
			if path.extension().is_none() {
				continue;
			}
			index.filesystem.insert(path, buf);
		}
		Ok(index)
	}
	/// Serialise a .mrpack file to a file<br>
	/// If no compression level is given, a default value will be used.<br>
	/// **DO NOT** use a compression level of less than three, it seems to be broken.
	pub fn to_file<P: AsRef<Path>>(
		&self,
		path: P,
		pretty: bool,
		compression_level: Option<i64>,
	) -> Result<()> {
		let path = path.as_ref();
		let mut writer = BufWriter::new(StdFile::create(path)?);
		let mut archive = ZipWriter::new(&mut writer);
		let options = Options::default()
			.compression_method(Deflated)
			.compression_level(compression_level);
		let mut index = Vec::new();
		if pretty {
			self.serialize(&mut Serializer::with_formatter(
				&mut index,
				PrettyFormatter::with_indent(b"    "),
			))?;
		} else {
			self.serialize(&mut Serializer::new(&mut index))?;
		};
		archive.start_file("modrinth.index.json", options)?;
		archive.write(&index)?;
		for (path, content) in self.filesystem.iter() {
			archive.start_file_from_path(path, options)?;
			copy(&mut &content[..], &mut archive)?;
		}
		archive.finish()?;
		Ok(())
	}
	#[cfg(feature = "resolve")]
	pub fn resolve<P: AsRef<Path>>(&self, _path: P) -> Result<()> {
		todo!()
	}
}
