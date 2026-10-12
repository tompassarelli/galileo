use crate::{value, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn rename(source: &str, names: &BTreeMap<String, String>) -> String {
    let mut result = String::new();
    let mut token = String::new();
    for ch in source.chars().chain(std::iter::once('\0')) {
        if ch.is_alphanumeric() || matches!(ch, '_' | '-') {
            token.push(ch);
        } else {
            result.push_str(names.get(&token).map_or(token.as_str(), String::as_str));
            token.clear();
            if ch != '\0' {
                result.push(ch);
            }
        }
    }
    result
}

fn copy(source: &Path, output: &Path, names: &BTreeMap<String, String>) -> Result<()> {
    fs::create_dir(output)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "non-UTF-8 port path")?;
        let target = output.join(rename(&name, names));
        if kind.is_dir() {
            copy(&entry.path(), &target, names)?;
        } else if kind.is_file() {
            fs::write(&target, rename(&fs::read_to_string(entry.path())?, names))?;
            fs::set_permissions(&target, entry.metadata()?.permissions())?;
        } else {
            return Err("scaled port must contain only regular files and directories".into());
        }
    }
    Ok(())
}

pub fn run(args: Vec<String>) -> Result<()> {
    let mut count = None;
    let mut root = None;
    let mut out = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--out" => out = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--help" | "-h" => {
                println!("Usage: compare scale COUNT --root PORT --out NEW_DIRECTORY\n\nPORT/modules contains one directory or source file per module. Copy the port\ninto clone-001 through clone-COUNT, suffixing every module name with _001,\netc. Rename module paths and exact identifier references in all UTF-8 files.\nThe output directory must not exist and must be outside the source port.");
                return Ok(());
            }
            _ if count.is_none() && !arg.starts_with('-') => count = Some(arg.parse::<usize>()?),
            _ => return Err(format!("unknown option: {arg}").into()),
        }
    }
    let count = count
        .filter(|count| *count > 0)
        .ok_or("scale needs a positive COUNT")?;
    let root = root.ok_or("scale requires --root")?.canonicalize()?;
    let out = out.ok_or("scale requires --out")?;
    if out.exists() {
        return Err("scaled output directory already exists".into());
    }
    let parent = out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let output = parent
        .canonicalize()?
        .join(out.file_name().ok_or("output needs a directory name")?);
    if output.starts_with(&root) {
        return Err("scaled output must be outside source port".into());
    }
    let mut modules = Vec::new();
    for entry in fs::read_dir(root.join("modules"))? {
        let entry = entry?;
        let path = entry.path();
        let name = if entry.file_type()?.is_dir() {
            path.file_name()
        } else {
            path.file_stem()
        }
        .and_then(|name| name.to_str())
        .ok_or("invalid module name")?;
        if name.is_empty()
            || !name
                .chars()
                .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '-'))
        {
            return Err("module names must be identifiers".into());
        }
        modules.push(name.to_owned());
    }
    modules.sort();
    if modules.is_empty() {
        return Err("port has no modules".into());
    }
    if modules.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("duplicate module name".into());
    }
    fs::create_dir(&output)?;
    for clone in 1..=count {
        let names = modules
            .iter()
            .map(|name| (name.clone(), format!("{name}_{clone:03}")))
            .collect();
        copy(&root, &output.join(format!("clone-{clone:03}")), &names)?;
    }
    println!(
        "clones\tmodules\n{count}\t{}",
        count
            .checked_mul(modules.len())
            .ok_or("module count overflow")?
    );
    Ok(())
}
