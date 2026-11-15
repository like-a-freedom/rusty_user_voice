//! Contract tests for the bundled skill corpus.
//!
//! The corpus is compiled into the binary, so the only way it can fall out of
//! step with the code is if this file stops checking it. These tests are the
//! enforcement point: a change to a flag, a tool, a reason token, or an
//! envelope status must update the corpus in the same change.

use std::collections::BTreeSet;
use std::path::Path;

use clap::CommandFactory;
use rusty_user_voice::{
    COMMAND_REASONS, Cli, OutputMode, ToolName, cli::SkillsCommand, skills, skills_cli,
};

/// The corpus text of one file, addressed by its path inside `skills/`.
fn corpus_text(path: &str) -> String {
    skills::catalog()
        .into_iter()
        .flat_map(|skill| {
            // The skill document keeps its corpus path; a reference keeps the
            // path relative to the skill directory, so both are resolved
            // against the skill's own directory.
            let directory = skill
                .path
                .rsplit_once('/')
                .map(|(dir, _)| dir)
                .unwrap_or("");
            let mut files = vec![(skill.path.to_owned(), skill.content)];
            files.extend(
                skill.references.iter().map(|reference| {
                    (format!("{directory}/{}", reference.path), reference.content)
                }),
            );
            files
        })
        .find(|(candidate, _)| candidate == path)
        .map(|(_, content)| content.to_owned())
        .unwrap_or_else(|| panic!("the corpus has no file '{path}'"))
}

/// Every long flag in the CLI, read out of the clap tree itself.
fn long_flags() -> BTreeSet<String> {
    fn walk(command: &clap::Command, flags: &mut BTreeSet<String>) {
        for argument in command.get_arguments() {
            if let Some(long) = argument.get_long()
                && long != "help"
                && long != "version"
            {
                flags.insert(format!("--{long}"));
            }
        }
        for subcommand in command.get_subcommands() {
            walk(subcommand, flags);
        }
    }

    let mut flags = BTreeSet::new();
    walk(&Cli::command(), &mut flags);
    flags
}

/// The skills subcommand, as the CLI runner sees it.
fn skills_command(command: SkillsCommand) -> SkillsCommand {
    command
}

fn run(command: SkillsCommand) -> Result<String, rusty_user_voice::CommandError> {
    let mut output = Vec::new();
    skills_cli::run_skills(&command, OutputMode::Human, &mut output)?;
    Ok(String::from_utf8(output).expect("the corpus is UTF-8"))
}

#[test]
fn every_long_flag_is_documented_in_the_cli_reference() {
    let reference = corpus_text("core/references/cli.md");
    let mut missing = Vec::new();
    for flag in long_flags() {
        if !reference.contains(&flag) {
            missing.push(flag);
        }
    }
    assert!(
        missing.is_empty(),
        "the CLI reference does not document: {missing:?}"
    );
}

#[test]
fn every_mcp_tool_is_documented_in_the_tool_reference() {
    let reference = corpus_text("core/references/mcp-tools.md");
    let mut missing = Vec::new();
    for tool in ToolName::ALL {
        if !reference.contains(tool.as_str()) {
            missing.push(tool.as_str());
        }
    }
    assert!(
        missing.is_empty(),
        "the MCP tool reference does not document: {missing:?}"
    );
}

#[test]
fn every_reason_token_is_documented_in_the_error_reference() {
    let reference = corpus_text("core/references/errors.md");
    let mut missing = Vec::new();
    for reason in COMMAND_REASONS {
        if !reference.contains(&format!("`{reason}`")) {
            missing.push(*reason);
        }
    }
    assert!(
        missing.is_empty(),
        "the error reference does not document: {missing:?}"
    );
}

#[test]
fn every_envelope_status_is_documented_in_the_error_reference() {
    let reference = corpus_text("core/references/errors.md");
    for status in ["ok", "partial", "no_results", "conflict"] {
        assert!(
            reference.contains(&format!("`{status}`")),
            "the error reference does not document the '{status}' status"
        );
    }
}

