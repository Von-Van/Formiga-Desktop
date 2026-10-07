use super::*;

#[test]
fn a_new_look_is_written_once_in_desktops_own_moment() {
    let created = datetime!(2026-10-07 9:00 UTC);
    let mut world = two_creature_world([85; 32], created);
    let reshaped = world.save.creatures[0].id;
    let now = created + Duration::hours(2);
    assert!(world.note_new_look(reshaped, now));
    assert!(
        !world.note_new_look(reshaped, now + Duration::minutes(20)),
        "a few looks tried in one sitting are one line"
    );
    assert!(world.note_new_look(reshaped, now + Duration::hours(7)));
    assert!(
        !world.note_new_look(424_242, now + Duration::hours(8)),
        "nobody by that id lives here"
    );
    let written: Vec<_> = world
        .save
        .companion
        .journal
        .iter()
        .filter(|entry| entry.moment == JournalMoment::NewLook)
        .collect();
    assert_eq!(written.len(), 2);
    assert!(written.iter().all(|entry| entry.creature == Some(reshaped)));
    assert_eq!(JournalMoment::NewLook.kind(), MomentKind::Ways);
    assert!(!JournalMoment::NewLook.is_noteworthy());
}
