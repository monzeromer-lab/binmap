//! The provider table's invariants (`A1.4`, `DESIGN-AI §6`).
//!
//! A table is only "a row plus a quirks record" if the rows are consistent.
//! These are the properties the loop and the picker assume and would break
//! silently without.

use binmap_agent::provider::{
    ApiShape, PROVIDERS, ToolCallingSupport, key_present, provider, selectable,
};

#[test]
fn every_provider_is_reachable_by_its_own_id() {
    for spec in PROVIDERS {
        let found = provider(spec.id).unwrap_or_else(|| panic!("{} is unreachable", spec.id));
        assert_eq!(found.id, spec.id);
    }
    assert!(provider("nonesuch").is_none());
}

#[test]
fn provider_ids_are_unique() {
    // Two rows with one id means `provider()` silently returns the first and
    // the second is dead configuration.
    let mut ids: Vec<&str> = PROVIDERS.iter().map(|spec| spec.id).collect();
    ids.sort_unstable();
    let count = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), count, "duplicate provider ids: {ids:?}");
}

#[test]
fn every_provider_offers_at_least_one_model() {
    // A provider with no models is selectable and then cannot be used.
    for spec in PROVIDERS {
        assert!(spec.default_model().is_some(), "{} offers no model", spec.id);
    }
}

#[test]
fn every_model_can_actually_drive_the_loop() {
    // Every step of the Mode A loop is a tool call, so a model that cannot
    // call a tool cannot participate. Better caught here than at step one.
    for spec in PROVIDERS {
        for model in spec.models {
            assert!(
                model.capabilities.can_drive_the_loop(),
                "{}/{} cannot call tools, so it does not belong in the table",
                spec.id,
                model.id
            );
            assert_ne!(model.capabilities.tool_calling, ToolCallingSupport::None);
        }
    }
}

#[test]
fn every_model_declares_a_usable_context_window_and_output_limit() {
    // §6.2: the context strategy reads these rather than assuming. A zero
    // would make every prompt look like an overflow.
    for spec in PROVIDERS {
        for model in spec.models {
            assert!(model.capabilities.context_window > 0, "{}/{}", spec.id, model.id);
            assert!(model.capabilities.max_output > 0, "{}/{}", spec.id, model.id);
            assert!(
                model.capabilities.max_output <= model.capabilities.context_window,
                "{}/{} claims it can emit more than it can hold",
                spec.id,
                model.id
            );
        }
    }
}

#[test]
fn a_cloud_provider_names_the_environment_variable_holding_its_key() {
    for spec in PROVIDERS {
        if spec.cloud {
            assert!(!spec.key_env.is_empty(), "{} is cloud but names no key variable", spec.id);
            assert!(
                spec.default_base_url.starts_with("https://"),
                "{} sends code over the network, so it must use TLS: {}",
                spec.id,
                spec.default_base_url
            );
        }
    }
}

#[test]
fn the_local_provider_is_not_cloud_and_needs_no_key() {
    // §7.3 authors prompts against this one, and §11 is about what leaves the
    // machine. It must be the row that never does.
    let local = provider("local").expect("there is a local provider");
    assert!(!local.cloud);
    assert!(
        key_present(local),
        "a local runner needs no key, so it must never be greyed out for lack of one"
    );
    assert!(local.default_base_url.contains("localhost"), "{}", local.default_base_url);
}

#[test]
fn a_local_model_is_free_rather_than_costing_zero() {
    // `None` and `Some((0.0, 0.0))` mean different things to a reader: "free"
    // versus "we priced it at nothing". The cost meter shows one of them.
    let local = provider("local").unwrap();
    let model = local.default_model().unwrap();
    assert_eq!(model.capabilities.cost_per_mtok, None);
    assert_eq!(model.capabilities.cost_of(1_000_000, 1_000_000), None);
}