#[test]
fn the_corpus_documents_every_command_the_cli_offers() {
    let reference = corpus_text("core/references/cli.md");
    for command in [
        "login",
        "logout",
        "workspace",
        "ideas search",
        "ideas get",
        "ideas collect",
        "ideas update",
        "ideas publish",
        "feedback record",
        "skills",
    ] {
        assert!(
            reference.contains(command),
            "the CLI reference does not document the '{command}' command"
        );
    }
}

#[test]
fn every_skill_document_has_frontmatter_with_a_name_and_a_description() {
    for skill in skills::catalog() {
        assert!(
            skill.content.starts_with("---\n"),
            "the '{}' skill has no frontmatter",
            skill.name
        );
        assert!(
            !skill.description.trim().is_empty(),
            "the '{}' skill has no description",
            skill.name
        );
    }
}

#[test]
fn every_reference_is_linked_from_the_skill_that_owns_it() {
    for skill in skills::catalog() {
        for reference in &skill.references {
            assert!(
                skill.content.contains(&reference.name),
                "the '{}' skill does not link its reference '{}'",
                skill.name,
                reference.name
            );
        }
    }
}

/// The "load more" map is the whole progressive disclosure mechanism. Every
/// row must be a command an agent can run, not a file name it cannot reach.
#[test]
fn the_load_more_map_gives_a_runnable_command_for_every_reference() {
    let core = corpus_text("core/SKILL.md");
    for reference in skills::find("core").expect("core is bundled").references {
        let command = format!("uservoice-cli skills get core --ref {}", reference.name);
        assert!(
            core.contains(&command),
            "the load more map does not give a command for '{}': expected '{command}'",
            reference.name
        );
    }
    for skill in ["triage", "feedback-loop"] {
        let command = format!("uservoice-cli skills get {skill}");
        assert!(
            core.contains(&command),
            "the load more map does not point at the '{skill}' skill: expected '{command}'"
        );
    }
}

/// A markdown link inside the corpus must resolve inside the corpus. A dead
/// link sends an agent looking for a file that does not exist.
#[test]
fn every_markdown_link_in_the_corpus_resolves() {
    let corpus_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("skills");
    for file in corpus_files(&corpus_root) {
        let content = std::fs::read_to_string(&file).expect("read the corpus file");
        let directory = file.parent().expect("a corpus file has a parent");
        for link in link_targets(&content) {
            let resolved = directory.join(&link);
            assert!(
                resolved.exists(),
                "the corpus file '{}' links to '{link}', which does not exist",
                file.display()
            );
        }
    }
}

