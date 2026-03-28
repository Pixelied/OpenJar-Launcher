use crate::*;
#[test]
fn mod_entries_use_core_jar_filename_as_name() {
    let name = canonical_lock_entry_name("mods", "meteor-client-1.21.1-0.5.8.jar", "Meteor");
    assert_eq!(name, "meteor-client-1.21.1-0.5.8");
}

#[test]
fn mod_entries_strip_disabled_suffix_before_naming() {
    let name = canonical_lock_entry_name(
        "mods",
        "trouser-streak-1.21.1.jar.disabled",
        "Trouser Streak",
    );
    assert_eq!(name, "trouser-streak-1.21.1");
}

#[test]
fn non_mod_entries_keep_existing_name() {
    let name = canonical_lock_entry_name(
        "resourcepacks",
        "fresh-animations-1.0.0.zip",
        "Fresh Animations",
    );
    assert_eq!(name, "Fresh Animations");
}