#[test]
fn a_priced_model_computes_a_cost_per_million_tokens() {
    let anthropic = provider("anthropic").unwrap();
    let model = anthropic.default_model().unwrap();
    let (input, output) = model.capabilities.cost_per_mtok.expect("a cloud model is priced");

    let cost = model.capabilities.cost_of(1_000_000, 1_000_000).expect("priced");
    assert!(
        (cost - (input + output)).abs() < 1e-9,
        "a million of each should cost input+output, got {cost}"
    );
    assert_eq!(model.capabilities.cost_of(0, 0), Some(0.0), "nothing used costs nothing");
}

#[test]
fn forbidding_cloud_models_makes_them_unselectable_with_a_reason() {
    // §6.3: not merely hidden. A picker that silently omitted them could not
    // explain the absence, and the user would think the app was broken.
    let restricted = selectable(false);
    assert_eq!(restricted.len(), PROVIDERS.len(), "every row is still listed");

    for (spec, refusal) in &restricted {
        if spec.cloud {
            let reason = refusal.expect("a cloud row must carry its reason when forbidden");
            assert!(reason.contains("does not allow"), "{reason}");
        } else {
            assert!(refusal.is_none(), "{} is local and stays available", spec.id);
        }
    }

    // And with cloud allowed, nothing is refused.
    assert!(selectable(true).iter().all(|(_, refusal)| refusal.is_none()));
}

#[test]
fn only_claude_speaks_anthropics_shape() {
    // The simplification the design leans on: two implementations, not five.
    // If a third shape appears, `backend_for` needs a new arm and this test is
    // where that is noticed.
    let anthropic: Vec<&str> = PROVIDERS
        .iter()
        .filter(|spec| spec.shape == ApiShape::Anthropic)
        .map(|spec| spec.id)
        .collect();
    assert_eq!(anthropic, vec!["anthropic"]);

    let compatible = PROVIDERS.iter().filter(|s| s.shape == ApiShape::OpenAiCompatible).count();
    assert!(compatible >= 4, "the OpenAI-compatible shape is meant to cover most of the table");
}

#[test]
fn a_provider_that_will_not_interleave_tools_with_reasoning_asks_for_a_separate_turn() {
    // §8 wants a hypothesis before every call. Where a model will not produce
    // both at once, the loop must ask twice rather than conclude the model
    // refused to hypothesise.
    let deepseek = provider("deepseek").expect("deepseek is in the table");
    assert!(deepseek.quirks.no_tools_with_reasoning);
    assert!(deepseek.quirks.needs_separate_hypothesis_turn());

    let local = provider("local").unwrap();
    assert!(!local.quirks.needs_separate_hypothesis_turn());
}

#[test]
fn a_model_name_that_is_not_offered_is_refused_rather_than_guessed() {
    let local = provider("local").unwrap();
    assert!(local.model("gpt-5").is_none(), "the local runner does not host GPT-5");
    assert!(local.model(local.default_model().unwrap().id).is_some());
}

// --- the table as the picker reads it (`U1.3`) ---

use binmap_core::reasoner::{Mode, Unavailable};

#[test]
fn none_is_always_offered_and_always_selectable() {
    // §9.1: a first-class choice listed beside the others. Every analysis
    // works without a model, so the deterministic path must never be the one
    // that is greyed out.
    for allow_cloud in [true, false] {
        let offered = binmap_agent::provider::reasoners(allow_cloud);
        let none =
            offered.iter().find(|reasoner| reasoner.mode == Mode::None).expect("None is offered");
        assert!(none.is_selectable(), "None must always be choosable");
        assert!(!none.cloud);
    }
}

#[test]
fn forbidding_cloud_greys_out_cloud_rows_but_keeps_them_listed() {
    let offered = binmap_agent::provider::reasoners(false);
    let cloud: Vec<_> = offered.iter().filter(|reasoner| reasoner.cloud).collect();
    assert!(!cloud.is_empty(), "the cloud rows are still listed");

    for reasoner in cloud {
        match &reasoner.unavailable {
            Some(Unavailable::CloudForbidden) => {}
            other => panic!("{} should be cloud-forbidden, got {other:?}", reasoner.id),
        }
        // And the reason must tell the user something true and reassuring.
        let reason = reasoner.unavailable.as_ref().unwrap().describe();
        assert!(reason.contains("leaves the machine"), "{reason}");
    }
}

