use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::Digest;

use slab_utils::cab::{bytes_to_hex, ensure_parent_dir, sha256_file};

const BUNDLE_MAGIC: [u8; 16] = *b"SLAB-BUNDLE-V1\0\0";
const FOOTER_LEN: u64 = 16 + (8 * 3);

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    NsisInstaller,
    Cab,
    PayloadManifest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedBundleManifest {
    pub format_version: u32,
    pub version: String,
    pub assets: Vec<EmbeddedAssetRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedAssetRecord {
    pub name: String,
    pub kind: AssetKind,
    pub offset: u64,
    pub len: u64,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub struct EmbeddedBundle {
    executable_path: PathBuf,
    manifest: EmbeddedBundleManifest,
    footer: BundleFooter,
}

#[derive(Clone, Debug)]
pub struct AssetInput {
    pub name: String,
    pub kind: AssetKind,
    pub source_path: PathBuf,
}

#[derive(Clone, Copy, Debug)]
struct BundleFooter {
    manifest_offset: u64,
    manifest_len: u64,
    base_executable_len: u64,
}

impl EmbeddedBundle {
    pub fn version(&self) -> &str {
        &self.manifest.version
    }

    pub fn extract_asset_to_path_with_progress<F>(
        &self,
        name: &str,
        output_path: &Path,
        on_chunk: F,
    ) -> Result<()>
    where
        F: FnMut(u64) -> Result<()>,
    {
        let asset = self.asset(name)?;
        ensure_parent_dir(output_path)?;
        copy_region_to_path(&self.executable_path, asset.offset, asset.len, output_path, on_chunk)?;
        let extracted_hash = sha256_file(output_path)?;
        if extracted_hash != asset.sha256 {
            bail!("embedded asset '{}' failed hash verification after extraction", asset.name);
        }
        Ok(())
    }

    pub fn read_asset_bytes(&self, name: &str) -> Result<Vec<u8>> {
        let asset = self.asset(name)?;
        let mut reader = BufReader::new(
            File::open(&self.executable_path)
                .with_context(|| format!("failed to open {}", self.executable_path.display()))?,
        );
        reader
            .seek(SeekFrom::Start(asset.offset))
            .with_context(|| format!("failed to seek to embedded asset '{}'", asset.name))?;
        let mut buffer = vec![0_u8; asset.len as usize];
        reader
            .read_exact(&mut buffer)
            .with_context(|| format!("failed to read embedded asset '{}'", asset.name))?;
        Ok(buffer)
    }

    pub fn write_base_executable_to_path_with_progress<F>(
        &self,
        output_path: &Path,
        on_chunk: F,
    ) -> Result<()>
    where
        F: FnMut(u64) -> Result<()>,
    {
        ensure_parent_dir(output_path)?;
        copy_region_to_path(
            &self.executable_path,
            0,
            self.footer.base_executable_len,
            output_path,
            on_chunk,
        )
    }

    pub fn asset_len(&self, name: &str) -> Result<u64> {
        Ok(self.asset(name)?.len)
    }

    pub fn base_executable_len(&self) -> u64 {
        self.footer.base_executable_len
    }

    fn asset(&self, name: &str) -> Result<&EmbeddedAssetRecord> {
        self.manifest
            .assets
            .iter()
            .find(|asset| asset.name == name)
            .ok_or_else(|| anyhow!("embedded asset '{}' is missing", name))
    }
}

pub fn write_embedded_bundle(
    base_executable_path: &Path,
    version: &str,
    assets: &[AssetInput],
    output_path: &Path,
) -> Result<()> {
    let base_bytes = read_base_executable_bytes(base_executable_path)?;
    ensure_parent_dir(output_path)?;
    let output_file = File::create(output_path)
        .with_context(|| format!("failed to create {}", output_path.display()))?;
    let mut writer = BufWriter::new(output_file);
    writer
        .write_all(&base_bytes)
        .with_context(|| format!("failed to write base executable to {}", output_path.display()))?;

    let mut next_offset = base_bytes.len() as u64;
    let mut asset_records = Vec::with_capacity(assets.len());
    for asset in assets {
        let (sha256, len) = copy_file_with_hash(&asset.source_path, &mut writer)?;
        asset_records.push(EmbeddedAssetRecord {
            name: asset.name.clone(),
            kind: asset.kind,
            offset: next_offset,
            len,
            sha256,
        });
        next_offset += len;
    }

    let manifest = EmbeddedBundleManifest {
        format_version: 1,
        version: version.to_string(),
        assets: asset_records,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .context("failed to serialize embedded bundle manifest")?;
    let manifest_offset = next_offset;
    writer.write_all(&manifest_bytes).with_context(|| {
        format!("failed to write embedded bundle manifest to {}", output_path.display())
    })?;
    next_offset += manifest_bytes.len() as u64;

    let footer = BundleFooter {
        manifest_offset,
        manifest_len: manifest_bytes.len() as u64,
        base_executable_len: base_bytes.len() as u64,
    };
    writer.write_all(&footer.to_bytes()).with_context(|| {
        format!("failed to write embedded bundle footer to {}", output_path.display())
    })?;
    next_offset += FOOTER_LEN;
    writer.flush().with_context(|| format!("failed to flush {}", output_path.display()))?;

    let written_len = fs::metadata(output_path)
        .with_context(|| format!("failed to read metadata for {}", output_path.display()))?
        .len();
    if written_len != next_offset {
        bail!(
            "embedded bundle size mismatch for '{}': expected {}, wrote {}",
            output_path.display(),
            next_offset,
            written_len
        );
    }

    Ok(())
}

pub fn load_embedded_bundle(executable_path: &Path) -> Result<Option<EmbeddedBundle>> {
    let mut reader = BufReader::new(
        File::open(executable_path)
            .with_context(|| format!("failed to open {}", executable_path.display()))?,
    );
    let file_len = reader
        .get_ref()
        .metadata()
        .with_context(|| format!("failed to read metadata for {}", executable_path.display()))?
        .len();

    if file_len < FOOTER_LEN {
        return Ok(None);
    }

    reader
        .seek(SeekFrom::End(-(FOOTER_LEN as i64)))
        .with_context(|| format!("failed to seek footer in {}", executable_path.display()))?;
    let mut footer_bytes = vec![0_u8; FOOTER_LEN as usize];
    reader
        .read_exact(&mut footer_bytes)
        .with_context(|| format!("failed to read footer in {}", executable_path.display()))?;
    let Some(footer) = BundleFooter::from_bytes(&footer_bytes) else {
        return Ok(None);
    };

    if footer.manifest_offset + footer.manifest_len + FOOTER_LEN != file_len {
        bail!("embedded bundle footer in '{}' is corrupt", executable_path.display());
    }

    reader
        .seek(SeekFrom::Start(footer.manifest_offset))
        .with_context(|| format!("failed to seek manifest in {}", executable_path.display()))?;
    let mut manifest_bytes = vec![0_u8; footer.manifest_len as usize];
    reader
        .read_exact(&mut manifest_bytes)
        .with_context(|| format!("failed to read manifest in {}", executable_path.display()))?;
    let manifest: EmbeddedBundleManifest =
        serde_json::from_slice(&manifest_bytes).with_context(|| {
            format!("failed to parse embedded bundle manifest in {}", executable_path.display())
        })?;

    Ok(Some(EmbeddedBundle { executable_path: executable_path.to_path_buf(), manifest, footer }))
}

pub fn read_base_executable_bytes(executable_path: &Path) -> Result<Vec<u8>> {
    let executable_path = executable_path.to_path_buf();
    let (path, len) = match load_embedded_bundle(&executable_path)? {
        Some(bundle) => (bundle.executable_path, bundle.footer.base_executable_len),
        None => {
            let len = fs::metadata(&executable_path)
                .with_context(|| {
                    format!("failed to read metadata for {}", executable_path.display())
                })?
                .len();
            (executable_path, len)
        }
    };

    let mut file =
        File::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut buffer = vec![0_u8; len as usize];
    file.read_exact(&mut buffer)
        .with_context(|| format!("failed to read base executable bytes from {}", path.display()))?;
    Ok(buffer)
}

fn copy_region_to_path<F>(
    source_path: &Path,
    offset: u64,
    len: u64,
    output_path: &Path,
    mut on_chunk: F,
) -> Result<()>
where
    F: FnMut(u64) -> Result<()>,
{
    let mut reader = BufReader::new(
        File::open(source_path)
            .with_context(|| format!("failed to open {}", source_path.display()))?,
    );
    reader
        .seek(SeekFrom::Start(offset))
        .with_context(|| format!("failed to seek {} to {}", source_path.display(), offset))?;

    let mut writer = BufWriter::new(
        File::create(output_path)
            .with_context(|| format!("failed to create {}", output_path.display()))?,
    );
    let mut remaining = len;
    let mut buffer = [0_u8; 1024 * 64];
    while remaining > 0 {
        let to_read = remaining.min(buffer.len() as u64) as usize;
        reader
            .read_exact(&mut buffer[..to_read])
            .with_context(|| format!("failed to read from {}", source_path.display()))?;
        writer
            .write_all(&buffer[..to_read])
            .with_context(|| format!("failed to write {}", output_path.display()))?;
        remaining -= to_read as u64;
        on_chunk(to_read as u64)?;
    }
    writer.flush().with_context(|| format!("failed to flush {}", output_path.display()))?;
    Ok(())
}

fn copy_file_with_hash(source_path: &Path, writer: &mut impl Write) -> Result<(String, u64)> {
    let file = File::open(source_path)
        .with_context(|| format!("failed to open {}", source_path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = sha2::Sha256::new();
    let mut len = 0_u64;
    let mut buffer = [0_u8; 1024 * 64];

    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("failed to read {}", source_path.display()))?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read]).with_context(|| {
            format!("failed to write bundled asset from {}", source_path.display())
        })?;
        sha2::Digest::update(&mut hasher, &buffer[..read]);
        len += read as u64;
    }

    Ok((bytes_to_hex(&sha2::Digest::finalize(hasher)), len))
}

impl BundleFooter {
    fn to_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(FOOTER_LEN as usize);
        bytes.extend_from_slice(&BUNDLE_MAGIC);
        bytes.extend_from_slice(&self.manifest_offset.to_le_bytes());
        bytes.extend_from_slice(&self.manifest_len.to_le_bytes());
        bytes.extend_from_slice(&self.base_executable_len.to_le_bytes());
        bytes
    }

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != FOOTER_LEN as usize {
            return None;
        }
        if bytes[..BUNDLE_MAGIC.len()] != BUNDLE_MAGIC {
            return None;
        }

        let mut index = BUNDLE_MAGIC.len();
        let manifest_offset = read_u64(bytes, &mut index)?;
        let manifest_len = read_u64(bytes, &mut index)?;
        let base_executable_len = read_u64(bytes, &mut index)?;
        Some(Self { manifest_offset, manifest_len, base_executable_len })
    }
}

fn read_u64(bytes: &[u8], index: &mut usize) -> Option<u64> {
    let end = *index + 8;
    let slice = bytes.get(*index..end)?;
    *index = end;
    Some(u64::from_le_bytes(slice.try_into().ok()?))
}
