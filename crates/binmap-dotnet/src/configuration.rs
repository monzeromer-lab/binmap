//! The .NET configuration matrix (`TOOLING-DOTNET §3`).
//!
//! "The richest of the three languages", and `§3.5` names the reason it is
//! worth sweeping rather than arguing about:
//!
//! > **"What would going AOT actually cost and save us?"** … For a company
//! > running C# services, that is a decision currently made by argument.
//! > Turning it into a measurement may justify the tool internally on its own.
//!
//! So `DeploymentShape` is the first axis, and it is the one a sweep should
//! run even if it runs nothing else: framework-dependent through NativeAOT is
//! the question, and the answer is a frontier rather than a number.
//!
//! The feature switches are the other half. `§3.2` calls them "exactly the
//! folklore knobs nobody measures on their own application", and each trades
//! functionality for size — `InvariantGlobalization` removes megabytes of ICU
//! data and the ability to sort a Turkish string correctly. A tool that
//! reported only the megabytes would be helping someone make a mistake, so
//! every switch here carries what it costs.
//!
//! ⚠ `§3` marks several property names as unverified against a specific SDK.
//! They are transcribed as documented and named in `UNVERIFIED` so a reader
//! knows which ones to check rather than discovering it from a build failure.

use serde::{Deserialize, Serialize};

/// How the application is published.
///
/// The flagship axis. Ordered from smallest deployment effort to largest,
/// which is roughly the order of increasing build time and decreasing size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DeploymentShape {
    /// The runtime is expected to be installed already.
    FrameworkDependent,
    /// The runtime ships with the application.
    SelfContained,
    /// Self-contained, with unused code removed.
    Trimmed,
    /// Trimmed and precompiled: larger on disk, faster to start.
    ReadyToRun,
    /// Compiled to a native binary. No runtime, no JIT.
    NativeAot,
}

impl DeploymentShape {
    pub const ALL: [Self; 5] = [
        Self::FrameworkDependent,
        Self::SelfContained,
        Self::Trimmed,
        Self::ReadyToRun,
        Self::NativeAot,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::FrameworkDependent => "framework-dependent",
            Self::SelfContained => "self-contained",
            Self::Trimmed => "trimmed",
            Self::ReadyToRun => "ReadyToRun",
            Self::NativeAot => "NativeAOT",
        }
    }

    /// The MSBuild properties this shape sets.
    ///
    /// Passed on the command line rather than written into the project, for
    /// the reason every backend here has the same rule: a sweep that edited
    /// someone's `.csproj` and then crashed would leave their project broken.
    pub fn properties(self) -> Vec<(&'static str, &'static str)> {
        match self {
            Self::FrameworkDependent => vec![("SelfContained", "false")],
            Self::SelfContained => vec![("SelfContained", "true")],
            Self::Trimmed => {
                vec![("SelfContained", "true"), ("PublishTrimmed", "true"), ("TrimMode", "full")]
            }
            Self::ReadyToRun => vec![
                ("SelfContained", "true"),
                ("PublishTrimmed", "true"),
                ("PublishReadyToRun", "true"),
            ],
            Self::NativeAot => vec![
                ("PublishAot", "true"),
                // The accounting `§2` depends on. Without these the backend
                // has no attribution at all and falls back to symbols, which
                // is precisely what `§2` says not to do.
                ("IlcGenerateMstatFile", "true"),
                ("IlcGenerateDgmlFile", "true"),
                // Per-site trim warnings rather than one line per assembly.
                ("TrimmerSingleWarn", "false"),
            ],
        }
    }

    /// What this shape costs, in terms that are not bytes.
    ///
    /// The trade a size tool must not hide: the smallest deployment is
    /// frequently the one that is hardest to debug or slowest to build.
    pub fn costs(self) -> &'static str {
        match self {
            Self::FrameworkDependent => {
                "Needs a matching runtime installed on the machine that runs it. Smallest to \
                 ship, most to assume."
            }
            Self::SelfContained => {
                "Bundles the whole runtime, which is tens of megabytes, in exchange for \
                 depending on nothing."
            }
            Self::Trimmed => {
                "Removes code the analyser cannot prove is reachable — which is not the same as \
                 code that is unreachable. Reflection is where this breaks, at runtime."
            }
            Self::ReadyToRun => {
                "Larger on disk for a faster start. The native code is in addition to the IL, \
                 not instead of it."
            }
            Self::NativeAot => {
                "No JIT, so no runtime code generation and no reflection over what was trimmed \
                 away. Build times rise substantially, and some libraries simply do not work."
            }
        }
    }
}

