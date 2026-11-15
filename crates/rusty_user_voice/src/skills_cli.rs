//! The `skills` command: serve the agent-facing corpus compiled into the
//! binary.
//!
//! This runs before [`crate::Settings::from_sources`], because the corpus has
//! to be readable without credentials. The app runner never sees it: the
//! command is intercepted in `bin/cli.rs` the same way `login` and `logout`
//! are.
//!
//! The output mode is the global one, so `skills` has no `--json` of its
//! own. `--output human` prints the document as it is, which is what an agent
//! wants to read. `--output json` wraps it, which is what a script wants.

use std::io::Write;

use serde::Serialize;

use crate::{
    cli::{OutputMode, SkillsCommand},
    cli_error::{
        CommandError, REASON_IO_ERROR, REASON_UNKNOWN_REFERENCE, REASON_UNKNOWN_SKILL,
        REASON_VALIDATION_FAILED,
    },
    skills,
};

/// A skill without its document, as `skills list` reports it.
#[derive(Serialize)]
struct SkillSummary {
    name: String,
    description: String,
    references: usize,
}

/// A reference document.
#[derive(Serialize)]
struct ReferenceDoc {
    path: String,
    content: String,
}

/// One skill with its document, and optionally its references.
#[derive(Serialize)]
struct SkillDoc {
    name: String,
    description: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    references: Option<Vec<ReferenceDoc>>,
}

/// What the command produced, before the mode decides its shape.
enum Payload {
    List(Vec<SkillSummary>),
    Skill(SkillDoc),
    Reference {
        name: String,
        reference: String,
        content: String,
    },
    Catalog(Vec<SkillDoc>),
}

/// Run a `skills` subcommand, writing the result to `output`.
///
/// The output mode is the global one. `human` prints the document as it is,
/// which is what an agent wants to read. `json` wraps it, which is what a
/// script wants. `ndjson` has no meaning for a document, so it is rejected
/// rather than silently printing prose.
pub fn run_skills(
    command: &SkillsCommand,
    mode: OutputMode,
    output: &mut dyn Write,
) -> Result<(), CommandError> {
    if mode == OutputMode::Ndjson {
        return Err(CommandError::new(
            REASON_VALIDATION_FAILED,
            "ndjson output is not supported for skills".to_owned(),
            "use `--output human` to read the document, or `--output json` for a wrapper"
                .to_owned(),
        ));
    }
    let payload = build_payload(command)?;
    render(payload, mode, output)
}

/// Turn the subcommand into the payload it produces.
fn build_payload(command: &SkillsCommand) -> Result<Payload, CommandError> {
    match command {
        SkillsCommand::List {} => Ok(Payload::List(
            skills::catalog()
                .iter()
                .map(|skill| SkillSummary {
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                    references: skill.references.len(),
                })
                .collect(),
        )),

        SkillsCommand::Get {
            name,
            full,
            reference,
            all,
        } => {
            if *all {
                return Ok(Payload::Catalog(
                    skills::catalog()
                        .iter()
                        .map(|skill| SkillDoc {
                            name: skill.name.clone(),
                            description: skill.description.clone(),
                            content: skill.content.to_owned(),
                            references: Some(
                                skill
                                    .references
                                    .iter()
                                    .map(|reference| ReferenceDoc {
                                        path: reference.path.clone(),
                                        content: reference.content.to_owned(),
                                    })
                                    .collect(),
                            ),
                        })
                        .collect(),
                ));
            }

            let name = name.as_deref().ok_or_else(|| {
                CommandError::new(
                    REASON_VALIDATION_FAILED,
                    "no skill name given".to_owned(),
                    "run `uservoice-cli skills list` to see the available skills".to_owned(),
                )
            })?;

            let skill = skills::find(name).ok_or_else(|| {
                CommandError::new(
                    REASON_UNKNOWN_SKILL,
                    format!("no skill named '{name}'"),
                    "run `uservoice-cli skills list` to see the available skills".to_owned(),
                )
            })?;

            if let Some(reference) = reference {
                let found = skill.reference(reference).ok_or_else(|| {
                    CommandError::new(
                        REASON_UNKNOWN_REFERENCE,
                        format!("skill '{name}' has no reference '{reference}'"),
                        reference_hint(&skill, reference),
                    )
                })?;
                return Ok(Payload::Reference {
                    name: skill.name.clone(),
                    reference: found.path.clone(),
                    content: found.content.to_owned(),
                });
            }

            Ok(Payload::Skill(SkillDoc {
                name: skill.name.clone(),
                description: skill.description.clone(),
                content: skill.content.to_owned(),
                references: if *full {
                    Some(
                        skill
                            .references
                            .iter()
                            .map(|reference| ReferenceDoc {
                                path: reference.path.clone(),
                                content: reference.content.to_owned(),
                            })
                            .collect(),
                    )
                } else {
                    None
                },
            }))
        }
    }
}

