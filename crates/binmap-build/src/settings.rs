//! `binmap.toml` — the project's own settings.
//!
//! `F0.2` asks for a *configurable* matrix. It was not configurable: the axes
//! were whatever `SweepMatrix::default` said, and a user who wanted to sweep
//! `overflow-checks` or narrow a matrix that takes an hour had no way to say
//! so. This file is how they say so.
//!
//! Everything is optional. A project with no `binmap.toml` gets the defaults,
//! which is the case this must not make worse — the file exists to let someone
//! narrow or widen a sweep, not to become a thing they have to write before
//! the tool works.

use binmap_core::config::{
    BenchmarkCommand, DebugInfo, Lto, OptLevel, PanicStrategy, ProjectConfig, Strip,
};
use binmap_core::error::{Error, Result};
use toml_edit::{DocumentMut, Item};

/// The file's name, in the project root.
pub const FILE: &str = "binmap.toml";

/// Load `binmap.toml` over a config, if the project has one.
///
/// Returns whether anything was read, so the interface can say "using your
/// binmap.toml" rather than leaving the user guessing whether it was found.
pub fn apply(config: &mut ProjectConfig) -> Result<bool> {
    let path = config.root.join(FILE);
    if !path.exists() {
        return Ok(false);
    }

    let text = std::fs::read_to_string(&path).map_err(|source| Error::io(&path, source))?;
    let document: DocumentMut =
        text.parse().map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;

    if let Some(sweep) = document.get("sweep").and_then(Item::as_table) {
        let mut matrix = config.matrix.clone();
        read_axis(sweep, "opt-level", &mut matrix.opt_level, opt_level)?;
        read_axis(sweep, "lto", &mut matrix.lto, lto)?;
        read_axis(sweep, "panic", &mut matrix.panic, panic_strategy)?;
        read_axis(sweep, "strip", &mut matrix.strip, strip)?;
        read_axis(sweep, "debug", &mut matrix.debug, debug_info)?;

        if let Some(values) = sweep.get("codegen-units").and_then(Item::as_array) {
            matrix.codegen_units =
                values.iter().filter_map(|v| v.as_integer()).map(|n| n as u32).collect();
        }
        if let Some(values) = sweep.get("overflow-checks").and_then(Item::as_array) {
            matrix.overflow_checks = values.iter().filter_map(|v| v.as_bool()).collect();
        }
        if let Some(values) = sweep.get("target-cpu").and_then(Item::as_array) {
            matrix.target_cpu =
                values.iter().filter_map(|v| v.as_str()).map(str::to_string).collect();
        }
        config.matrix = matrix;
    }

    if let Some(benchmark) = document.get("benchmark").and_then(Item::as_table) {
        let program = benchmark
            .get("program")
            .and_then(Item::as_str)
            .ok_or_else(|| Error::Config(format!("{FILE}: [benchmark] needs a `program`")))?;
        let arguments = benchmark
            .get("arguments")
            .and_then(Item::as_array)
            .map(|values| {
                values.iter().filter_map(|v| v.as_str()).map(str::to_string).collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let samples = benchmark
            .get("samples")
            .and_then(Item::as_integer)
            .map(|n| n.max(3) as u32)
            .unwrap_or(10);
        config.benchmark =
            Some(BenchmarkCommand { program: program.to_string(), arguments, samples });
    }

    if let Some(parallelism) =
        document.get("sweep").and_then(|s| s.get("parallelism")).and_then(Item::as_integer)
    {
        config.parallelism = parallelism.max(1) as usize;
    }

    Ok(true)
}

/// Read one axis, refusing a value we do not recognise rather than dropping it.
///
/// Silently ignoring `opt-level = ["fast"]` would sweep a matrix the user did
/// not ask for and report results they would reasonably read as covering it.
fn read_axis<T>(
    table: &toml_edit::Table,
    key: &str,
    axis: &mut Vec<T>,
    parse: fn(&str) -> Option<T>,
) -> Result<()> {
    let Some(values) = table.get(key).and_then(Item::as_array) else {
        return Ok(());
    };

    let mut parsed = Vec::new();
    for value in values.iter() {
        // Both `lto = [false, "fat"]` and `opt-level = [3, "s"]` are natural to
        // write, so a number or a boolean is accepted where a name is.
        let spelling = value
            .as_str()
            .map(str::to_string)
            .or_else(|| value.as_integer().map(|n| n.to_string()))
            .or_else(|| value.as_bool().map(|b| b.to_string()))
            .ok_or_else(|| {
                Error::Config(format!("{FILE}: [sweep] {key} has a value that is not a name"))
            })?;

        parsed.push(parse(&spelling).ok_or_else(|| {
            Error::Config(format!("{FILE}: [sweep] {key} does not accept `{spelling}`"))
        })?);
    }
    *axis = parsed;
    Ok(())
}

fn opt_level(value: &str) -> Option<OptLevel> {
    Some(match value {
        "0" => OptLevel::Zero,
        "1" => OptLevel::One,
        "2" => OptLevel::Two,
        "3" => OptLevel::Three,
        "s" => OptLevel::Size,
        "z" => OptLevel::SizeNoLoopVec,
        _ => return None,
    })
}

fn lto(value: &str) -> Option<Lto> {
    Some(match value {
        "false" | "off" => Lto::Off,
        "thin" => Lto::Thin,
        "fat" | "true" => Lto::Fat,
        _ => return None,
    })
}

fn panic_strategy(value: &str) -> Option<PanicStrategy> {
    Some(match value {
        "unwind" => PanicStrategy::Unwind,
        "abort" => PanicStrategy::Abort,
        _ => return None,
    })
}

fn strip(value: &str) -> Option<Strip> {
    Some(match value {
        "none" | "false" => Strip::None,
        "debuginfo" => Strip::Debuginfo,
        "symbols" | "true" => Strip::Symbols,
        _ => return None,
    })
}

fn debug_info(value: &str) -> Option<DebugInfo> {
    Some(match value {
        "0" | "false" => DebugInfo::None,
        "line-tables-only" => DebugInfo::LineTablesOnly,
        "1" | "limited" => DebugInfo::Limited,
        "2" | "true" | "full" => DebugInfo::Full,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use binmap_core::config::SweepMatrix;

    fn config_with(contents: &str) -> (tempfile::TempDir, ProjectConfig) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        std::fs::write(directory.path().join(FILE), contents).unwrap();
        let config = ProjectConfig::new(directory.path());
        (directory, config)
    }

    #[test]
    fn a_project_without_the_file_keeps_the_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let mut config = ProjectConfig::new(directory.path());
        let defaults = config.matrix.clone();
        assert!(!apply(&mut config).unwrap());
        assert_eq!(config.matrix, defaults);
    }

    #[test]
    fn an_axis_can_be_narrowed_or_widened() {
        let (_d, mut config) =
            config_with("[sweep]\nopt-level = [\"3\", \"z\"]\noverflow-checks = [false, true]\n");
        assert!(apply(&mut config).unwrap());
        assert_eq!(config.matrix.opt_level, vec![OptLevel::Three, OptLevel::SizeNoLoopVec]);
        assert_eq!(config.matrix.overflow_checks, vec![false, true]);
        // Axes the file did not mention are untouched.
        assert_eq!(config.matrix.lto, SweepMatrix::default().lto);
    }

    #[test]
    fn numbers_and_booleans_are_accepted_where_names_are() {
        // `lto = [false, "fat"]` and `opt-level = [3, "s"]` are both natural to
        // write, and refusing either would be pedantry.
        let (_d, mut config) =
            config_with("[sweep]\nopt-level = [3, \"s\"]\nlto = [false, \"fat\"]\n");
        apply(&mut config).unwrap();
        assert_eq!(config.matrix.opt_level, vec![OptLevel::Three, OptLevel::Size]);
        assert_eq!(config.matrix.lto, vec![Lto::Off, Lto::Fat]);
    }

    #[test]
    fn a_value_we_do_not_recognise_is_refused_and_named() {
        // Dropping it silently would sweep a matrix the user did not ask for
        // and report results they would read as covering it.
        let (_d, mut config) = config_with("[sweep]\nopt-level = [\"fast\"]\n");
        let error = apply(&mut config).unwrap_err().to_string();
        assert!(error.contains("opt-level"), "{error}");
        assert!(error.contains("fast"), "{error}");
    }

    #[test]
    fn a_benchmark_is_read_and_never_invented() {
        let (_d, mut config) = config_with(
            "[benchmark]\nprogram = \"{artifact}\"\narguments = [\"--bench\"]\nsamples = 20\n",
        );
        apply(&mut config).unwrap();
        let benchmark = config.benchmark.expect("declared");
        assert_eq!(benchmark.program, "{artifact}");
        assert_eq!(benchmark.arguments, ["--bench"]);
        assert_eq!(benchmark.samples, 20);

        // And a project that declares none still has none.
        let (_d, mut bare) = config_with("[sweep]\n");
        apply(&mut bare).unwrap();
        assert!(bare.benchmark.is_none());
    }

    #[test]
    fn a_benchmark_with_no_program_says_so() {
        let (_d, mut config) = config_with("[benchmark]\nsamples = 10\n");
        let error = apply(&mut config).unwrap_err().to_string();
        assert!(error.contains("needs a `program`"), "{error}");
    }

    #[test]
    fn malformed_toml_names_the_file() {
        let (_d, mut config) = config_with("[sweep\nopt-level = ");
        let error = apply(&mut config).unwrap_err().to_string();
        assert!(error.contains(FILE), "{error}");
    }
}