/// Every markdown file under the corpus root.
fn corpus_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory).expect("read a corpus directory") {
            let path = entry.expect("read a corpus entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// The relative targets of the markdown links in one document.
fn link_targets(content: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("](") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find(')') else { break };
        let target = &rest[..end];
        let target = target.split('#').next().unwrap_or(target);
        if !target.is_empty()
            && !target.starts_with("http://")
            && !target.starts_with("https://")
            && !target.starts_with("mailto:")
        {
            targets.push(target.to_owned());
        }
        rest = &rest[end + 1..];
    }
    targets
}

/// Every file on disk must be embedded in the binary. A file the build script
/// skipped would be invisible to `skills get` and to the agent.
#[test]
fn every_corpus_file_is_embedded_in_the_binary() {
    let corpus_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("skills");
    let embedded: BTreeSet<String> = skills::catalog()
        .into_iter()
        .flat_map(|skill| {
            let mut files = vec![skill.path.to_owned()];
            files.extend(skill.references.iter().map(|reference| {
                format!(
                    "{}/{}",
                    skill.path.split('/').next().unwrap_or_default(),
                    reference.path
                )
            }));
            files
        })
        .collect();

    for file in corpus_files(&corpus_root) {
        let relative = file
            .strip_prefix(&corpus_root)
            .expect("the corpus file is under the corpus root")
            .to_string_lossy()
            .replace('\\', "/");
        assert!(
            embedded.contains(&relative),
            "the corpus file '{relative}' is not embedded in the binary"
        );
    }
}

#[test]
fn the_corpus_respects_its_context_budget() {
    /// Word budget per file. The always-loaded layer is the small one; a
    /// reference is read on demand.
    const BUDGETS: &[(&str, usize)] = &[
        ("core/SKILL.md", 1800),
        ("core/references/cli.md", 1200),
        ("core/references/mcp-tools.md", 900),
        ("core/references/pagination.md", 700),
        ("core/references/errors.md", 700),
        ("core/references/envelope.md", 700),
        ("triage/SKILL.md", 900),
        ("feedback-loop/SKILL.md", 900),
    ];
    for (path, budget) in BUDGETS {
        let words = corpus_text(path).split_whitespace().count();
        assert!(
            words <= *budget,
            "'{path}' is {words} words, over the {budget} word budget"
        );
    }
}
#[test]
fn the_whole_corpus_stays_inside_the_context_budget() {
    let words: usize = skills::catalog()
        .into_iter()
        .flat_map(|skill| {
            let mut files = vec![skill.content];
            files.extend(skill.references.iter().map(|reference| reference.content));
            files
        })
        .map(|content| content.split_whitespace().count())
        .sum();
    assert!(
        words <= 6000,
        "the corpus is {words} words, over the 6000 word budget"
    );
}

#[test]
fn listing_names_every_bundled_skill() {
    let listing = run(skills_command(SkillsCommand::List {})).expect("list");
    for skill in skills::catalog() {
        assert!(
            listing.contains(&skill.name),
            "the listing does not mention the '{}' skill",
            skill.name
        );
    }
}

#[test]
fn getting_a_skill_prints_its_document() {
    let output = run(skills_command(SkillsCommand::Get {
        name: Some("core".to_owned()),
        full: false,
        reference: None,
        all: false,
    }))
    .expect("get core");
    assert!(output.contains("# UserVoice core"));
    assert!(output.contains("name: core"));
}

#[test]
fn getting_a_reference_prints_only_that_reference() {
    let output = run(skills_command(SkillsCommand::Get {
        name: Some("core".to_owned()),
        full: false,
        reference: Some("cli.md".to_owned()),
        all: false,
    }))
    .expect("get core --ref cli.md");
    assert!(output.contains("name: cli"));
    assert!(!output.contains("# UserVoice core"));
}

#[test]
fn a_reference_resolves_by_path_as_well_as_by_name() {
    let by_name = run(skills_command(SkillsCommand::Get {
        name: Some("core".to_owned()),
        full: false,
        reference: Some("cli.md".to_owned()),
        all: false,
    }))
    .expect("get core --ref cli.md");
    let by_path = run(skills_command(SkillsCommand::Get {
        name: Some("core".to_owned()),
        full: false,
        reference: Some("references/cli.md".to_owned()),
        all: false,
    }))
    .expect("get core --ref references/cli.md");
    assert_eq!(by_name, by_path);
}

#[test]
fn an_unknown_skill_is_a_classified_failure() {
    let error = run(skills_command(SkillsCommand::Get {
        name: Some("nope".to_owned()),
        full: false,
        reference: None,
        all: false,
    }))
    .expect_err("an unknown skill must fail");
    assert_eq!(error.reason, "unknown_skill");
    assert_eq!(error.exit_code, 1);
}

#[test]
fn an_unknown_reference_is_a_classified_failure() {
    let error = run(skills_command(SkillsCommand::Get {
        name: Some("core".to_owned()),
        full: false,
        reference: Some("nope.md".to_owned()),
        all: false,
    }))
    .expect_err("an unknown reference must fail");
    assert_eq!(error.reason, "unknown_reference");
    assert_eq!(error.exit_code, 1);
}

#[test]
fn the_skills_command_is_offered_by_the_cli() {
    // `skills` is intercepted in `bin/cli.rs` before the app is constructed.
    // It must still be a real subcommand, or an agent has no way to reach the
    // corpus.
    let names: Vec<String> = Cli::command()
        .get_subcommands()
        .map(|subcommand| subcommand.get_name().to_owned())
        .collect();
    assert!(
        names.iter().any(|name| name == "skills"),
        "the CLI does not offer a 'skills' command: {names:?}"
    );
}

#[test]
fn the_help_output_lists_the_skills_command() {
    let help = Cli::command().render_long_help().to_string();
    assert!(help.contains("skills"), "the help does not list 'skills'");
}
