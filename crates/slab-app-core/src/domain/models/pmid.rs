// Re-export the shared structured PMID constant from the slab-types crate.
pub use slab_types::settings::PMID;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use slab_types::settings::{PMID, SettingPmid};

    use crate::domain::models::embedded_settings_schema;

    #[test]
    fn nested_builder_generates_expected_pmid() {
        assert_eq!(PMID.setup.backends.dir().as_str(), "setup.backends.dir");
        assert_eq!(
            PMID.runtime.model_auto_unload.idle_minutes().as_str(),
            "runtime.model_auto_unload.idle_minutes"
        );
    }

    #[test]
    fn structured_pmids_cover_embedded_schema() {
        let schema = embedded_settings_schema().expect("schema");
        let expected: BTreeSet<String> = schema
            .sections()
            .iter()
            .flat_map(|section| section.subsections.iter())
            .flat_map(|subsection| subsection.properties.iter())
            .map(|property| property.pmid.clone())
            .collect();
        let actual: BTreeSet<String> =
            PMID.all().into_iter().map(SettingPmid::into_string).collect();

        assert_eq!(actual, expected);
    }
}
