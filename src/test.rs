#[cfg(all(feature = "std"))]
mod std {
    use crate::*;

    #[cfg(feature = "fs")]
    #[test]
    fn from_mrpack() -> Result<()> {
        std::println!(
            "{:#?}",
            ModrinthModpack::from_path("./tests/Fabulously.Optimized-v6.1.0-beta.7.mrpack")?
        );
        Ok(())
    }
    #[test]
    fn to_mrpack() -> Result<()> {
        ModrinthModpack::from_path("./tests/Fabulously.Optimized-v6.1.0-beta.7.mrpack")?.to_file(
            "./target/Fabulously.Optimized-v6.1.0-beta.7.mrpack",
            true,
            Some(9),
        )
    }
}
