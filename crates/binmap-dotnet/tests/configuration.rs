//! The .NET configuration matrix (`TOOLING-DOTNET §3`).

use binmap_dotnet::configuration::{
    Configuration, DeploymentShape, FEATURE_SWITCHES, OptimizationPreference, UNVERIFIED,
    deployment_shapes, feature_switches,
};

#[test]
fn the_flagship_sweep_is_every_deployment_shape() {
    // §3.5: "What would going AOT actually cost and save us?" — the question
    // that justifies the backend, answered as a frontier rather than a number.
    let shapes = deployment_shapes();
    assert_eq!(shapes.len(), DeploymentShape::ALL.len());
    assert!(shapes.iter().any(|c| c.shape == DeploymentShape::NativeAot));
    assert!(shapes.iter().any(|c| c.shape == DeploymentShape::FrameworkDependent));
}

#[test]
fn nativeaot_asks_for_the_accounting_the_backend_depends_on() {
    // §2: without the mstat and dgml files there is no attribution at all,
    // and the fallback is the symbol-table mangling §2 says not to touch.
    let properties = DeploymentShape::NativeAot.properties();
    let names: Vec<&str> = properties.iter().map(|(name, _)| *name).collect();

    assert!(names.contains(&"PublishAot"));
    assert!(names.contains(&"IlcGenerateMstatFile"), "{names:?}");
    assert!(names.contains(&"IlcGenerateDgmlFile"), "{names:?}");
    // And per-site warnings, or §5's findings collapse to one line per
    // assembly.
    assert!(names.contains(&"TrimmerSingleWarn"), "{names:?}");
}

#[test]
fn only_nativeaot_can_produce_the_dependency_graph() {
    for shape in DeploymentShape::ALL {
        let configuration = Configuration { shape, disabled: Vec::new(), optimization: None };
        assert_eq!(
            configuration.produces_the_dependency_graph(),
            shape == DeploymentShape::NativeAot,
            "{shape:?}"
        );
    }
}

#[test]
fn every_deployment_shape_states_what_it_costs_in_something_other_than_bytes() {
    // The smallest deployment is frequently the hardest to debug or the
    // slowest to build, and a size tool that hides that helps someone make a
    // mistake.
    for shape in DeploymentShape::ALL {
        assert!(!shape.costs().is_empty(), "{shape:?} claims to cost nothing");
        assert!(!shape.label().is_empty());
    }
    assert!(
        DeploymentShape::Trimmed.costs().contains("Reflection"),
        "{}",
        DeploymentShape::Trimmed.costs()
    );
    assert!(
        DeploymentShape::NativeAot.costs().contains("do not work"),
        "{}",
        DeploymentShape::NativeAot.costs()
    );
}

#[test]
fn every_feature_switch_says_what_stops_working() {
    // §3.2 calls these "the folklore knobs nobody measures", and a switch
    // shown with only its saving is one someone will flip and regret.
    assert!(FEATURE_SWITCHES.len() >= 8, "the list is the point");
    for switch in FEATURE_SWITCHES {
        assert!(!switch.costs.is_empty(), "{} claims to cost nothing", switch.property);
        assert!(!switch.property.is_empty());
        assert!(
            switch.disabling_value == "true" || switch.disabling_value == "false",
            "{}",
            switch.property
        );
    }
}

#[test]
fn the_switch_that_saves_most_says_so_and_says_why_that_is_dangerous() {
    // `InvariantGlobalization` removes megabytes of ICU data and the ability
    // to sort a Turkish string correctly.
    let invariant = FEATURE_SWITCHES
        .iter()
        .find(|switch| switch.property == "InvariantGlobalization")
        .expect("it is in the table");
    assert!(invariant.costs.contains("ICU"), "{}", invariant.costs);
    assert!(invariant.costs.contains("Turkish"), "a concrete consequence: {}", invariant.costs);
}

#[test]
fn stack_trace_support_names_the_trade_the_design_singles_out() {
    // §3.2: "diagnosability for size, which is precisely the kind of trade a
    // tool should quantify rather than a developer guess at".
    let switch = FEATURE_SWITCHES
        .iter()
        .find(|switch| switch.property == "StackTraceSupport")
        .expect("it is in the table");
    assert!(switch.costs.contains("stack traces"), "{}", switch.costs);
}

