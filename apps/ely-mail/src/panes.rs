//! The message list and the reading pane (C12.9).

use ely_gpui_component::{
    buttons::{ToggleGroup, ToggleItem},
    data_display::DescriptionList,
    feedback::{EmptyState, ErrorView},
    layout::SplitPane,
    mail::MailList,
    primitives::IconName,
};
use ely_mail_engine::Filter;
use gpui::{
    AnyElement, Axis, Context, IntoElement, ParentElement as _, SharedString, Styled as _, div, px,
};

use crate::{
    mail::{Mail, text},
    view::local_time,
};

type Filtered = fn(Filter) -> bool;

const FILTERS: [(&str, &str, IconName, Filtered); 3] = [
    ("unread", "mail.filter.unread", IconName::Mail, |f| f.unread),
    ("flagged", "mail.filter.flagged", IconName::Star, |f| {
        f.flagged
    }),
    (
        "attachments",
        "mail.filter.attachments",
        IconName::Paperclip,
        |f| f.attachments,
    ),
];

impl Mail {
    pub fn panes(&self, cx: &mut Context<Self>) -> AnyElement {
        SplitPane::new("mail-panes", Axis::Horizontal, px(240.))
            .pane(self.list_pane(cx))
            .pane(self.reader(cx))
            .into_any_element()
    }

    fn list_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let on: Vec<&str> = FILTERS
            .iter()
            .filter(|(_, _, _, on)| on(self.filter))
            .map(|(key, _, _, _)| *key)
            .collect();
        let toggles = FILTERS.iter().fold(
            ToggleGroup::new("mail-filters").multiple().selected(on),
            |group, (key, label, icon, _)| {
                let words = text(cx, label, &[]);
                group.item(ToggleItem::new(*key).icon(*icon).label(words))
            },
        );
        let this = cx.weak_entity();
        let toggles = toggles.on_change(move |keys, _, cx| {
            let has = |key: &str| keys.iter().any(|each| each == key);
            let filter = Filter {
                unread: has("unread"),
                flagged: has("flagged"),
                attachments: has("attachments"),
            };
            this.update(cx, |mail, cx| mail.set_filter(filter, cx)).ok();
        });
        let list = match &self.listing.mails {
            None => div().into_any_element(),
            Some(Err(detail)) => ErrorView::new(
                "mail-list-error",
                IconName::TriangleAlert,
                text(cx, "mail.list.unread", &[]),
            )
            .detail(detail.clone())
            .into_any_element(),
            Some(Ok(mails)) => {
                let this = cx.weak_entity();
                MailList::new("mail-list", mails.clone())
                    .selected(self.listing.selected.clone())
                    .on_select(move |keys, _, cx| {
                        this.update(cx, |mail, cx| mail.select(keys, cx)).ok();
                    })
                    .into_any_element()
            }
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().p_2().child(toggles))
            .child(div().flex_1().min_h_0().child(list))
            .into_any_element()
    }

    fn reader(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(row) = &self.listing.chosen else {
            return EmptyState::new(
                "mail-reader-empty",
                IconName::Mail,
                text(cx, "mail.reader.none", &[]),
            )
            .into_any_element();
        };
        let label = |key: &str| text(cx, key, &[]);
        let date: SharedString = match row.at {
            Some(at) => local_time(at.as_second()).into(),
            None => label("mail.reader.no_date"),
        };
        let details = DescriptionList::new()
            .text(label("mail.reader.from"), row.from.clone())
            .text(label("mail.reader.date"), date);
        div()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_xl().child(row.subject.clone()))
            .child(details)
            .into_any_element()
    }
}
