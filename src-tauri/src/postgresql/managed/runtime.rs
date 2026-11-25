use super::{
    failure,
    files::{Dir, Identity},
    Result,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
pub const VERSION: &str = "18.6";
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pin {
    version: String,
    platform: String,
    manifest_sha256: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    version: String,
    platform: String,
    files: BTreeMap<String, Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    sha256: String,
    bytes: u64,
    executable: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Owner {
    schema_version: u32,
    identity: Identity,
    manifest_sha256: String,
}
pub struct Runtime {
    pub path: PathBuf,
    pub manifest_sha256: String,
}
pub fn hash(b: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, b)
        .as_ref()
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}
fn read(dir: &Dir, name: &str, limit: u64, owned: bool) -> Result<Vec<u8>> {
    if owned {
        dir.read(name, limit)
    } else {
        dir.read_bundle(name, limit)
    }
}
fn manifest(dir: &Dir, owned: bool) -> Result<(Manifest, Vec<u8>, Pin)> {
    let pin: Pin = serde_json::from_str(include_str!("runtime-pin.json"))
        .map_err(|_| failure("Invalid runtime pin"))?;
    let bytes = read(dir, "manifest.json", 1024 * 1024, owned)?;
    if hash(&bytes) != pin.manifest_sha256 {
        return Err(failure(
            "PostgreSQL payload manifest does not match the reviewed pin",
        ));
    }
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|_| failure("Invalid payload manifest"))?;
    if manifest.schema_version != 1
        || manifest.version != VERSION
        || pin.version != VERSION
        || manifest.platform != "macos-arm64"
        || pin.platform != manifest.platform
        || manifest.files.len() > 4096
    {
        return Err(failure("Unsupported PostgreSQL payload"));
    }
    Ok((manifest, bytes, pin))
}
fn parent(root: &Dir, path: &str, create: bool) -> Result<(Dir, String)> {
    let mut parts: Vec<_> = path.split('/').collect();
    if parts
        .iter()
        .any(|s| s.is_empty() || *s == "." || *s == ".." || s.contains(['\\', ':']))
    {
        return Err(failure("Unsafe payload entry"));
    }
    let leaf = parts
        .pop()
        .ok_or_else(|| failure("Missing payload entry"))?
        .to_owned();
    let mut dir = None;
    for part in parts {
        let base = dir.as_ref().unwrap_or(root);
        dir = Some(if create {
            base.existing_or_make(part)?
        } else {
            base.child(part)?
        });
    }
    Ok((dir.unwrap_or(root.child_for_read()?), leaf))
}
fn verify(dir: &Dir, m: &Manifest, owned: bool) -> Result<()> {
    fn tree(
        dir: &Dir,
        prefix: &str,
        m: &Manifest,
        owned: bool,
        depth: usize,
        count: &mut usize,
    ) -> Result<()> {
        if depth > 32 {
            return Err(failure("Oversized runtime tree"));
        }
        if owned {
            dir.private()?;
        } else {
            dir.bundle_directory()?;
        }
        for (name, mode) in dir.entries()? {
            *count += 1;
            if *count > 8192 {
                return Err(failure("Oversized runtime tree"));
            }
            let relative = format!("{prefix}{name}");
            if mode & libc::S_IFMT as u32 == libc::S_IFDIR as u32 {
                let next = format!("{relative}/");
                if !m.files.keys().any(|p| p.starts_with(&next)) {
                    return Err(failure("Unlisted runtime directory"));
                }
                tree(&dir.child(&name)?, &next, m, owned, depth + 1, count)?;
            } else if mode & libc::S_IFMT as u32 == libc::S_IFREG as u32 {
                if prefix.is_empty() && (name == "manifest.json" || (owned && name == "owner.json"))
                {
                    continue;
                }
                let entry = m
                    .files
                    .get(&relative)
                    .ok_or_else(|| failure("Unlisted runtime file"))?;
                if mode & (if owned { 0o077 } else { 0o7022 }) != 0
                    || (mode & 0o100 != 0) != entry.executable
                {
                    return Err(failure("Unsafe runtime file permissions"));
                }
            } else {
                return Err(failure("Runtime links and special files are forbidden"));
            }
        }
        Ok(())
    }
    tree(dir, "", m, owned, 0, &mut 0)?;
    for (name, e) in &m.files {
        if e.bytes > 128 * 1024 * 1024 {
            return Err(failure("Oversized PostgreSQL runtime file"));
        }
        let (p, leaf) = parent(dir, name, false)?;
        let b = read(&p, &leaf, e.bytes, owned)?;
        if b.len() as u64 != e.bytes || hash(&b) != e.sha256 {
            return Err(failure("PostgreSQL runtime integrity check failed"));
        }
    }
    Ok(())
}
pub fn provision(base: &Dir, base_path: &Path, source: &Path) -> Result<Runtime> {
    base.assert_path(base_path)?;
    let name = format!("runtime-{VERSION}");
    if base.exists(&name)? {
        return installed(base, base_path);
    }
    let src = Dir::absolute(source)?;
    let (m, b, pin) = manifest(&src, false)?;
    verify(&src, &m, false)?;
    let stage = format!(".runtime-{:032x}", rand::random::<u128>());
    let dir = base.make(&stage)?;
    for (name, e) in &m.files {
        let (s, leaf) = parent(&src, name, false)?;
        let bytes = s.read_bundle(&leaf, e.bytes)?;
        if hash(&bytes) != e.sha256 {
            return Err(failure("Source payload changed"));
        }
        let (d, leaf) = parent(&dir, name, true)?;
        d.create(&leaf, &bytes, e.executable)?;
    }
    dir.create("manifest.json", &b, false)?;
    dir.create(
        "owner.json",
        &serde_json::to_vec(&Owner {
            schema_version: 1,
            identity: dir.identity()?,
            manifest_sha256: pin.manifest_sha256,
        })
        .map_err(|_| failure("Cannot encode runtime owner"))?,
        false,
    )?;
    verify(&dir, &m, true)?;
    src.assert_path(source)?;
    base.assert_path(base_path)?;
    base.publish(&stage, &name)?;
    installed(base, base_path)
}
pub fn installed(base: &Dir, base_path: &Path) -> Result<Runtime> {
    base.assert_path(base_path)?;
    let path = base_path.join(format!("runtime-{VERSION}"));
    let dir = Dir::absolute(&path)?;
    dir.private()?;
    let (m, _, pin) = manifest(&dir, true)?;
    let owner: Owner = serde_json::from_slice(&dir.read("owner.json", 4096)?)
        .map_err(|_| failure("Unowned PostgreSQL runtime"))?;
    if owner.schema_version != 1
        || owner.identity != dir.identity()?
        || owner.manifest_sha256 != pin.manifest_sha256
    {
        return Err(failure("PostgreSQL runtime ownership changed"));
    }
    verify(&dir, &m, true)?;
    dir.assert_path(&path)?;
    Ok(Runtime {
        path,
        manifest_sha256: pin.manifest_sha256,
    })
}
