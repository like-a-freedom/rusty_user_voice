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
pub fn run_skills(
    command: &SkillsCommand,
    mode: OutputMode,
    output: &mut dyn Write,
) -> Result<(), CommandError> {
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
                        format!(
                            "run `uservoice-cli skills get {name} --full` to see the references of '{name}'"
                        ),
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
            Payload::Catalog(skills) => Ok(serde_json::json!({ "skills": skills })),
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