#[test]
fn cloud_forbidden_outranks_a_missing_key() {
    // Telling someone to set a key for a provider the project will not permit
    // is advice that cannot help.
    let offered = binmap_agent::provider::reasoners(false);
    let anthropic = offered
        .iter()
        .find(|reasoner| reasoner.id.starts_with("anthropic/"))
        .expect("Claude is listed");
    assert_eq!(anthropic.unavailable, Some(Unavailable::CloudForbidden));
}

#[test]
fn claude_says_it_arrives_later_rather_than_failing_at_the_first_call() {
    // Sending Anthropic an OpenAI-shaped body and reporting whatever it
    // returns would blame the user for our missing wire shape.
    let offered = binmap_agent::provider::reasoners(true);
    let anthropic = offered
        .iter()
        .find(|reasoner| reasoner.id.starts_with("anthropic/"))
        .expect("Claude is listed");

    match &anthropic.unavailable {
        Some(Unavailable::NotImplemented { arrives_in }) => assert_eq!(arrives_in, "Phase 1.5"),
        other => panic!("expected a not-implemented reason, got {other:?}"),
    }
}

#[test]
fn the_local_runner_is_selectable_with_no_key_at_all() {
    // The one provider that always works, and the one §7.3 authors against.
    let offered = binmap_agent::provider::reasoners(true);
    let local = offered
        .iter()
        .find(|reasoner| reasoner.id.starts_with("local/"))
        .expect("a local runner is listed");
    assert!(local.is_selectable(), "{:?}", local.unavailable);
    assert_eq!(local.price_label(), "free", "a local model is free, not $0.00");
}

#[test]
fn selecting_an_unavailable_reasoner_is_refused_with_its_reason() {
    // The reason is already known here. Discovering it at the first model call
    // would report a setup problem as a session failure.
    let mut choice = binmap_agent::provider::choice(false);
    let cloud_id = choice
        .available
        .iter()
        .find(|reasoner| reasoner.cloud)
        .expect("a cloud row exists")
        .id
        .clone();

    let error = choice.select(&cloud_id).expect_err("a forbidden row cannot be selected");
    assert!(error.contains("does not allow cloud"), "{error}");
    assert_eq!(choice.selected, "none", "and the selection did not change");
}

#[test]
fn selecting_a_reasoner_that_does_not_exist_says_so() {
    let mut choice = binmap_agent::provider::choice(true);
    assert!(choice.select("nonesuch/model").is_err());
}

#[test]
fn the_default_choice_calls_no_model() {
    // §7's phase table has the product working with no model at all, so a
    // default that reached for one would contradict it.
    let choice = binmap_agent::provider::choice(true);
    assert_eq!(choice.selected, "none");
    assert!(!choice.uses_a_model());

    let mut chosen = choice;
    chosen.select("local/qwen3-coder").expect("the local runner is selectable");
    assert!(chosen.uses_a_model());
}

#[test]
fn every_offered_reasoner_describes_itself_readably() {
    for allow_cloud in [true, false] {
        for reasoner in binmap_agent::provider::reasoners(allow_cloud) {
            let described = reasoner.describe();
            assert!(!described.is_empty(), "{} describes itself as nothing", reasoner.id);
            assert_eq!(described.lines().count(), 1, "{described:?} spans lines");
            assert!(!reasoner.price_label().is_empty(), "{} has no price label", reasoner.id);
        }
    }
}

#[test]
fn a_reasoner_id_survives_into_a_session_unchanged() {
    // The id is what a session records, so it must be stable and unique.
    let offered = binmap_agent::provider::reasoners(true);
    let mut ids: Vec<&str> = offered.iter().map(|reasoner| reasoner.id.as_str()).collect();
    ids.sort_unstable();
    let count = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), count, "duplicate reasoner ids would make a session ambiguous");
}
