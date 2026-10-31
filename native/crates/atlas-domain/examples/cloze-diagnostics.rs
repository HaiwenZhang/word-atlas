//! Emits sentence matches for cross-language migration diagnostics; not part of the application.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = atlas_domain::Catalog::bundled()?;
    for word in catalog.entries() {
        for (i, example) in word.ex.iter().enumerate() {
            let gap = atlas_domain::text::gap(&example.en, &word.w);
            println!(
                "{}",
                serde_json::json!([
                    word.id,
                    i,
                    gap.map(|g| g.before),
                    gap.map(|g| g.hidden),
                    gap.map(|g| g.after)
                ])
            );
        }
    }
    Ok(())
}
