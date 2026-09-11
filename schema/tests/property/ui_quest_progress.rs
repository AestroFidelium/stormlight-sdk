//! What a panel prints about a task in progress (stormlight/server#132).
//!
//! The mark said *that* a row is a quest; this is the running count. It is a
//! [`ValueBinding`] rather than a [`TalentText`](stormlight_mod_abi::ui::TalentText)
//! because that is exactly what it is — a number read from generic state — and
//! putting it there means a quest's progress is printable, comparable and
//! **drawable as a bar** by every mechanism a number already has, instead of being
//! a string only a text widget can consume.
//!
//! **A coordinate, not a counter.** Which stack counter a task is counted in is the
//! gameplay mod's business; a HUD that named it would be authoring content, and
//! would break the moment the mod renamed it. The panel names the same
//! `(tier, option)` it already names for the row's words, its icon and its click.
//!
//! Three parts, all meaningful, which is the argument for one binding over two
//! texts: the count, the goal it is counted toward, and the fraction between them.

use bolero::{TypeGenerator, check};
use stormlight_mod_abi::ids::{StackId, StatId};
use stormlight_mod_abi::impacts::PoolRef;
use stormlight_mod_abi::ui::{
    Layout, RootVisibility, Strip, Style, SummonGate, TextSource, UiRoot, UiSubject, ValueBinding,
    ValuePart, Widget, WidgetKind,
};

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// The coordinates a panel prints a count at — including ones past the end of
    /// every tree there is.
    cells: Vec<(u8, u8)>,
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
        .cells
        .iter()
        .map(|&(tier, option)| Widget {
            name: format!("count{tier}_{option}"),
            layout: Layout::default(),
            style: Style::default(),
            kind: WidgetKind::Text {
                text: TextSource::Value {
                    binding: ValueBinding::TalentQuest { tier, option },
                    part: part(s.part),
                    decimals: 0,
                },
            },
        })
        .collect();
    UiRoot {
        name: "panel".into(),
        when: RootVisibility::Always,
        summon: SummonGate::Ignored,
        subject: UiSubject::LocalPlayer,
        strip: Strip::default(),
        root: Widget {
            name: "panel".into(),
            layout: Layout::default(),
            style: Style::default(),
            kind: WidgetKind::Panel { children },
        },
    }
}

#[test]
fn a_quest_coordinate_survives_the_trip_to_the_host() {
    check!().with_type::<Scenario>().for_each(|s| {
        let declared = a_root(s);
        let bytes = postcard::to_allocvec(&declared).expect("a panel encodes");
        let decoded: UiRoot = postcard::from_bytes(&bytes).expect("and decodes");
        assert_eq!(decoded, declared, "a quest coordinate did not survive the wire");
    });
}

/// Like every other talent coordinate: a cell past the end of a short tree is not
/// a declaration that is *wrong*, because an interface cannot know which unit it
/// will be shown for. It reads empty at runtime instead.
#[test]
fn any_coordinate_is_a_legal_one() {
    check!().with_type::<Scenario>().for_each(|s| {
        assert_eq!(a_root(s).validate(), Ok(()), "a quest coordinate was refused");
    });
}

/// The variant order is the wire tag. This one is **appended**: putting it beside
/// `Pool`, where a reserve arguably belongs, would move `Stat`, `Level`,
/// `CastProgress`, `Event` and `TierLevel` each by one and silently reinterpret
/// every descriptor already built.
#[test]
fn it_took_the_tag_after_the_ones_that_existed() {
    for (tag, binding) in [
        ValueBinding::Health,
        ValueBinding::Pool(PoolRef::Shield),
        ValueBinding::Stat(StatId(0)),
        ValueBinding::Level,
        ValueBinding::CastProgress,
    ]
    .iter()
    .enumerate()
    {
        let bytes = postcard::to_allocvec(binding).expect("serialize");
        assert_eq!(bytes.first(), Some(&(tag as u8)), "{binding:?} moved off wire tag {tag}");
    }
    let tier = postcard::to_allocvec(&ValueBinding::TierLevel(0)).expect("serialize");
    assert_eq!(tier.first(), Some(&6), "the tier level is not the seventh tag");
    let quest = postcard::to_allocvec(&ValueBinding::TalentQuest { tier: 0, option: 0 })
        .expect("serialize");
    assert_eq!(quest.first(), Some(&7), "the quest count is not the eighth tag");
}

/// Stated as a test because the alternative was tempting and would have been
/// wrong. A HUD *can* name a stack counter directly — that is what `Pool` is for,
/// and it is how a mod draws a combo meter of its own. But a quest's counter is
/// named by the gameplay mod that declared the talent, and an interface repeating
/// that handle would be authoring content and would go silently wrong the moment
/// the two disagreed.
#[test]
fn a_bare_counter_is_still_reachable_and_is_a_different_question() {
    let mine = ValueBinding::Pool(PoolRef::Stacks(StackId(3)));
    let theirs = ValueBinding::TalentQuest { tier: 1, option: 2 };
    assert_ne!(mine, theirs, "the two ways to read a counter collapsed into one");
}
