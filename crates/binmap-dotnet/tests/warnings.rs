//! Trim and AOT warnings as findings (`TOOLING-DOTNET §5`).
//!
//! The lines are MSBuild's documented format. As with the DGML reader, there
//! is no .NET SDK on this machine, so these are the shapes the documentation
//! describes rather than ones observed from a real build — stated here rather
//! than implied.

use binmap_dotnet::warnings::{Kind, parse};

const BUILD_OUTPUT: &str = r#"
Determining projects to restore...
  App -> /home/x/App/bin/Release/net9.0/App.dll
/home/x/App/Serializer.cs(42,13): warning IL2026: Using member 'System.Text.Json.JsonSerializer.Serialize(Object, Type)' which has 'RequiresUnreferencedCodeAttribute' can break functionality when trimming. [/home/x/App/App.csproj]
/home/x/App/Serializer.cs(42,13): warning IL3050: Using member 'System.Text.Json.JsonSerializer.Serialize(Object, Type)' which has 'RequiresDynamicCodeAttribute' may break functionality when AOT compiling. [/home/x/App/App.csproj]
/home/x/App/Plugins.cs(17,9): warning IL2057: Unrecognized value passed to the parameter 'typeName' of method 'System.Type.GetType(String)'. [/home/x/App/App.csproj]
  App -> published
Build succeeded.
"#;

#[test]
fn every_warning_becomes_a_finding_with_a_location() {
    // §5: "a source location, a deterministic origin, and a known class of
    // remedy" — the combination is what makes it actionable.
    let warnings = parse(BUILD_OUTPUT);
    assert_eq!(warnings.warnings.len(), 3, "{:?}", warnings.warnings);

    for warning in &warnings.warnings {
        assert!(warning.file.is_some(), "{warning:?}");
        assert!(warning.line.is_some(), "{warning:?}");
        assert!(!warning.remedy().is_empty(), "{} offers no remedy", warning.code);
        assert!(!warning.message.is_empty());
    }
}

#[test]
fn aot_warnings_come_before_trim_ones() {
    // One breaks the published binary and the other makes it bigger, so a
    // list sorted the other way puts the survivable problem first.
    let warnings = parse(BUILD_OUTPUT);
    assert_eq!(warnings.warnings[0].kind, Kind::Aot, "{:?}", warnings.warnings[0]);
    assert!(warnings.blocks_aot());
}

#[test]
fn the_two_classes_are_told_apart_by_their_code() {
    assert_eq!(Kind::of("IL2026"), Some(Kind::Trim));
    assert_eq!(Kind::of("IL3050"), Some(Kind::Aot));
    assert_eq!(Kind::of("CS0168"), None, "a C# compiler warning is not one of these");
    assert_eq!(Kind::of("IL"), None);
    assert_eq!(Kind::of("nonsense"), None);
}

#[test]
fn an_aot_warning_says_the_build_will_fail_rather_than_merely_grow() {
    // The distinction that decides whether someone stops what they are doing.
    assert!(Kind::Aot.costs().contains("will fail"), "{}", Kind::Aot.costs());
    assert!(
        Kind::Trim.costs().contains("happens in production"),
        "and a trim warning's worse failure is a runtime one: {}",
        Kind::Trim.costs()
    );
}

#[test]
fn the_member_a_warning_is_about_is_extracted() {
    // ILC quotes it, which is the only reliable marker — the messages
    // themselves are prose and change between versions.
    let warnings = parse(BUILD_OUTPUT);
    let json =
        warnings.warnings.iter().find(|warning| warning.code == "IL3050").expect("the AOT warning");
    assert_eq!(
        json.member.as_deref(),
        Some("System.Text.Json.JsonSerializer.Serialize(Object, Type)")
    );
}

#[test]
fn the_remedy_for_a_known_code_names_the_actual_route_out() {
    let warnings = parse(BUILD_OUTPUT);

    let requires_dynamic = warnings.warnings.iter().find(|w| w.code == "IL3050").expect("IL3050");
    assert!(
        requires_dynamic.remedy().contains("JsonSerializerContext"),
        "the common source-generated replacement: {}",
        requires_dynamic.remedy()
    );

    let unrecognized = warnings.warnings.iter().find(|w| w.code == "IL2057").expect("IL2057");
    assert!(
        unrecognized.remedy().contains("DynamicallyAccessedMembers"),
        "{}",
        unrecognized.remedy()
    );
}

