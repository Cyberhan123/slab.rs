use crate::domain::models::{
    DownloadModelCommand, ListModelsFilter, ModelSpec, TaskStatus, UnifiedModelStatus,
};
use crate::error::AppCoreError;
use crate::infra::db::ModelDownloadStore;
use crate::test_support::{
    TEST_FILENAME, TEST_HUB_PROVIDER, TEST_REPO_ID, TestAppCore, downloadable_llama_command,
    local_llama_command,
};

#[tokio::test]
async fn model_download_reuses_existing_active_task_for_same_source() {
    let app = TestAppCore::new().await;
    let model = app.model.create_model(downloadable_llama_command("download-reuse")).await.unwrap();
    app.seed_model_download(
        "existing-download-task",
        &model.id,
        TEST_REPO_ID,
        TEST_FILENAME,
        Some(TEST_HUB_PROVIDER),
        TaskStatus::Running,
    )
    .await;

    let accepted = app
        .model
        .download_model(DownloadModelCommand { model_id: model.id.clone() })
        .await
        .expect("download should reuse active task");

    assert_eq!(accepted.operation_id, "existing-download-task");
    let downloads = app.store.list_model_downloads().await.expect("list downloads");
    assert_eq!(downloads.len(), 1);
    assert_eq!(downloads[0].task_id, "existing-download-task");
}

#[tokio::test]
async fn model_download_requires_repo_id_and_filename() {
    let app = TestAppCore::new().await;
    let mut missing_repo = local_llama_command("download-missing-repo");
    missing_repo.spec = ModelSpec {
        hub_provider: Some(TEST_HUB_PROVIDER.to_owned()),
        filename: Some(TEST_FILENAME.to_owned()),
        ..ModelSpec::default()
    };
    let missing_repo_model =
        app.model.persist_model_definition_with_options(missing_repo, false).await.unwrap();

    let repo_error = app
        .model
        .download_model(DownloadModelCommand { model_id: missing_repo_model.id })
        .await
        .expect_err("missing repo_id should fail");
    assert!(
        matches!(&repo_error, AppCoreError::BadRequest(message) if message.contains("missing repo_id")),
        "unexpected error: {repo_error}"
    );

    let mut missing_filename = downloadable_llama_command("download-missing-filename");
    missing_filename.spec.filename = None;
    let missing_filename_model =
        app.model.persist_model_definition_with_options(missing_filename, false).await.unwrap();

    let filename_error = app
        .model
        .download_model(DownloadModelCommand { model_id: missing_filename_model.id })
        .await
        .expect_err("missing filename should fail");
    assert!(
        matches!(&filename_error, AppCoreError::BadRequest(message) if message.contains("missing filename")),
        "unexpected error: {filename_error}"
    );
}

#[tokio::test]
async fn model_download_status_projects_into_list_models() {
    let app = TestAppCore::new().await;
    let pending =
        app.model.create_model(downloadable_llama_command("download-pending")).await.unwrap();
    let failed =
        app.model.create_model(downloadable_llama_command("download-failed")).await.unwrap();
    app.seed_model_download(
        "pending-download-task",
        &pending.id,
        TEST_REPO_ID,
        TEST_FILENAME,
        Some(TEST_HUB_PROVIDER),
        TaskStatus::Pending,
    )
    .await;
    app.seed_model_download(
        "failed-download-task",
        &failed.id,
        TEST_REPO_ID,
        TEST_FILENAME,
        Some(TEST_HUB_PROVIDER),
        TaskStatus::Failed,
    )
    .await;

    let models = app.model.list_models(ListModelsFilter::default()).await.expect("list models");
    let pending_model = models.iter().find(|model| model.id == pending.id).unwrap();
    let failed_model = models.iter().find(|model| model.id == failed.id).unwrap();

    assert_eq!(pending_model.status, UnifiedModelStatus::Downloading);
    assert_eq!(failed_model.status, UnifiedModelStatus::Error);
}
