pub use slab_types::settings::PMID;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use slab_types::settings::{PMID, SettingPmid};

    #[test]
    fn current_pmids_include_expected_paths() {
        assert_eq!(PMID.general.language().as_str(), "general.language");
        assert_eq!(PMID.runtime.mode().as_str(), "runtime.mode");
        assert_eq!(
            PMID.runtime.ggml.backends.llama.context_length().as_str(),
            "runtime.ggml.backends.llama.context_length"
        );
    }

    #[test]
    fn current_pmids_are_unique() {
        let actual: BTreeSet<String> =
            PMID.all().into_iter().map(SettingPmid::into_string).collect();

        assert_eq!(actual.len(), PMID.all().len());
        assert!(actual.contains("providers.registry"));
        assert!(actual.contains("server.swagger.enabled"));
    }
}
