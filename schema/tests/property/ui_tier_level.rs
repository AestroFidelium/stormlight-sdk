//! What a tier's button may print (stormlight/server#111).
//!
//! A tier strip has to be labelled, and the two things an interface could reach for
//! before this were both wrong. A literal numeral is a HUD authoring content — and
//! worse, authoring it *badly*, because `["I", "II", …][tier]` is an array with a
//! length and a tree may declare more tiers than it has entries. The subject's own
//! level does not help either: it says where the player is, not where the next
//! talent is.
//!
//! [`ValueBinding::TierLevel`] is the number the tree already carries — the unit
//! level at which that tier becomes choosable. It goes in `ValueBinding` rather than
//! in `TextSource` because that is exactly what a `ValueBinding` is, "a number read
//! from generic state", and putting it there means a tier's level is printable,
//! comparable and drawable by every mechanism a number already has.
//!
//! **A coordinate, not a level.** The interface names a tier *index*; what level
//! that unlocks at is the tree's answer. So a mod that declares its tiers at
//! different levels needs no HUD change, and this HUD names no content.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ui::{
    Layout, RootVisibility, Strip, Style, SummonGate, TextSource, UiRoot, UiSubject, ValueBinding,
    ValuePart, Widget, WidgetKind,
};

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// The tier each label names.
    tiers: Vec<u8>,
    /// Which part of the reading it prints — a tier's level is unbounded, so all
    /// three have to survive the wire even where two of them are the same number.
    part: u8,
}

fn part(byte: u8) -> ValuePart {
    match byte % 3 {
        0 => ValuePart::Current,
        1 => ValuePart::Max,
        _ => ValuePart::Fraction,
    }
}

fn a_root(s: &Scenario) -> UiRoot {
    let children = s
        .tiers
        .iter()
        .map(|&tier| Widget {
            name: format!("tier{tier}"),
            layout: Layout::default(),
            style: Style::default(),
            kind: WidgetKind::Text {
                text: TextSource::Value {
                    binding: ValueBinding::TierLevel(tier),
                    part: part(s.part),
                    decimals: 0,
                },
            },
        })
        .collect();
    UiRoot {
        name: "strip".into(),
        when: RootVisibility::Always,
        summon: SummonGate::Ignored,
        subject: UiSubject::LocalPlayer,
        strip: Strip::default(),
        root: Widget {
            name: "strip".into(),
            layout: Layout::default(),
            style: Style::default(),
            kind: WidgetKind::Panel { children },
        },
    }
}

#[test]
fn a_tier_level_survives_the_trip_to_the_host() {
    check!().with_type::<Scenario>().for_each(|s| {
        let declared = a_root(s);
        let bytes = postcard::to_allocvec(&declared).expect("a strip encodes");
        let decoded: UiRoot = postcard::from_bytes(&bytes).expect("and decodes");
        assert_eq!(decoded, declared, "a tier coordinate did not survive the wire");
    });
}

#[test]
fn any_tier_coordinate_is_a_legal_one() {
    check!().with_type::<Scenario>().for_each(|s| {
        // Including tiers past the end of every tree there is. A coordinate that
        // names nothing prints nothing, exactly as a talent coordinate does — it is
        // not a way for a declaration to be *wrong*, because the interface cannot
        // know which unit it will be shown for.
        assert_eq!(a_root(s).validate(), Ok(()), "a tier coordinate was refused");
    });
}

#[test]
fn it_is_a_binding_rather_than_a_talent_coordinate() {
    // Stated as a test because the alternative was tempting and would have been
    // wrong: `TextSource::Talent` addresses `(tier, option)` and reads a *card*,
    // which is the offered talent's own content. A tier's level belongs to the tree
    // and to no option in it, so it reads like every other number.
    let binding = ValueBinding::TierLevel(3);
    let text = TextSource::Value { binding, part: ValuePart::Current, decimals: 0 };
    assert!(
        matches!(text, TextSource::Value { binding: ValueBinding::TierLevel(3), .. }),
        "a tier's unlock level is not reachable as an ordinary number",
    );
}