/// Write the payload in the requested shape.
fn render(payload: Payload, mode: OutputMode, output: &mut dyn Write) -> Result<(), CommandError> {
    if mode == OutputMode::Json {
        let value = match payload {
            Payload::List(skills) => Ok(serde_json::json!({ "skills": skills })),
            Payload::Skill(skill) => serde_json::to_value(skill),
            Payload::Reference {
                name,
                reference,
                content,
            } => Ok(serde_json::json!({
                "name": name,
                "reference": reference,
                "content": content,
            })),
            // The catalog carries the documents, the list carries the
            // metadata. Distinct keys, so a consumer can tell them apart
            // without inspecting the elements.
            Payload::Catalog(skills) => Ok(serde_json::json!({ "catalog": skills })),
        };
        let value = value.map_err(|error| {
            CommandError::new(
                REASON_IO_ERROR,
                format!("cannot encode the skill: {error}"),
                "this is a bug in the bundled corpus".to_owned(),
            )
        })?;
        serde_json::to_writer_pretty(&mut *output, &value)
            .map_err(|error| json_error(error, "write the skill"))?;
        return writeln!(output).map_err(|error| io_error(error, "write the skill"));
    }

    match payload {
        Payload::List(skills) => {
            for skill in skills {
                writeln!(
                    output,
                    "{:<14} {:>2} refs  {}",
                    skill.name, skill.references, skill.description
                )
                .map_err(|error| io_error(error, "write the skill list"))?;
            }
        }
        Payload::Skill(skill) => {
            write_text(&skill.content, output)?;
            if let Some(references) = skill.references {
                for reference in references {
                    write_text(&reference.content, output)?;
                }
            }
        }
        Payload::Reference { content, .. } => write_text(&content, output)?,
        Payload::Catalog(skills) => {
            for skill in skills {
                write_text(&skill.content, output)?;
                if let Some(references) = skill.references {
                    for reference in references {
                        write_text(&reference.content, output)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Print one document as it is.
fn write_text(text: &str, output: &mut dyn Write) -> Result<(), CommandError> {
    output
        .write_all(text.as_bytes())
        .map_err(|error| io_error(error, "write the skill"))?;
    if !text.ends_with('\n') {
        writeln!(output).map_err(|error| io_error(error, "write the skill"))?;
    }
    Ok(())
}

/// The recovery hint for a reference that the skill does not own.
///
/// A skill with references can list them. A skill without any cannot, so the
/// hint names the skill that does hold the reference, if there is one.
fn reference_hint(skill: &crate::skills::Skill, reference: &str) -> String {
    let owner = skills::catalog().into_iter().find(|candidate| {
        candidate
            .references
            .iter()
            .any(|found| found.name == reference || found.path == reference)
    });

    if let Some(owner) = owner
        && owner.name != skill.name
    {
        return format!(
            "run `uservoice-cli skills get {} --ref {reference}`",
            owner.name
        );
    }

    if skill.references.is_empty() {
        return format!(
            "the '{}' skill has no references; run `uservoice-cli skills list` to see the corpus",
            skill.name
        );
    }

    format!(
        "run `uservoice-cli skills get {} --full` to see the references of '{}'",
        skill.name, skill.name
    )
}

fn io_error(error: std::io::Error, action: &str) -> CommandError {
    CommandError::new(
        REASON_IO_ERROR,
        format!("cannot {action}: {error}"),
        "check that stdout is writable".to_owned(),
    )
}

fn json_error(error: serde_json::Error, action: &str) -> CommandError {
    CommandError::new(
        REASON_IO_ERROR,
        format!("cannot {action}: {error}"),
        "check that stdout is writable".to_owned(),
    )
}
