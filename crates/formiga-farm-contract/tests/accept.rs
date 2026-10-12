//! What Desktop keeps of a proposal, and that keeping it changes a creature's look and nothing
//! else about it.

use formiga_core::forms::{Design, Form, Ink, Marking, MarkingKind, Part, PartKind, Plan, Sculpt};
use formiga_farm_contract::*;
use time::Duration;

fn seal_of(snapshot: &FarmSnapshot) -> SessionSeal {
    SessionSeal::of(snapshot, &encode(snapshot).unwrap())
}

fn sculpted(plan: Plan) -> Design {
    let mut sculpt = Sculpt::starter(plan);
    sculpt.coat.primary = [0x30, 0x60, 0x90];
    sculpt
        .markings
        .push(Marking::new(MarkingKind::Stripes, [0x10, 0x10, 0x10]));
    sculpt.fit(Part::new(PartKind::BackTallDorsal, Ink::Primary));
    Design {
        form: Form::Sculpted {
            sculpt: sculpt.normalized(),
        },
        face: formiga_core::forms::plain_face(),
    }
}

fn edit(snapshot: &FarmSnapshot, serial: u32, design: Design) -> FarmProposal {
    let creature = snapshot.creature().unwrap();
    FarmProposal::new(
        &seal_of(snapshot),
        serial,
        sample::OPENED + Duration::minutes(5),
        "0.1.0",
        ProposalKind::EditExisting {
            target: creature.id,
            expected_revision: creature.revision.clone(),
        },
        design,
        Lineage::new(Some("animal.orca"), Some("Splash")),
    )
}

/// Everything about a creature but its look, as JSON, for comparing.
fn all_but_its_look(creature: &formiga_core::Creature) -> serde_json::Value {
    let mut value = serde_json::to_value(creature).unwrap();
    value["appearance"]["design"] = serde_json::Value::Null;
    value["appearance"]["face"] = serde_json::Value::Null;
    value["appearance"]["sculpt"] = serde_json::Value::Null;
    value["origin"]["design"] = serde_json::Value::Null;
    value
}

#[test]
fn a_radically_different_body_keeps_everything_that_makes_it_who_it_is() {
    let mut save = sample::colony();
    let snapshot = sample::edit_snapshot(0);
    let before_save = save.clone();
    let proposal = edit(&snapshot, 1, sculpted(Plan::Floater));
    let accepted = accept_proposal(
        &seal_of(&snapshot),
        &snapshot,
        &proposal,
        Current::Creature {
            creature: &save.creatures[0],
            available: true,
        },
    )
    .expect("kept");
    let Accepted::Edit { target, design } = accepted else {
        panic!("an edit")
    };
    assert_eq!(target, save.creatures[0].id);
    let creature = save.creatures.iter_mut().find(|c| c.id == target).unwrap();
    let before = creature.clone();
    apply_design(creature, &design);
    assert!(creature.appearance.sculpt.is_some());
    // Its look is the proposed one, exactly.
    assert_eq!(Design::of(&creature.appearance), design);
    // And nothing else about it changed: id, name, temperament, family, habits, memories.
    assert_eq!(all_but_its_look(creature), all_but_its_look(&before));
    // An older reader draws a companion recipe in its colours.
    let fallback = creature.appearance.design.expect("a fallback recipe");
    assert_eq!(fallback, fallback.bounded());
    assert_eq!(fallback.body, formiga_core::BodyPlan::Round);
    // Nor about anyone else, nor the colony's bonds.
    assert_eq!(save.relationships, before_save.relationships);
    assert_eq!(save.creatures[1..], before_save.creatures[1..]);
}

/// A creature already reshaped opens with the sculpt Desktop keeps in its save, beside the
/// recipe a trip carries, so the next change is made from the form it really has and is not
/// taken for stale.
#[test]
fn a_sculpted_creature_opens_as_its_sculpt_and_can_be_reshaped_again() {
    let mut save = sample::colony();
    let first = sculpted(Plan::Floater);
    apply_design(&mut save.creatures[0], &first);
    let snapshot = project_edit(
        &save,
        save.creatures[0].id,
        true,
        SessionId::parse(sample::EDIT_SESSION).unwrap(),
        sample::OPENED,
        sample::DESKTOP_VERSION,
        sample::capabilities(),
        Renderer::this_build(),
    )
    .expect("projects");
    let opened = snapshot.creature().unwrap();
    assert_eq!(opened.sculpt.as_ref(), first.form.sculpt());
    assert_eq!(opened.design().unwrap(), first);
    assert_eq!(opened.revision, first.revision());
    assert!(decode::<FarmSnapshot>(&encode(&snapshot).unwrap()).is_ok());
    // A trip still carries only the recipe beside it.
    assert_eq!(
        opened.appearance.to_genome().unwrap().design,
        save.creatures[0].appearance.design
    );
    let second = sculpted(Plan::Crawler);
    let proposal = edit(&snapshot, 1, second.clone());
    let Ok(Accepted::Edit { design, .. }) = accept_proposal(
        &seal_of(&snapshot),
        &snapshot,
        &proposal,
        Current::Creature {
            creature: &save.creatures[0],
            available: true,
        },
    ) else {
        panic!("kept")
    };
    apply_design(&mut save.creatures[0], &design);
    assert_eq!(Design::of(&save.creatures[0].appearance), second);
    // And back to a companion recipe lets the sculpt go.
    let recipe = sample::colony().creatures[0].appearance.design.unwrap();
    apply_design(
        &mut save.creatures[0],
        &Design {
            form: Form::Companion { recipe },
            face: second.face,
        },
    );
    assert_eq!(save.creatures[0].appearance.sculpt, None);
    assert_eq!(save.creatures[0].appearance.design, Some(recipe));
}

