use crate::domain::models::{
    ChatModelSource, ListModelsFilter, ModelSpec, UnifiedModelStatus, UpdateModelCommand,
};
use crate::error::AppCoreError;
use crate::infra::db::ModelStore;
use crate::test_support::{
    TEST_FILENAME, TEST_HUB_PROVIDER, TEST_PROVIDER_ID, TEST_REPO_ID, TestAppCore,
    cloud_chat_model_command, downloadable_llama_command, ready_local_llama_command,
};

#[tokio::test]
async fn model_catalog_create_get_list_and_persists_pack() {
    let app = TestAppCore::new().await;
    let mut command = downloadable_llama_command("catalog-local");
    command.display_name = " Catalog Local ".to_owned();

    let created = app.model.create_model(command).await.expect("create model");

    assert_eq!(created.id, "catalog-local");
    assert_eq!(created.display_name, "Catalog Local");
    assert_eq!(created.status, UnifiedModelStatus::NotDownloaded);
    assert_eq!(created.spec.hub_provider.as_deref(), Some(TEST_HUB_PROVIDER));
    assert!(app.model_pack_path(&created.id).is_file());

    let fetched = app.model.get_model(&created.id).await.expect("get model");
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.spec.repo_id.as_deref(), Some(TEST_REPO_ID));

    let listed = app.model.list_models(ListModelsFilter::default()).await.expect("list models");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);
    assert!(app.runtime.loads().is_empty());
    assert!(app.runtime.unloads().is_empty());
}

#[tokio::test]
async fn model_catalog_list_fails_on_corrupt_model_record() {
    let app = TestAppCore::new().await;
    let created =
        app.model.create_model(downloadable_llama_command("catalog-corrupt")).await.unwrap();
    let mut record = super::super::catalog::model_to_record(&created).expect("model record");
    record.spec = "null".to_owned();
    app.store.upsert_model(record).await.expect("replace model record");

    let error =
        app.model.list_models(ListModelsFilter::default()).await.expect_err("list should fail");

    assert!(matches!(
        error,
        AppCoreError::Internal(message) if message.contains("invalid spec JSON")
    ));
}

#[tokio::test]
async fn model_catalog_update_preserves_same_source_download_state_and_clears_changed_source() {
    let app = TestAppCore::new().await;
    let created =
        app.model.create_model(downloadable_llama_command("catalog-update")).await.unwrap();
    let local_path = app.model_cache_dir.join(TEST_FILENAME).to_string_lossy().into_owned();
    app.seed_downloaded_model_state(&created.id, &local_path).await;

    let same_source = app
        .model
        .update_model(
            &created.id,
            UpdateModelCommand {
                display_name: Some("Renamed Local".to_owned()),
                kind: None,
                backend_id: None,
                capabilities: None,
                status: None,
                spec: None,
                runtime_presets: None,
            },
        )
        .await
        .expect("update same source");

    assert_eq!(same_source.display_name, "Renamed Local");
    assert_eq!(same_source.spec.local_path.as_deref(), Some(local_path.as_str()));
    assert!(!same_source.materialized_artifacts.is_empty());
    assert_eq!(
        same_source.selected_download_source.as_ref().map(|source| source.repo_id.as_str()),
        Some(TEST_REPO_ID)
    );

    let changed_source = app
        .model
        .update_model(
            &created.id,
            UpdateModelCommand {
                display_name: None,
                kind: None,
                backend_id: None,
                capabilities: None,
                status: Some(UnifiedModelStatus::NotDownloaded),
                spec: Some(ModelSpec {
                    repo_id: Some("slab/other-llama".to_owned()),
                    hub_provider: Some(TEST_HUB_PROVIDER.to_owned()),
                    filename: Some("other-model.gguf".to_owned()),
                    ..ModelSpec::default()
                }),
                runtime_presets: None,
            },
        )
        .await
        .expect("update changed source");

    assert!(changed_source.spec.local_path.is_none());
    assert!(changed_source.materialized_artifacts.is_empty());
    assert!(changed_source.selected_download_source.is_none());
    assert_eq!(changed_source.spec.repo_id.as_deref(), Some("slab/other-llama"));
}

#[tokio::test]
async fn model_catalog_delete_removes_db_row_and_persisted_pack() {
    let app = TestAppCore::new().await;
    let created =
        app.model.create_model(downloadable_llama_command("catalog-delete")).await.unwrap();
    let pack_path = app.model_pack_path(&created.id);
    assert!(pack_path.is_file());

    let deleted = app.model.delete_model(&created.id).await.expect("delete model");

    assert_eq!(deleted.id, created.id);
    assert!(!pack_path.exists());
    assert!(app.store.get_model(&created.id).await.expect("get deleted model").is_none());
}

#[tokio::test]
async fn model_catalog_list_chat_models_filters_by_capability_and_provider() {
    let app = TestAppCore::new().await;
    let model_path = app.model_cache_dir.join("ready-chat.gguf");
    std::fs::write(&model_path, []).expect("write local model fixture");
    app.model
        .create_model(ready_local_llama_command("local-chat", &model_path))
        .await
        .expect("create local chat model");
    app.model
        .create_model(cloud_chat_model_command("cloud-chat", TEST_PROVIDER_ID))
        .await
        .expect("create cloud chat model");
    app.model
        .create_model(cloud_chat_model_command("cloud-missing-provider", "missing-provider"))
        .await
        .expect("create unknown-provider cloud model");

    let options = app.model.list_chat_models().await.expect("list chat models");
    let ids = options.iter().map(|option| option.id.as_str()).collect::<Vec<_>>();

    assert!(ids.contains(&"local-chat"));
    assert!(ids.contains(&"cloud-chat"));
    assert!(!ids.contains(&"cloud-missing-provider"));

    let local = options.iter().find(|option| option.id == "local-chat").unwrap();
    assert!(local.downloaded);
    assert!(!local.pending);
    assert_eq!(local.source, ChatModelSource::Local);

    let cloud = options.iter().find(|option| option.id == "cloud-chat").unwrap();
    assert!(cloud.downloaded);
    assert_eq!(cloud.source, ChatModelSource::Cloud);
    assert_eq!(cloud.provider_id.as_deref(), Some(TEST_PROVIDER_ID));
}