/// A feature switch: a subsystem traded for size.
/// Serialize only, for the same reason as `Configuration`: this is a
/// compile-time table of `&'static str`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FeatureSwitch {
    pub property: &'static str,
    /// The value that *removes* the feature.
    pub disabling_value: &'static str,
    /// What stops working. Never empty, because a switch shown with only its
    /// saving is one someone will flip and regret.
    pub costs: &'static str,
}

/// `§3.2`'s switches, each with what it costs.
///
/// These are "exactly the folklore knobs nobody measures on their own
/// application", which is the case for sweeping them and the reason each needs
/// its trade stated rather than assumed.
pub const FEATURE_SWITCHES: &[FeatureSwitch] = &[
    FeatureSwitch {
        property: "InvariantGlobalization",
        disabling_value: "true",
        costs: "Removes several megabytes of ICU data, and with it correct casing, sorting and \
                comparison for every culture except the invariant one. A Turkish dotted i stops \
                behaving. The biggest single saving here and the one most likely to be regretted.",
    },
    FeatureSwitch {
        property: "UseSystemResourceKeys",
        disabling_value: "true",
        costs: "Exception messages become resource keys rather than sentences. Smaller, and \
                every stack trace in production gets harder to read.",
    },
    FeatureSwitch {
        property: "EventSourceSupport",
        disabling_value: "false",
        costs: "No EventSource telemetry, which most diagnostics tooling is built on.",
    },
    FeatureSwitch {
        property: "StackTraceSupport",
        disabling_value: "false",
        costs: "Exceptions stop carrying usable stack traces. This is diagnosability traded \
                directly for bytes, and is exactly the trade a tool should quantify rather than \
                a developer guess at.",
    },
    FeatureSwitch {
        property: "DebuggerSupport",
        disabling_value: "false",
        costs: "A debugger cannot attach to the published build.",
    },
    FeatureSwitch {
        property: "MetadataUpdaterSupport",
        disabling_value: "false",
        costs: "No hot reload. Irrelevant in production and missed in development.",
    },
    FeatureSwitch {
        property: "HttpActivityPropagationSupport",
        disabling_value: "false",
        costs: "Distributed tracing stops propagating across HTTP calls, so spans appear \
                unparented.",
    },
    FeatureSwitch {
        property: "UseNativeHttpHandler",
        disabling_value: "true",
        costs: "Uses the platform's HTTP stack instead of the managed one. Smaller, with \
                different behaviour for proxies and certificates.",
    },
    FeatureSwitch {
        property: "NullabilityInfoContextSupport",
        disabling_value: "false",
        costs: "Reflection can no longer read nullability annotations, which some serialisers \
                and validators rely on.",
    },
];

/// ILC's own optimisation knobs (`§3.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptimizationPreference {
    Speed,
    Size,
    Blended,
}

impl OptimizationPreference {
    pub const ALL: [Self; 3] = [Self::Speed, Self::Size, Self::Blended];

    pub fn value(self) -> &'static str {
        match self {
            Self::Speed => "Speed",
            Self::Size => "Size",
            Self::Blended => "Blended",
        }
    }
}