#[test]
fn a_change_made_from_a_look_the_creature_no_longer_has_is_stale() {
    let mut save = sample::colony();
    let snapshot = sample::edit_snapshot(0);
    // Meanwhile the creature's look changed.
    let mut changed = save.creatures[0].appearance.design.unwrap();
    changed.coat = [1, 2, 3];
    formiga_core::apply_creature_design(&mut save.creatures[0], Some(changed));
    let proposal = edit(&snapshot, 1, sculpted(Plan::Upright));
    let rejection = accept_proposal(
        &seal_of(&snapshot),
        &snapshot,
        &proposal,
        Current::Creature {
            creature: &save.creatures[0],
            available: true,
        },
    )
    .unwrap_err();
    let Rejection::Stale { current, revision } = &rejection else {
        panic!("{rejection:?}")
    };
    assert_eq!(current.form.recipe(), Some(changed.bounded()));
    assert_eq!(current.revision(), *revision);
    // Farm is told what it looks like now.
    let verdict = FarmVerdict::new(&seal_of(&snapshot), 1, sample::OPENED, rejection.verdict());
    assert!(decode::<FarmVerdict>(&encode(&verdict).unwrap()).is_ok());
}

#[test]
fn nothing_is_kept_for_a_creature_that_is_away_or_gone() {
    let save = sample::colony();
    let snapshot = sample::edit_snapshot(1);
    let proposal = edit(&snapshot, 1, sculpted(Plan::Percher));
    let seal = seal_of(&snapshot);
    let away = Current::Creature {
        creature: &save.creatures[1],
        available: false,
    };
    assert_eq!(
        accept_proposal(&seal, &snapshot, &proposal, away),
        Err(Rejection::Unavailable)
    );
    assert_eq!(
        accept_proposal(&seal, &snapshot, &proposal, Current::Gone),
        Err(Rejection::Gone)
    );
}

#[test]
fn a_proposal_from_another_session_or_snapshot_changes_nothing() {
    let save = sample::colony();
    let snapshot = sample::edit_snapshot(0);
    let proposal = edit(&snapshot, 1, sculpted(Plan::Crawler));
    let current = Current::Creature {
        creature: &save.creatures[0],
        available: true,
    };
    let other = sample::edit_snapshot(1);
    assert_eq!(
        accept_proposal(&seal_of(&other), &other, &proposal, current),
        Err(Rejection::WrongSession)
    );
}

#[test]
fn a_sculpted_form_is_refused_by_a_desktop_that_cannot_draw_it() {
    let save = sample::colony();
    let mut snapshot = sample::edit_snapshot(0);
    snapshot
        .capabilities
        .retain(|c| *c != FarmCapability::SculptedForms);
    let proposal = edit(&snapshot, 1, sculpted(Plan::TallQuadruped));
    let current = Current::Creature {
        creature: &save.creatures[0],
        available: true,
    };
    assert_eq!(
        accept_proposal(&seal_of(&snapshot), &snapshot, &proposal, current),
        Err(Rejection::Unsupported)
    );
    // A companion recipe is still kept.
    let mut recipe = save.creatures[0].appearance.design.unwrap();
    recipe.coat = [200, 100, 50];
    let companion = Design {
        form: Form::Companion { recipe },
        face: save.creatures[0].appearance.face,
    };
    let proposal = edit(&snapshot, 2, companion);
    assert!(accept_proposal(&seal_of(&snapshot), &snapshot, &proposal, current).is_ok());
    // And a plan Desktop does not name is refused even where sculpts are kept.
    let mut snapshot = sample::edit_snapshot(0);
    snapshot.renderer.plans.retain(|p| *p != Plan::Floater);
    let proposal = edit(&snapshot, 1, sculpted(Plan::Floater));
    assert_eq!(
        accept_proposal(&seal_of(&snapshot), &snapshot, &proposal, current),
        Err(Rejection::Unsupported)
    );
}