#[test]
fn a_code_with_no_specific_remedy_still_gets_its_classs_advice() {
    let warnings =
        parse("/x/A.cs(1,1): warning IL2999: something new in a later SDK. [/x/A.csproj]");
    assert_eq!(warnings.warnings.len(), 1);
    assert!(!warnings.warnings[0].remedy().is_empty());
    assert!(warnings.warnings[0].remedy().contains("source generator"));
}

#[test]
fn aggregated_warnings_are_recognised_and_the_setting_named() {
    // §5: the default collapses every site in an assembly into one line, and
    // a project that has not turned it off gets a short, useless list.
    let aggregated = parse(
        "App.dll: warning IL2104: Assembly 'App' produced trim warnings. [/x/App.csproj]\n\
         Other.dll: warning IL2104: Assembly 'Other' produced trim warnings. [/x/App.csproj]",
    );

    assert!(aggregated.looks_aggregated);
    assert!(
        aggregated.describe().contains("TrimmerSingleWarn"),
        "the setting is named: {}",
        aggregated.describe()
    );
}

#[test]
fn per_site_warnings_are_not_mistaken_for_aggregated_ones() {
    let detailed = parse(BUILD_OUTPUT);
    assert!(!detailed.looks_aggregated);
    assert!(!detailed.describe().contains("TrimmerSingleWarn"));
}

#[test]
fn a_build_log_full_of_other_things_yields_only_warnings() {
    // A build log contains plenty that is not a warning, and a line that does
    // not match is skipped rather than guessed at.
    let noisy = parse(
        "Determining projects to restore...\n\
         error CS0103: The name 'x' does not exist\n\
         /x/A.cs(3,1): warning CS0168: variable declared but never used\n\
         /x/A.cs(9,1): warning IL2026: Using member 'Foo.Bar()' which has ... [/x/A.csproj]\n\
         Build succeeded.",
    );
    assert_eq!(noisy.warnings.len(), 1, "{:?}", noisy.warnings);
    assert_eq!(noisy.warnings[0].code, "IL2026");
}

#[test]
fn a_warning_with_no_position_still_records_its_file() {
    let warnings = parse("App.dll: warning IL3053: AOT analysis found a problem.");
    assert_eq!(warnings.warnings.len(), 1);
    assert_eq!(warnings.warnings[0].file.as_deref(), Some("App.dll"));
    assert_eq!(warnings.warnings[0].line, None);
}

#[test]
fn a_warning_with_a_line_but_no_column_is_read() {
    let warnings = parse("/x/A.cs(42): warning IL2026: Using member 'Foo.Bar()'.");
    assert_eq!(warnings.warnings[0].line, Some(42));
    assert_eq!(warnings.warnings[0].column, None);
}

#[test]
fn nothing_in_produces_nothing_out_rather_than_a_false_positive() {
    let empty = parse("");
    assert!(empty.warnings.is_empty());
    assert!(!empty.looks_aggregated, "no warnings is not aggregated warnings");
    assert!(!empty.blocks_aot());
}

#[test]
fn every_warning_describes_itself_in_one_readable_line() {
    for warning in parse(BUILD_OUTPUT).warnings {
        let described = warning.describe();
        assert_eq!(described.lines().count(), 1, "{described:?}");
        assert!(described.contains(&warning.code));
    }
}

#[test]
fn msbuilds_project_trailer_is_stripped_from_the_message() {
    // `[/path/to/the.csproj]` is appended to every warning in a multi-project
    // build. It is identical on every line, so it is noise in a list and
    // carries nothing the file path does not.
    let warnings = parse("/x/A.cs(9,1): warning IL2026: Using member 'Foo.Bar()'. [/x/App.csproj]");
    assert_eq!(warnings.warnings[0].message, "Using member 'Foo.Bar()'.");
    assert!(!warnings.warnings[0].describe().contains(".csproj"));
}

#[test]
fn a_message_that_legitimately_ends_in_a_bracket_is_not_truncated() {
    // A generic type's message ends in `]`, and stripping blindly would eat
    // part of the type name.
    let warnings = parse("/x/A.cs(9,1): warning IL2026: Using member 'Foo.Bar(System.Int32[])'");
    assert!(
        warnings.warnings[0].message.ends_with("System.Int32[])'"),
        "{}",
        warnings.warnings[0].message
    );
}
