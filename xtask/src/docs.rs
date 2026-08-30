use anyhow::{ensure, Context};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[derive(Deserialize)]
struct Site {
    canonical_url: String,
    sections: Vec<Section>,
    pages: Vec<Page>,
}

#[derive(Deserialize)]
struct Section {
    id: String,
    label: String,
    #[serde(rename = "order")]
    _order: u32,
}

#[derive(Deserialize)]
struct Page {
    id: String,
    source: String,
    route: String,
    section: String,
    order: u32,
    status: String,
    description: String,
}

#[derive(Serialize)]
struct ManifestEntry {
    path: String,
    sha256: String,
}

pub(crate) fn generate(root: &Path) -> anyhow::Result<PathBuf> {
    let output = root.join("target/docs-site");
    if output.exists() {
        fs::remove_dir_all(&output).context("failed to replace target/docs-site")?;
    }
    generate_to(root, &output)?;
    println!(
        "PASS: generated public documentation projection at {}",
        output.display()
    );
    Ok(output)
}

pub(crate) fn verify(root: &Path, locked: bool) -> anyhow::Result<()> {
    if locked {
        run(
            root,
            "pnpm",
            &["install", "--frozen-lockfile", "--ignore-scripts"],
        )?;
    }
    run(root, "node", &["scripts/validate-docs.mjs"])?;
    run(root, "node", &["scripts/validate-docs.mjs", "--self-test"])?;
    let first = tempfile::tempdir().context("create first documentation projection")?;
    let second = tempfile::tempdir().context("create second documentation projection")?;
    generate_to(root, first.path())?;
    generate_to(root, second.path())?;
    ensure!(
        inventory(first.path())? == inventory(second.path())?,
        "documentation projection is not deterministic"
    );
    println!(
        "PASS: documentation sources, personas, routes, STE policy, and deterministic projection"
    );
    Ok(())
}

pub(crate) fn package(root: &Path, locked: bool) -> anyhow::Result<()> {
    if locked {
        run(root, "pnpm", &["install", "--frozen-lockfile"])?;
    }
    run(root, "node", &["scripts/validate-authored-contracts.mjs"])?;
    run(root, "node", &["scripts/validate-generated-contracts.mjs"])?;
    run(
        root,
        "node",
        &["scripts/validate-integration-contracts.mjs"],
    )?;
    run(root, "node", &["scripts/validate-okf-uat.mjs"])?;
    let generated_directory = tempfile::tempdir().context("generate contract artifacts")?;
    let generated = crate::generate::generate_to(root, generated_directory.path())?;
    crate::generate::verify_checked_in(root, &generated)?;
    verify(root, false)?;
    generate(root)?;
    run(root, "pnpm", &["--filter", "@fasti/tokens", "build"])?;
    run(root, "pnpm", &["--filter", "@fasti/deploy-plan", "build"])?;
    run(root, "pnpm", &["--filter", "@fasti/docs", "typecheck"])?;
    run(root, "pnpm", &["--filter", "@fasti/docs", "build"])?;
    run(root, "node", &["scripts/validate-docs-build.mjs"])?;
    println!("PASS: packaged immutable static documentation site");
    Ok(())
}