#[test]
fn a_malformed_proposal_is_refused_on_reading_and_on_keeping() {
    let save = sample::colony();
    let snapshot = sample::edit_snapshot(0);
    let mut proposal = edit(&snapshot, 1, sculpted(Plan::Floater));
    if let Form::Sculpted { sculpt } = &mut proposal.design.form {
        sculpt.shape.head = 99;
    }
    assert!(encode(&proposal).is_err(), "a writer refuses it");
    let mut value = serde_json::to_value(&proposal).unwrap();
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(
        decode::<FarmProposal>(&bytes).is_err(),
        "a reader refuses it"
    );
    let current = Current::Creature {
        creature: &save.creatures[0],
        available: true,
    };
    assert!(matches!(
        accept_proposal(&seal_of(&snapshot), &snapshot, &proposal, current),
        Err(Rejection::Invalid(_))
    ));
    // A part no catalogue has, a file path for a preset, a NaN: none of them read.
    value["design"]["form"]["sculpt"]["shape"]["head"] = 5.into();
    value["design"]["form"]["sculpt"]["parts"][0]["kind"] = "ears.from_a_file".into();
    assert!(decode::<FarmProposal>(&serde_json::to_vec(&value).unwrap()).is_err());
    let mut value = serde_json::to_value(edit(&snapshot, 1, sculpted(Plan::Floater))).unwrap();
    value["lineage"]["preset"] = "/Users/someone/.ssh/id_rsa".into();
    assert!(decode::<FarmProposal>(&serde_json::to_vec(&value).unwrap()).is_err());
    let text = String::from_utf8(encode(&edit(&snapshot, 1, sculpted(Plan::Floater))).unwrap())
        .unwrap()
        .replacen("\"size\": 5", "\"size\": NaN", 1);
    assert!(decode::<FarmProposal>(text.as_bytes()).is_err());
}

#[test]
fn a_new_companion_is_only_a_design_handed_to_desktops_own_welcome() {
    let snapshot = sample::create_snapshot();
    let seal = seal_of(&snapshot);
    let proposal = FarmProposal::new(
        &seal,
        1,
        sample::OPENED,
        "0.1.0",
        ProposalKind::CreateNew,
        sculpted(Plan::LargeQuadruped),
        Lineage::new(Some("animal.african_elephant"), None),
    );
    match accept_proposal(&seal, &snapshot, &proposal, Current::Nobody) {
        Ok(Accepted::Create { design, lineage }) => {
            assert_eq!(design, proposal.design);
            assert_eq!(lineage.preset.as_deref(), Some("animal.african_elephant"));
        }
        other => panic!("{other:?}"),
    }
    // Desktop's welcome makes the companion; the design then goes on it like any other.
    let mut newcomer = sample::colony().creatures[2].clone();
    apply_design(&mut newcomer, &proposal.design);
    assert_eq!(Design::of(&newcomer.appearance), proposal.design);
    // An edit cannot be slipped into a create session, nor a create into an edit.
    let edit_snapshot = sample::edit_snapshot(0);
    let wrong = FarmProposal::new(
        &seal_of(&edit_snapshot),
        1,
        sample::OPENED,
        "0.1.0",
        ProposalKind::CreateNew,
        sculpted(Plan::Floater),
        Lineage::default(),
    );
    let save = sample::colony();
    let current = Current::Creature {
        creature: &save.creatures[0],
        available: true,
    };
    assert_eq!(
        accept_proposal(&seal_of(&edit_snapshot), &edit_snapshot, &wrong, current),
        Err(Rejection::Unsupported)
    );
}

#[test]
fn a_creature_from_before_recipes_can_keep_its_look_and_nothing_can_be_made_into_one() {
    let save = sample::colony();
    let index = save
        .creatures
        .iter()
        .position(|c| c.appearance.design.is_none())
        .expect("the sample colony has one from before recipes");
    let snapshot = sample::edit_snapshot(index);
    let creature = &save.creatures[index];
    let mut face = creature.appearance.face;
    face.eye_spacing = if face.eye_spacing == 4 { 5 } else { 4 };
    let keep = Design {
        form: Form::Original,
        face,
    };
    let proposal = edit(&snapshot, 1, keep.clone());
    let current = Current::Creature {
        creature,
        available: true,
    };
    let Ok(Accepted::Edit { design, .. }) =
        accept_proposal(&seal_of(&snapshot), &snapshot, &proposal, current)
    else {
        panic!("kept")
    };
    let mut changed = creature.clone();
    apply_design(&mut changed, &design);
    assert_eq!(changed.appearance.sculpt, None);
    assert_eq!(changed.appearance.face.eye_spacing, face.eye_spacing);
    assert_eq!(changed.appearance.design, None);
    // Someone with a recipe cannot be given one.
    let other = sample::edit_snapshot(0);
    let proposal = edit(&other, 1, keep);
    let current = Current::Creature {
        creature: &save.creatures[0],
        available: true,
    };
    assert!(matches!(
        accept_proposal(&seal_of(&other), &other, &proposal, current),
        Err(Rejection::Invalid(_))
    ));
}

#[test]
fn a_snapshot_tells_farm_nothing_about_the_creatures_life() {
    let text = String::from_utf8(encode(&sample::edit_snapshot(0)).unwrap()).unwrap();
    for private in [
        "temperament",
        "habits",
        "relationships",
        "memory",
        "axes",
        "phrase",
        "position",
        "colony_seed",
        "source_colony_seed",
    ] {
        assert!(!text.contains(private), "{private}");
    }
    assert!(decode::<FarmSnapshot>(text.as_bytes()).is_ok());
}