/// Properties `§3` marks as unverified against a specific SDK.
///
/// Listed rather than left implicit so a reader knows which to check against
/// their own toolchain instead of discovering it from a build failure. The
/// design marks these with ⚠ for the same reason.
pub const UNVERIFIED: &[&str] = &[
    "IlcGenerateMstatFile",
    "IlcGenerateDgmlFile",
    "IlcOptimizationPreference",
    "IlcFoldVirtualMethodBodies",
    "IlcDisableReflection",
    "StackTraceSupport",
    "TrimmerSingleWarn",
];

/// One point in the matrix.
/// Serialize but not Deserialize: `disabled` names switches from the static
/// table, so a configuration that could be read back would be one a config
/// file could invent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Configuration {
    pub shape: DeploymentShape,
    /// Switches to disable, by property name.
    pub disabled: Vec<&'static str>,
    pub optimization: Option<OptimizationPreference>,
}

impl Configuration {
    /// The baseline: whatever the project already does.
    ///
    /// As on every other backend, it asserts nothing the toolchain would not
    /// have done anyway. A baseline that quietly turned something off would
    /// make every number in the product flattering.
    pub fn baseline() -> Self {
        Self {
            shape: DeploymentShape::FrameworkDependent,
            disabled: Vec::new(),
            optimization: None,
        }
    }

    /// The MSBuild arguments this becomes.
    pub fn arguments(&self) -> Vec<String> {
        let mut arguments: Vec<String> = self
            .shape
            .properties()
            .into_iter()
            .map(|(name, value)| format!("-p:{name}={value}"))
            .collect();

        for property in &self.disabled {
            let switch = FEATURE_SWITCHES
                .iter()
                .find(|switch| switch.property == *property)
                .map(|switch| switch.disabling_value)
                .unwrap_or("false");
            arguments.push(format!("-p:{property}={switch}"));
        }

        if let Some(optimization) = self.optimization {
            arguments.push(format!("-p:IlcOptimizationPreference={}", optimization.value()));
        }
        arguments
    }

    /// A stable name, used as a directory and a session key.
    pub fn name(&self) -> String {
        let mut name = self.shape.label().replace(' ', "-").to_lowercase();
        for property in &self.disabled {
            name.push('-');
            name.push_str(&property.to_lowercase());
        }
        if let Some(optimization) = self.optimization {
            name.push('-');
            name.push_str(&optimization.value().to_lowercase());
        }
        name
    }

    /// Everything this configuration gives up, so a frontier can show the
    /// trade rather than only the saving.
    pub fn costs(&self) -> Vec<&'static str> {
        let mut costs = vec![self.shape.costs()];
        for property in &self.disabled {
            if let Some(switch) =
                FEATURE_SWITCHES.iter().find(|switch| switch.property == *property)
            {
                costs.push(switch.costs);
            }
        }
        costs
    }

    /// Whether this configuration can produce the ILC accounting `§2` needs.
    pub fn produces_the_dependency_graph(&self) -> bool {
        self.shape == DeploymentShape::NativeAot
    }
}

/// The flagship sweep (`§3.5`).
///
/// Every deployment shape and nothing else — one run answering "what would
/// going AOT actually cost and save us", which is the question that justifies
/// the backend.
pub fn deployment_shapes() -> Vec<Configuration> {
    DeploymentShape::ALL
        .iter()
        .map(|shape| Configuration { shape: *shape, disabled: Vec::new(), optimization: None })
        .collect()
}

/// NativeAOT with each feature switch tried on its own.
///
/// One at a time rather than in combination: the combinations multiply beyond
/// what anyone will wait for, and the question a reader has is what each knob
/// is worth, not what all five hundred subsets are worth.
pub fn feature_switches() -> Vec<Configuration> {
    let mut configurations = vec![Configuration {
        shape: DeploymentShape::NativeAot,
        disabled: Vec::new(),
        optimization: None,
    }];
    configurations.extend(FEATURE_SWITCHES.iter().map(|switch| Configuration {
        shape: DeploymentShape::NativeAot,
        disabled: vec![switch.property],
        optimization: None,
    }));
    configurations
}