fn generate_to(root: &Path, output: &Path) -> anyhow::Result<()> {
    let source = fs::read_to_string(root.join("docs/site.yaml")).context("read docs/site.yaml")?;
    let site: Site = serde_saphyr::from_str(&source).context("parse docs/site.yaml")?;
    let content = output.join("content");
    let static_root = output.join("static");
    fs::create_dir_all(&content)?;
    fs::create_dir_all(&static_root)?;
    let commit = command_output(root, "git", &["rev-parse", "HEAD"])?;

    for page in site.pages.iter().filter(|page| page.status == "published") {
        let physical = confined_source(root, &page.source)?;
        ensure!(physical.is_file(), "{} is not a file", page.source);
        let markdown = fs::read_to_string(&physical)?;
        let title = markdown
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("# "))
            .context("public page must start with one H1")?;
        let projected_markdown = pin_source_links(&markdown, &commit);
        let projected = format!(
            "---\nid: {}\nslug: {}\ntitle: {}\ndescription: {}\nsidebar_position: {}\ncustom_edit_url: {}\n---\n\n{}",
            serde_json::to_string(&page.id)?,
            serde_json::to_string(page.route.trim_end_matches('/'))?,
            serde_json::to_string(title)?,
            serde_json::to_string(&page.description)?,
            page.order,
            serde_json::to_string(&format!(
                "https://github.com/Scrobble-dev/Fasti/edit/dev/{}",
                page.source
            ))?,
            projected_markdown
        );
        let content_path = PathBuf::from(format!("{}.md", page.id));
        ensure!(
            lexically_confined(&content_path),
            "unsafe documentation page id {}",
            page.id
        );
        write(&content.join(content_path), projected.as_bytes())?;
        let route = route_path(&page.route);
        ensure!(
            !route.as_os_str().is_empty() && lexically_confined(&route),
            "unsafe documentation route {}",
            page.route
        );
        write(
            &static_root.join("markdown").join(route).join("index.md"),
            markdown.as_bytes(),
        )?;
    }

    copy(
        root,
        &static_root,
        "contracts/generated/v1/openapi.json",
        "openapi.json",
    )?;
    copy(
        root,
        &static_root,
        "contracts/generated/v1/conformance-openapi.json",
        "openapi-conformance.json",
    )?;
    copy(
        root,
        &static_root,
        "contracts/generated/v1/capabilities.json",
        "capabilities.json",
    )?;
    copy(
        root,
        &static_root,
        "contracts/generated/v1/problems.json",
        "problems.json",
    )?;
    copy(
        root,
        &static_root,
        "contracts/asyncapi/v1/transport.yaml",
        "asyncapi/transport.yaml",
    )?;
    copy(
        root,
        &static_root,
        "contracts/jsonld/v1/context.jsonld",
        "jsonld/context.jsonld",
    )?;
    copy(
        root,
        &static_root,
        "contracts/jsonld/v1/vocabulary.jsonld",
        "jsonld/vocabulary.jsonld",
    )?;
    copy(
        root,
        &static_root,
        "packages/sdk/src/generated.ts",
        "sdk/generated.ts",
    )?;
    copy_tree(root, &static_root, "packages/schemas/schemas", "schemas")?;
    copy_tree(root, &static_root, "contracts/okf/v1", "okf")?;
    copy_tree(root, &static_root, "brand/logos", "brand/logos")?;
    copy(
        root,
        &static_root,
        "diagrams/documentation-publication.svg",
        "diagrams/documentation-publication.svg",
    )?;

    let problems: Value = serde_json::from_slice(&fs::read(
        root.join("contracts/generated/v1/problems.json"),
    )?)?;
    for problem in problems["problems"]
        .as_array()
        .context("problem catalogue needs problems")?
    {
        let problem_type = problem["type"].as_str().context("problem needs type")?;
        let slug = problem_type
            .rsplit('/')
            .next()
            .context("problem type needs slug")?;
        ensure!(valid_slug(slug), "unsafe problem route slug {slug}");
        let title = problem["title"].as_str().context("problem needs title")?;
        let detail = problem["detail"].as_str().context("problem needs detail")?;
        let code = problem["code"].as_str().context("problem needs code")?;
        let safe_state = problem["safe_state"].as_str().unwrap_or("not stated");
        let retryability = problem["retryability"].as_str().unwrap_or("not stated");
        let page = format!(
            "---\ntitle: {}\nslug: {}\ndescription: {}\ncustom_edit_url: null\n---\n\n# {}\n\n`{}`\n\n{}\n\n## Safe state\n\n`{}`\n\n## Retry\n\n`{}`\n\n<a href=\"/problems.json\">Read the complete problem catalogue</a>\n",
            serde_json::to_string(title)?,
            serde_json::to_string(&format!("/v1/problems/{slug}"))?,
            serde_json::to_string(detail)?,
            escape_mdx_text(title),
            escape_mdx_text(code),
            escape_mdx_text(detail),
            escape_mdx_text(safe_state),
            escape_mdx_text(retryability)
        );
        write(
            &content.join("v1/problems").join(slug).join("index.md"),
            page.as_bytes(),
        )?;
    }

    let sidebars = site.sections.iter().map(|section| {
        let mut items = site.pages.iter().filter(|page| page.status == "published" && page.section == section.id).collect::<Vec<_>>();
        items.sort_by_key(|page| page.order);
        json!({"type":"category","label":section.label,"items":items.into_iter().map(|page| page.id.clone()).collect::<Vec<_>>()})
    }).collect::<Vec<_>>();
    write_json(&output.join("sidebars.json"), &json!({"docs": sidebars}))?;

    let release = json!({
        "schema_version": 1,
        "source_commit": commit,
        "canonical_url": site.canonical_url,
        "supported_release": false,
        "support_state": "unsupported",
        "statement": "Fasti has no supported public release. This site documents exact contract and implementation states."
    });
    write_json(&static_root.join("release.json"), &release)?;

    let llms = site
        .pages
        .iter()
        .filter(|page| page.status == "published")
        .map(|page| {
            format!(
                "- {}{} — {}",
                site.canonical_url, page.route, page.description
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    write(&static_root.join("llms.txt"), format!("# Fasti documentation\n\nFasti records media activity. Players play media.\n\n{llms}\n").as_bytes())?;
    write(
        &static_root.join("robots.txt"),
        b"User-agent: *\nAllow: /\nSitemap: https://fasti.scrobble.dev/sitemap.xml\n",
    )?;

    let manifest = inventory(output)?
        .into_iter()
        .filter(|(path, _)| path != "static/docs-manifest.json")
        .map(|(path, sha256)| ManifestEntry { path, sha256 })
        .collect::<Vec<_>>();
    write_json(
        &static_root.join("docs-manifest.json"),
        &json!({"schema_version":1,"source_commit":commit,"files":manifest}),
    )?;
    Ok(())
}

fn lexically_confined(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn route_path(route: &str) -> PathBuf {
    route
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect()
}

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn escape_mdx_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '{' => escaped.push_str("&#123;"),
            '}' => escaped.push_str("&#125;"),
            '`' => escaped.push_str("&#96;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn pin_source_links(markdown: &str, commit: &str) -> String {
    markdown.replace(
        "https://github.com/Scrobble-dev/Fasti/blob/dev/",
        &format!("https://github.com/Scrobble-dev/Fasti/blob/{commit}/"),
    )
}

fn confined_source(root: &Path, source: &str) -> anyhow::Result<PathBuf> {
    ensure!(
        lexically_confined(Path::new(source)),
        "{source} is not a confined relative source"
    );
    let physical_root = root.canonicalize().context("resolve repository root")?;
    let physical = root
        .join(source)
        .canonicalize()
        .with_context(|| format!("resolve {source}"))?;
    ensure!(
        physical.starts_with(&physical_root),
        "{source} resolves outside the repository"
    );
    Ok(physical)
}

fn copy(root: &Path, output: &Path, source: &str, target: &str) -> anyhow::Result<()> {
    let source_path = confined_source(root, source)?;
    ensure!(source_path.is_file(), "{source} is not a file");
    write(
        &output.join(target),
        &fs::read(source_path).with_context(|| format!("read {source}"))?,
    )
}

fn copy_tree(root: &Path, output: &Path, source: &str, target: &str) -> anyhow::Result<()> {
    let source_root = confined_source(root, source)?;
    ensure!(source_root.is_dir(), "{source} is not a directory");
    for entry in walk(&source_root)? {
        let relative = entry.strip_prefix(&source_root)?;
        write(&output.join(target).join(relative), &fs::read(&entry)?)?;
    }
    Ok(())
}

fn walk(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        ensure!(
            !file_type.is_symlink(),
            "refuse symbolic link {}",
            path.display()
        );
        if file_type.is_dir() {
            files.extend(walk(&path)?);
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    fs::create_dir_all(path.parent().context("generated path has no parent")?)?;
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}

fn write_json(path: &Path, value: &Value) -> anyhow::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write(path, &bytes)
}

fn inventory(root: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    if !root.exists() {
        return Ok(result);
    }
    for path in walk(root)? {
        let relative = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        result.insert(relative, crate::evidence::sha256_bytes(&fs::read(path)?));
    }
    Ok(result)
}

fn run(root: &Path, program: &str, arguments: &[&str]) -> anyhow::Result<()> {
    let status = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .status()
        .with_context(|| format!("start {program}"))?;
    ensure!(status.success(), "{program} {} failed", arguments.join(" "));
    Ok(())
}

fn command_output(root: &Path, program: &str, arguments: &[&str]) -> anyhow::Result<String> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .output()
        .with_context(|| format!("start {program}"))?;
    ensure!(
        output.status.success(),
        "{program} {} failed",
        arguments.join(" ")
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_routes_reject_traversal_and_unsafe_problem_slugs() {
        assert!(lexically_confined(Path::new("docs/start/index.md")));
        assert!(!lexically_confined(Path::new("docs/../SECURITY.md")));
        assert!(valid_slug("validation-failed"));
        assert!(!valid_slug("../escape"));
    }

    #[test]
    fn generated_mdx_is_literal_and_source_links_are_immutable() {
        assert_eq!(
            escape_mdx_text("a<&>{}`"),
            "a&lt;&amp;&gt;&#123;&#125;&#96;"
        );
        assert_eq!(
            pin_source_links(
                "https://github.com/Scrobble-dev/Fasti/blob/dev/docs/capability-ledger.md",
                "abc123"
            ),
            "https://github.com/Scrobble-dev/Fasti/blob/abc123/docs/capability-ledger.md"
        );
    }

    #[cfg(unix)]
    #[test]
    fn publication_rejects_symlink_escapes() {
        use std::os::unix::fs::symlink;

        let repository = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("private.txt"), b"private").unwrap();

        symlink(
            outside.path().join("private.txt"),
            repository.path().join("fixed.txt"),
        )
        .unwrap();
        assert!(copy(repository.path(), output.path(), "fixed.txt", "fixed.txt").is_err());

        fs::create_dir(repository.path().join("tree")).unwrap();
        symlink(
            outside.path().join("private.txt"),
            repository.path().join("tree/private.txt"),
        )
        .unwrap();
        assert!(copy_tree(repository.path(), output.path(), "tree", "tree").is_err());
    }
}
