#[cfg(all(feature = "std", feature = "fs"))]
mod filesystem {
	use crate::Modpack;
	use std::{
		borrow::ToOwned,
		fs::{create_dir, exists, read_dir, remove_dir_all, write},
		io::Result,
		println,
	};
	#[test]
	fn comprehensive() -> Result<()> {
		if exists("./target/tests/")? {
			remove_dir_all("./target/tests/")?;
		}
		create_dir("./target/tests/")?;
		read_dir("./tests/")?
			.filter(|entry| {
				entry
					.as_ref()
					.is_ok_and(|file| file.path().extension().is_some_and(|ext| ext == "mrpack"))
			})
			.map(|mrpack| -> Result<()> {
				let path = mrpack.unwrap().path();
				println!("Parsing: {path:?}");
				let modpack = Modpack::from_path(&path)?;
				let output =
					"./target/tests/".to_owned() + path.file_name().unwrap().to_str().unwrap();
				println!("Exporting to: {output:?}");
				modpack.to_file(&output, true, None)?;
				println!("Checking new against original");
				let modpack_check = Modpack::from_path(&output)?;
				if modpack_check != modpack {
					write(output.clone() + ".new", std::format!("{modpack_check:?}"))?;
					write(output + ".old", std::format!("{modpack:?}"))?;
					panic!("Assertion failed")
				}
				println!("Success\n");
				Ok(())
			})
			.collect::<Result<()>>()
	}
}
