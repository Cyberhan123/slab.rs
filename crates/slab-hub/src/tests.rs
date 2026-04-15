use crate::{
    HubClient, HubEndpoints, HubProvider, HubProviderPreference, endpoints::DEFAULT_HF_ENDPOINT,
};

#[test]
fn parses_provider_aliases() {
    assert_eq!("hf".parse::<HubProvider>().ok(), Some(HubProvider::HfHub));
    assert_eq!("models_cat".parse::<HubProvider>().ok(), Some(HubProvider::ModelsCat));
    assert_eq!(
        "huggingface_hub".parse::<HubProvider>().ok(),
        Some(HubProvider::HuggingfaceHubRust)
    );
}

#[test]
fn auto_provider_preference_normalizes_blank_values() {
    assert_eq!(
        HubProviderPreference::from_optional_str(None).unwrap(),
        HubProviderPreference::Auto
    );
    assert_eq!(
        HubProviderPreference::from_optional_str(Some(" auto ")).unwrap(),
        HubProviderPreference::Auto
    );
    assert_eq!(
        HubProviderPreference::from_optional_str(Some("AUTO")).unwrap(),
        HubProviderPreference::Auto
    );
}

#[test]
fn explicit_provider_preference_disables_fallback_order_changes() {
    let providers = HubClient::new()
        .with_provider_preference(HubProviderPreference::Provider(HubProvider::HfHub))
        .enabled_providers()
        .expect("providers");
    assert_eq!(providers, vec![HubProvider::HfHub]);
}

#[test]
fn auto_provider_preference_uses_default_enabled_order() {
    let providers = HubClient::new().enabled_providers().expect("providers");
    let mut expected = Vec::new();
    #[cfg(feature = "provider-hf-hub")]
    expected.push(HubProvider::HfHub);
    #[cfg(feature = "provider-models-cat")]
    expected.push(HubProvider::ModelsCat);
    #[cfg(feature = "provider-huggingface-hub-rust")]
    expected.push(HubProvider::HuggingfaceHubRust);
    assert_eq!(providers, expected);
}

#[test]
fn auto_provider_preference_reuses_last_successful_provider_first() {
    let client = HubClient::new();
    #[cfg(all(feature = "provider-hf-hub", feature = "provider-models-cat"))]
    {
        client.set_cached_provider(Some(HubProvider::ModelsCat));
        let providers = client.enabled_providers().expect("providers");
        assert_eq!(providers.first().copied(), Some(HubProvider::ModelsCat));
    }
}

#[test]
fn huggingface_hub_rust_probe_uses_default_hf_endpoint() {
    let endpoints = HubEndpoints {
        hf_endpoint: "https://custom-hf.example".to_owned(),
        models_cat_endpoint: "https://modelscope.example".to_owned(),
    };

    assert_eq!(HubProvider::HuggingfaceHubRust.base_url(&endpoints), DEFAULT_HF_ENDPOINT);
    assert_eq!(HubProvider::HfHub.base_url(&endpoints), "https://custom-hf.example");
}