#[test]
fn feature_switches_are_swept_one_at_a_time() {
    // The combinations multiply beyond what anyone will wait for, and the
    // question a reader has is what each knob is worth.
    let configurations = feature_switches();
    assert_eq!(configurations.len(), FEATURE_SWITCHES.len() + 1, "plus the baseline");
    assert!(configurations.iter().all(|c| c.disabled.len() <= 1));
    assert!(configurations.iter().all(|c| c.shape == DeploymentShape::NativeAot));
}

#[test]
fn the_baseline_asserts_nothing_the_toolchain_would_not_do_anyway() {
    // A baseline that quietly turned something off would make every number in
    // the product flattering — which the native side learned the hard way.
    let baseline = Configuration::baseline();
    assert!(baseline.disabled.is_empty());
    assert!(baseline.optimization.is_none());
    assert_eq!(baseline.shape, DeploymentShape::FrameworkDependent);
}

#[test]
fn a_configuration_becomes_msbuild_arguments_rather_than_a_project_edit() {
    // The rule every backend here shares: a sweep that edited someone's
    // .csproj and then crashed would leave their project broken.
    let configuration = Configuration {
        shape: DeploymentShape::NativeAot,
        disabled: vec!["InvariantGlobalization"],
        optimization: Some(OptimizationPreference::Size),
    };
    let arguments = configuration.arguments();

    assert!(arguments.iter().all(|argument| argument.starts_with("-p:")), "{arguments:?}");
    assert!(arguments.contains(&"-p:PublishAot=true".to_string()));
    assert!(arguments.contains(&"-p:InvariantGlobalization=true".to_string()));
    assert!(arguments.contains(&"-p:IlcOptimizationPreference=Size".to_string()));
}

#[test]
fn a_configuration_reports_every_trade_it_makes_not_only_the_shapes() {
    let configuration = Configuration {
        shape: DeploymentShape::NativeAot,
        disabled: vec!["InvariantGlobalization", "StackTraceSupport"],
        optimization: None,
    };
    let costs = configuration.costs();
    assert_eq!(costs.len(), 3, "the shape and both switches: {costs:?}");
    assert!(costs.iter().any(|cost| cost.contains("ICU")));
    assert!(costs.iter().any(|cost| cost.contains("stack traces")));
}

#[test]
fn every_configuration_has_a_distinct_name() {
    // Names are directory names and session keys, so a collision would make
    // one configuration measure another.
    let mut all = deployment_shapes();
    all.extend(feature_switches());
    let names: std::collections::BTreeSet<String> = all.iter().map(Configuration::name).collect();
    // The NativeAOT baseline appears in both lists, which is the one overlap.
    assert_eq!(names.len(), all.len() - 1, "{names:?}");
}

#[test]
fn properties_the_design_marks_unverified_are_listed_rather_than_left_implicit() {
    // §3 marks several with ⚠ because the names have changed across SDKs. A
    // reader should know which to check rather than discovering it from a
    // build failure.
    assert!(UNVERIFIED.contains(&"IlcGenerateDgmlFile"));
    assert!(UNVERIFIED.contains(&"TrimmerSingleWarn"));
    assert!(UNVERIFIED.contains(&"StackTraceSupport"));

    // And every unverified property that the matrix actually emits is in the
    // list, or the list is decoration.
    let emitted: std::collections::BTreeSet<String> = deployment_shapes()
        .iter()
        .chain(feature_switches().iter())
        .flat_map(Configuration::arguments)
        .filter_map(|argument| argument.strip_prefix("-p:")?.split('=').next().map(str::to_string))
        .collect();
    for property in ["IlcGenerateDgmlFile", "IlcGenerateMstatFile", "TrimmerSingleWarn"] {
        assert!(emitted.contains(property), "{property} is never emitted");
        assert!(UNVERIFIED.contains(&property), "{property} is emitted but not flagged");
    }
}
