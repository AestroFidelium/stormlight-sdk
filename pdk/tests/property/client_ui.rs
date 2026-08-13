//! The HUD authoring path (stormlight/server#66) — a cosmetic mod declares widget
//! trees on its [`ClientContext`] the same way it declares visuals, and `finish`
//! must hand the host a bundle it can adopt. Laws:
//!   - **Reachable**: every handle a declared binding carries resolves to its name
//!     in the matching `Names` table, so adoption can map it by name.
//!   - **Author-order**: roots come out in the order they were declared, and each
//!     one the builders produced passes `validate` — the authoring path cannot
//!     express a tree the host would reject.
//!   - **Emit/decode**: `finish` → postcard → decode reproduces the exact bundle,
//!     UI included.

use bolero::{TypeGenerator, check};
use stormlight_mod_sdk::abi::ids::Slot;
use stormlight_mod_sdk::abi::impacts::PoolRef;
use stormlight_mod_sdk::abi::ui::{
    Anchor, Length, RootVisibility, UiAction, UiSubject, ValueBinding, ValuePart,
};
use stormlight_mod_sdk::abi::visuals::ClientRegistration;
use stormlight_mod_sdk::bindings::to_bytes_client;
use stormlight_mod_sdk::client::ClientContext;
use stormlight_mod_sdk::ui::{
    WidgetExt, ability_slot, bar, bound_text, button, icon, panel, talent_list, text,
};

#[derive(Debug, TypeGenerator)]
struct Scenario {
    /// 0..=3 resource bars in the HUD.
    resources: u8,
    /// 0..=3 ability slots on the bar.
    slots: u8,
    /// 0..=2 extra stat readouts.
    stats: u8,
}

/// The HUD a scenario asks for: a health bar, one bar per named resource, an
/// ability row, a stat readout column, and a talent panel gated on a pending pick.
fn author(s: &Scenario, ctx: &mut ClientContext) -> (usize, usize) {
    let resources = (s.resources % 4) as usize;
    let stats = (s.stats % 3) as usize;
    let slots = (s.slots % 4) as usize;

    let mut bars = vec![bar(ValueBinding::Health).sized(Length::Px(180.0), Length::Px(12.0))];
    for i in 0..resources {
        let res = ctx.resource(&format!("res{i}"));
        bars.push(bar(ValueBinding::Pool(PoolRef::Resource(res))));
    }
    for i in 0..stats {
        let stat = ctx.stat(&format!("stat{i}"));
        bars.push(bound_text(ValueBinding::Stat(stat), ValuePart::Current, 0).font_size(12.0));
    }
    ctx.ui(
        "vitals",
        RootVisibility::Always,
        UiSubject::LocalPlayer,
        panel(bars).at(Anchor::BottomLeft, Length::Px(16.0), Length::Px(-16.0)),
    );

    let row: Vec<_> =
        (0..slots).map(|i| ability_slot(Slot(i as u8), "QWER").image("mod://m/slot.png")).collect();
    ctx.ui(
        "abilities",
        RootVisibility::Always,
        UiSubject::LocalPlayer,
        panel(row).at(Anchor::BottomCenter, Length::Px(0.0), Length::Px(-8.0)),
    );

    ctx.ui(
        "talents",
        RootVisibility::WhileTalentPending,
        UiSubject::LocalPlayer,
        panel(vec![
            text("choose"),
            talent_list(),
            button(UiAction::PickTalent { option: 0 }, vec![icon("mod://m/pick.png")]),
        ]),
    );

    (resources, stats)
}

#[test]
fn every_bound_handle_resolves_to_its_declared_name() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ClientContext::new();
        let (resources, stats) = author(s, &mut ctx);
        let reg = ctx.finish();

        assert_eq!(reg.names.resources.len(), resources, "one name per declared resource");
        assert_eq!(reg.names.stats.len(), stats, "one name per declared stat");
        for (raw, name) in reg.names.resources.iter().enumerate() {
            assert_eq!(name, &format!("res{raw}"), "resource table is not dense/in order");
        }
        for (raw, name) in reg.names.stats.iter().enumerate() {
            assert_eq!(name, &format!("stat{raw}"), "stat table is not dense/in order");
        }
    });
}

#[test]
fn declared_roots_keep_author_order_and_validate() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ClientContext::new();
        author(s, &mut ctx);
        let reg = ctx.finish();

        let names: Vec<&str> = reg.ui.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["vitals", "abilities", "talents"], "roots left author order");
        for root in &reg.ui {
            assert_eq!(root.validate(), Ok(()), "the builders authored an invalid tree");
        }
    });
}

#[test]
fn interning_the_same_name_twice_reuses_one_handle() {
    check!().with_type::<u8>().for_each(|&_seed| {
        let mut ctx = ClientContext::new();
        let a = ctx.resource("energy");
        let b = ctx.resource("energy");
        let s1 = ctx.stat("power");
        let s2 = ctx.stat("power");
        assert_eq!(a, b, "one name, one resource handle");
        assert_eq!(s1, s2, "one name, one stat handle");
        let reg = ctx.finish();
        assert_eq!(reg.names.resources.len(), 1);
        assert_eq!(reg.names.stats.len(), 1);
    });
}

#[test]
fn a_built_bundle_with_ui_decodes_equal() {
    check!().with_type::<Scenario>().for_each(|s| {
        let mut ctx = ClientContext::new();
        author(s, &mut ctx);
        let reg = ctx.finish();

        let decoded: ClientRegistration =
            postcard::from_bytes(&to_bytes_client(&reg)).expect("decode");
        assert_eq!(reg, decoded, "a bundle carrying UI did not survive the wasm boundary");
    });
}
